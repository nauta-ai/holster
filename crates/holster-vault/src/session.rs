//! Session tokens and session state.
//!
//! T1.6: in-memory session store with idle-timeout validation.
//!
//! Design:
//! - `SessionToken` is a UUID v4 newtype. Token is a session identifier, not
//!   crypto material — its Debug shows the UUID intentionally (for logging).
//! - `SessionState` carries the derived AES key wrapped in `secrecy::Secret`,
//!   plus created_at / last_activity_at timestamps.
//! - `SessionStore` is a Mutex-wrapped HashMap. Sessions are NOT persisted —
//!   they live only in process memory. App restart = all sessions invalidated.
//! - Idle timeout is checked on every `validate_token` call. Expired tokens
//!   are removed (and their AES keys dropped/zeroized) atomically.
//! - On `revoke_token`, the SessionState is dropped → secrecy::Secret zeroizes.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use chrono::{DateTime, Utc};
use secrecy::{ExposeSecret, Secret};
use uuid::Uuid;

use crate::error::VaultError;

/// Default idle timeout — 15 minutes. Configurable via `SessionStore::with_timeout`.
const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(15 * 60);

// ── SessionToken ─────────────────────────────────────────────────────────────

/// Session identifier. UUID v4 wrapped to prevent confusion with `KeyMetadata.id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionToken(Uuid);

impl SessionToken {
    /// Generate a fresh token. Each session gets a new one.
    pub fn new() -> Self {
        SessionToken(Uuid::new_v4())
    }

    /// Underlying UUID — for serialization or logging.
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for SessionToken {
    fn default() -> Self {
        Self::new()
    }
}

// ── SessionState ─────────────────────────────────────────────────────────────

/// In-memory session state. The AES key is wrapped in `Secret` so it cannot
/// leak via Debug, Display, or accidental Serialize. Dropping the struct
/// (e.g. via `revoke_token`) zeroizes the key as part of `Secret::drop`.
pub struct SessionState {
    pub token: SessionToken,
    pub created_at: DateTime<Utc>,
    pub last_activity_at: DateTime<Utc>,
    pub aes_key: Secret<[u8; 32]>,
}

impl SessionState {
    pub fn new(aes_key: Secret<[u8; 32]>) -> Self {
        let now = Utc::now();
        SessionState {
            token: SessionToken::new(),
            created_at: now,
            last_activity_at: now,
            aes_key,
        }
    }

    /// True if `now - last_activity_at > timeout`.
    pub fn is_idle_expired(&self, timeout: Duration, now: DateTime<Utc>) -> bool {
        let elapsed = now.signed_duration_since(self.last_activity_at);
        match elapsed.to_std() {
            Ok(d) => d > timeout,
            Err(_) => false, // negative elapsed (clock skew) — not expired
        }
    }
}

// Hand-rolled Debug — never expose the AES key.
impl std::fmt::Debug for SessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionState")
            .field("token", &self.token)
            .field("created_at", &self.created_at)
            .field("last_activity_at", &self.last_activity_at)
            .field("aes_key", &"<redacted>")
            .finish()
    }
}

// ── SessionStore ─────────────────────────────────────────────────────────────

pub struct SessionStore {
    sessions: Mutex<HashMap<SessionToken, SessionState>>,
    idle_timeout: Duration,
}

impl SessionStore {
    /// New store with the default 15-minute idle timeout.
    pub fn new() -> Self {
        Self::with_timeout(DEFAULT_IDLE_TIMEOUT)
    }

    /// New store with a custom idle timeout. Useful for tests.
    pub fn with_timeout(idle_timeout: Duration) -> Self {
        SessionStore {
            sessions: Mutex::new(HashMap::new()),
            idle_timeout,
        }
    }

    /// Create a new session with the given AES key. Returns the new token.
    /// The key is moved into the store; caller no longer holds it.
    pub fn create(&self, aes_key: Secret<[u8; 32]>) -> Result<SessionToken, VaultError> {
        let state = SessionState::new(aes_key);
        let token = state.token;
        let mut sessions = self.lock_sessions()?;
        sessions.insert(token, state);
        Ok(token)
    }

    /// Validate a token. Errors if not present or idle-expired. Expired
    /// sessions are removed (key zeroized) before the error is returned.
    pub fn validate(&self, token: SessionToken) -> Result<(), VaultError> {
        let mut sessions = self.lock_sessions()?;
        let state = sessions.get(&token).ok_or(VaultError::InvalidSession)?;
        if state.is_idle_expired(self.idle_timeout, Utc::now()) {
            sessions.remove(&token);  // drops Secret → zeroizes
            return Err(VaultError::SessionExpired);
        }
        Ok(())
    }

    /// Update last_activity_at. Errors if token missing or already expired.
    /// Caller usually invokes this after a successful operation.
    pub fn touch(&self, token: SessionToken) -> Result<(), VaultError> {
        let mut sessions = self.lock_sessions()?;
        let state = sessions.get_mut(&token).ok_or(VaultError::InvalidSession)?;
        if state.is_idle_expired(self.idle_timeout, Utc::now()) {
            sessions.remove(&token);
            return Err(VaultError::SessionExpired);
        }
        state.last_activity_at = Utc::now();
        Ok(())
    }

    /// Look up the AES key for a valid session. Caller is responsible for
    /// using the secret only as long as needed and not cloning it around.
    /// We return the secret by clone (cheap — 32 bytes); caller drops it.
    pub fn aes_key(&self, token: SessionToken) -> Result<Secret<[u8; 32]>, VaultError> {
        let sessions = self.lock_sessions()?;
        let state = sessions.get(&token).ok_or(VaultError::InvalidSession)?;
        if state.is_idle_expired(self.idle_timeout, Utc::now()) {
            return Err(VaultError::SessionExpired);
        }
        // Clone the underlying bytes into a fresh Secret. Both copies will
        // zeroize on drop.
        let bytes: [u8; 32] = *state.aes_key.expose_secret();
        Ok(Secret::new(bytes))
    }

    /// Revoke a session. Returns Ok(()) whether or not it existed (idempotent).
    /// On removal, dropping the SessionState drops its Secret → zeroizes the key.
    pub fn revoke(&self, token: SessionToken) -> Result<(), VaultError> {
        let mut sessions = self.lock_sessions()?;
        sessions.remove(&token);
        Ok(())
    }

    /// Number of active sessions. Mostly for tests.
    pub fn len(&self) -> Result<usize, VaultError> {
        Ok(self.lock_sessions()?.len())
    }

    /// Whether the store is empty. Mostly for tests.
    pub fn is_empty(&self) -> Result<bool, VaultError> {
        Ok(self.lock_sessions()?.is_empty())
    }

    fn lock_sessions(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, HashMap<SessionToken, SessionState>>, VaultError> {
        self.sessions
            .lock()
            .map_err(|_| VaultError::Crypto("session store mutex poisoned".into()))
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use chrono::Duration as ChronoDuration;

    fn fresh_key(byte: u8) -> Secret<[u8; 32]> {
        Secret::new([byte; 32])
    }

    #[test]
    fn create_yields_unique_tokens() {
        let store = SessionStore::new();
        let t1 = store.create(fresh_key(1)).unwrap();
        let t2 = store.create(fresh_key(2)).unwrap();
        assert_ne!(t1, t2);
        assert_eq!(store.len().unwrap(), 2);
    }

    #[test]
    fn validate_succeeds_for_fresh_session() {
        let store = SessionStore::new();
        let t = store.create(fresh_key(7)).unwrap();
        assert!(store.validate(t).is_ok());
    }

    #[test]
    fn validate_rejects_unknown_token() {
        let store = SessionStore::new();
        let bogus = SessionToken::new();
        let err = store.validate(bogus).unwrap_err();
        assert!(matches!(err, VaultError::InvalidSession));
    }

    #[test]
    fn validate_rejects_expired_session() {
        // Create with a 1-millisecond timeout so it expires immediately.
        let store = SessionStore::with_timeout(Duration::from_millis(1));
        let t = store.create(fresh_key(7)).unwrap();
        // Sleep past the timeout
        std::thread::sleep(Duration::from_millis(10));
        let err = store.validate(t).unwrap_err();
        assert!(matches!(err, VaultError::SessionExpired));
        // Expired session was removed
        assert_eq!(store.len().unwrap(), 0);
    }

    #[test]
    fn touch_updates_last_activity_at() {
        let store = SessionStore::with_timeout(Duration::from_secs(60));
        let t = store.create(fresh_key(7)).unwrap();
        // Force-rewind last_activity_at so we can detect the touch
        {
            let mut sessions = store.sessions.lock().unwrap();
            let s = sessions.get_mut(&t).unwrap();
            s.last_activity_at = Utc::now() - ChronoDuration::seconds(30);
        }
        store.touch(t).unwrap();
        let sessions = store.sessions.lock().unwrap();
        let s = sessions.get(&t).unwrap();
        let elapsed = Utc::now().signed_duration_since(s.last_activity_at);
        assert!(elapsed.num_seconds() < 5, "touch should bring last_activity within 5s of now");
    }

    #[test]
    fn touch_rejects_expired_and_removes() {
        let store = SessionStore::with_timeout(Duration::from_millis(1));
        let t = store.create(fresh_key(7)).unwrap();
        std::thread::sleep(Duration::from_millis(10));
        let err = store.touch(t).unwrap_err();
        assert!(matches!(err, VaultError::SessionExpired));
        assert_eq!(store.len().unwrap(), 0, "expired session should be removed by touch");
    }

    #[test]
    fn revoke_removes_session() {
        let store = SessionStore::new();
        let t = store.create(fresh_key(7)).unwrap();
        assert_eq!(store.len().unwrap(), 1);
        store.revoke(t).unwrap();
        assert!(store.is_empty().unwrap());
        // Token now invalid
        let err = store.validate(t).unwrap_err();
        assert!(matches!(err, VaultError::InvalidSession));
    }

    #[test]
    fn revoke_is_idempotent_on_unknown_token() {
        let store = SessionStore::new();
        let bogus = SessionToken::new();
        // Should not error
        store.revoke(bogus).unwrap();
    }

    #[test]
    fn aes_key_returns_clone_of_stored_secret() {
        let store = SessionStore::new();
        let original_byte = 0x42u8;
        let t = store.create(fresh_key(original_byte)).unwrap();
        let secret = store.aes_key(t).unwrap();
        let bytes = secret.expose_secret();
        assert_eq!(*bytes, [original_byte; 32]);
        // Original session still in store
        assert_eq!(store.len().unwrap(), 1);
    }

    #[test]
    fn aes_key_rejects_expired_session() {
        let store = SessionStore::with_timeout(Duration::from_millis(1));
        let t = store.create(fresh_key(7)).unwrap();
        std::thread::sleep(Duration::from_millis(10));
        let err = store.aes_key(t).unwrap_err();
        assert!(matches!(err, VaultError::SessionExpired));
    }

    #[test]
    fn session_state_debug_redacts_aes_key() {
        let state = SessionState::new(fresh_key(0xFF));
        let dbg = format!("{state:?}");
        assert!(dbg.contains("<redacted>"),
                "expected redacted marker, got: {dbg}");
        // 0xFF (which would render as "ff" or "255" or "[255, 255, ...]") must not appear
        assert!(!dbg.contains("255"), "Debug leaked AES key bytes: {dbg}");
        assert!(!dbg.contains("ff, ff"), "Debug leaked AES key bytes: {dbg}");
    }

    #[test]
    fn revoke_drops_secret_zeroize_path_smoke() {
        // We can't read freed memory in safe Rust to "see" zeros, but we can
        // verify revoke removes the entry and that the Secret drop runs without
        // panicking. zeroize::Zeroize is implemented for [u8; 32] in secrecy
        // already, so we trust the library and assert the drop path executes.
        let store = SessionStore::new();
        let t = store.create(fresh_key(0xAB)).unwrap();
        // Before revoke: fetching the key returns the right bytes
        let snap = store.aes_key(t).unwrap();
        assert_eq!(*snap.expose_secret(), [0xAB; 32]);
        drop(snap);  // local clone zeroizes here

        store.revoke(t).unwrap();  // remove + drop Secret → zeroize
        // Post-revoke: token is gone
        let err = store.aes_key(t).unwrap_err();
        assert!(matches!(err, VaultError::InvalidSession));
    }
}
