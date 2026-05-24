//! Public Vault facade — ties crypto + db + session into one user-facing API.
//!
//! T1.7: integration of T1.2-T1.6.
//!
//! Lifecycle:
//!   Vault::create(path, password)   → initialize a fresh vault on disk
//!   Vault::open(path)               → handle to an existing vault (locked)
//!   vault.unlock(password)          → derives keys, returns SessionToken
//!   vault.add_key / list_keys / get_key_value / delete_key
//!   vault.lock(token)               → revokes session (zeroizes session key)
//!
//! Key storage:
//!   Salt lives in a sidecar file `<vault.db>.salt` (mode 0600). It must be
//!   readable WITHOUT the SQLCipher key (chicken-and-egg solved). The db
//!   itself stores a redundant copy in vault_meta.
//!
//! Every method except create/open/unlock validates the session token first.
//! Calling any data method without a valid session → VaultError::InvalidSession.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use chrono::Utc;
use secrecy::{ExposeSecret, Secret};
use thiserror::Error;
use uuid::Uuid;

use crate::agent_profile::AgentProfileStore;
use crate::audit::{AuditEvent, AuditLogger, AuditOutcome, EventKind, FetchAuditEvent};
use crate::crypto::{decrypt_key_value, derive_keys, encrypt_key_value, generate_salt};
use crate::db::{Database, InsertKeyParams};
use crate::error::VaultError;
use crate::models::{AddKeyInput, KeyMetadata};
use crate::session::{SessionStore, SessionToken};

const SALT_LEN: usize = 16;
const MIN_PASSWORD_LEN: usize = 8;
const MIRROR_LOCK_TIMEOUT: Duration = Duration::from_secs(5);
const MIRROR_LOCK_POLL: Duration = Duration::from_millis(25);

#[derive(Debug, Error)]
pub enum MirrorError {
    #[error("mirror lock timed out")]
    LockTimeout,

    #[error("vault error: {0}")]
    Vault(#[from] VaultError),

    #[error("io error")]
    Io(#[from] std::io::Error),
}

#[derive(Clone)]
pub struct MirrorSecretInput {
    pub id: Uuid,
    pub provider: crate::models::Provider,
    pub label: String,
    pub key_value: String,
    pub project_tag: Option<String>,
    pub expires_at: Option<chrono::DateTime<Utc>>,
    pub notes: Option<String>,
}

impl std::fmt::Debug for MirrorSecretInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MirrorSecretInput")
            .field("id", &self.id)
            .field("provider", &self.provider)
            .field("label", &self.label)
            .field("key_value", &"<redacted>")
            .field("project_tag", &self.project_tag)
            .field("expires_at", &self.expires_at)
            .field("notes", &self.notes)
            .finish()
    }
}

#[derive(Clone, Debug)]
pub enum MirrorSecretEntry {
    Add(MirrorSecretInput),
    Delete { id: Uuid },
    Supersede { old_id: Uuid, new_id: Uuid },
}

struct MirrorLock {
    path: PathBuf,
    _file: File,
}

impl Drop for MirrorLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub fn mirror_secret_to(
    target_path: &Path,
    target_password: &str,
    entry: MirrorSecretEntry,
) -> Result<Option<KeyMetadata>, MirrorError> {
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let _lock = acquire_mirror_lock(target_path)?;
    match entry {
        MirrorSecretEntry::Add(input) => {
            let vault = open_or_create_mirror_vault(target_path, target_password)?;
            let token = vault.unlock(target_password)?;
            let metadata = vault.add_key_with_id(token, input)?;
            let _ = vault.lock(token);
            Ok(Some(metadata))
        }
        MirrorSecretEntry::Delete { id } => {
            if !target_path.exists() {
                return Ok(None);
            }
            let vault = Vault::open(target_path)?;
            let token = vault.unlock(target_password)?;
            match vault.delete_key(token, id) {
                Ok(()) => {
                    let _ = vault.lock(token);
                    Ok(None)
                }
                Err(VaultError::KeyNotFound(_)) => {
                    let _ = vault.lock(token);
                    Ok(None)
                }
                Err(err) => {
                    let _ = vault.lock(token);
                    Err(MirrorError::Vault(err))
                }
            }
        }
        MirrorSecretEntry::Supersede { old_id, new_id } => {
            if !target_path.exists() {
                return Ok(None);
            }
            let vault = Vault::open(target_path)?;
            let token = vault.unlock(target_password)?;
            let result = vault.mark_superseded(old_id, new_id);
            let _ = vault.lock(token);
            result?;
            Ok(None)
        }
    }
}

/// Public Vault handle. Created via `Vault::create` (new) or `Vault::open` (existing).
/// Operations require an active session via `unlock`.
pub struct Vault {
    db_path: PathBuf,
    sessions: SessionStore,
    /// Database connection, lazily opened on first successful unlock and
    /// kept alive for subsequent sessions until `Vault` is dropped.
    db: Mutex<Option<Database>>,
}

// Hand-rolled Debug — Vault holds an open SQLCipher connection (via Database)
// whose internal state contains the derived key. Exposing it via Debug would
// be a leak. Surface only path + lock state + session count.
impl std::fmt::Debug for Vault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let session_count = self.sessions.len().unwrap_or(0);
        let unlocked = self.db.try_lock().map(|s| s.is_some()).unwrap_or(false);
        f.debug_struct("Vault")
            .field("db_path", &self.db_path)
            .field("unlocked", &unlocked)
            .field("active_sessions", &session_count)
            .finish()
    }
}

impl Vault {
    /// Initialize a new vault at `path`. Generates a fresh salt, writes it
    /// to a sidecar file, derives keys, creates the SQLCipher schema.
    /// Returns a Vault that is **locked** — caller must `unlock(password)`.
    pub fn create(path: &Path, password: &str) -> Result<Self, VaultError> {
        if password.len() < MIN_PASSWORD_LEN {
            return Err(VaultError::WeakPassword);
        }
        if path.exists() {
            return Err(VaultError::VaultAlreadyExists);
        }

        let salt = generate_salt();
        write_salt_sidecar(path, &salt)?;

        let keys = derive_keys(password, &salt)?;
        let db = Database::open(path, keys.sqlcipher_key.expose_secret())?;
        // Mirror salt inside the db so it is recoverable from the sidecar's siblings.
        db.set_salt(&salt)?;
        // V-4 fix: tighten perms on the vault file to 0600 (owner rw only).
        // SQLCipher creates the file via SQLite, which inherits the user's
        // umask — typically 0644 on macOS. Mirrors the sidecar treatment.
        // Best-effort on Unix; not fatal on weird filesystems.
        set_vault_file_perms(path);
        // Drop db handle — start in locked state. Caller calls unlock() to get a session.
        drop(db);

        Ok(Vault {
            db_path: path.to_path_buf(),
            sessions: SessionStore::new(),
            db: Mutex::new(None),
        })
    }

    /// Open an existing vault. Returns a Vault that is **locked**. Caller
    /// must `unlock(password)` to do anything.
    pub fn open(path: &Path) -> Result<Self, VaultError> {
        if !path.exists() {
            return Err(VaultError::VaultNotFound);
        }
        if !salt_sidecar_path(path).exists() {
            return Err(VaultError::Migration(
                "vault salt sidecar missing — vault corrupted or wrong path".into(),
            ));
        }
        Ok(Vault {
            db_path: path.to_path_buf(),
            sessions: SessionStore::new(),
            db: Mutex::new(None),
        })
    }

    /// Unlock the vault with the master password. Derives keys, opens (or
    /// re-opens) the SQLCipher connection, and returns a fresh session
    /// token. Wrong password → VaultError::BadPassword.
    ///
    /// Implementation note (V-1 fix): every `unlock` opens a *fresh* SQLCipher
    /// connection with the freshly-derived key and runs a sentinel query
    /// against it. On success, the new connection replaces any prior slot
    /// contents. On failure, `BadPassword` is returned and the prior slot
    /// is preserved untouched. This guarantees wrong-password is detected
    /// at unlock time even when an earlier unlock left a connection open.
    /// The cost is one extra Argon2 + SQLCipher open per re-unlock —
    /// acceptable for an interactive flow.
    pub fn unlock(&self, password: &str) -> Result<SessionToken, VaultError> {
        let salt = read_salt_sidecar(&self.db_path)?;
        let keys = derive_keys(password, &salt)?;

        // Always open a fresh connection so the password is validated by
        // SQLCipher itself. Do this *before* taking the db_slot lock so
        // a slow Argon2/open doesn't block other readers, and so a wrong
        // password leaves the existing connection intact.
        let new_db = Database::open(&self.db_path, keys.sqlcipher_key.expose_secret())?;
        // Sentinel query — fails (decrypt error) if the PRAGMA key was wrong.
        new_db.get_salt().map_err(|_| VaultError::BadPassword)?;

        let mut db_slot = self
            .db
            .lock()
            .map_err(|_| VaultError::Crypto("vault db mutex poisoned".into()))?;
        // Replace any prior connection. Dropping the old `Database` closes
        // its connection cleanly; the new one is now the canonical handle.
        *db_slot = Some(new_db);
        drop(db_slot);

        // Create a session bound to the AES key derived above
        self.sessions.create(keys.aes_key)
    }

    /// Revoke a session. Idempotent — revoking an unknown token is OK.
    /// Note: the SQLCipher connection stays open (cheap to reuse on next unlock).
    pub fn lock(&self, token: SessionToken) -> Result<(), VaultError> {
        self.sessions.revoke(token)
    }

    // ── Key operations (require valid session) ───────────────────────────────

    pub fn add_key(
        &self,
        token: SessionToken,
        input: AddKeyInput,
    ) -> Result<KeyMetadata, VaultError> {
        self.add_key_with_id(
            token,
            MirrorSecretInput {
                id: Uuid::new_v4(),
                provider: input.provider,
                label: input.label,
                key_value: input.key_value,
                project_tag: input.project_tag,
                expires_at: input.expires_at,
                notes: input.notes,
            },
        )
    }

    fn add_key_with_id(
        &self,
        token: SessionToken,
        input: MirrorSecretInput,
    ) -> Result<KeyMetadata, VaultError> {
        if let Ok(existing) = self.with_db(|db| db.select_key_by_id(input.id).map(|r| r.metadata)) {
            self.sessions.validate(token)?;
            return Ok(existing);
        }

        let aes_key = self.aes_key_for(token)?;
        let (ciphertext, nonce) = encrypt_key_value(&aes_key, &input.key_value)?;

        let now = Utc::now();
        let params = InsertKeyParams {
            id: input.id,
            provider: input.provider,
            label: input.label,
            project_tag: input.project_tag,
            key_ciphertext: ciphertext,
            key_nonce: nonce,
            created_at: now,
            expires_at: input.expires_at,
            notes: input.notes,
        };
        let metadata = self.with_db(|db| db.insert_key(params))?;
        let event = AuditEvent::from_metadata(EventKind::Add, &metadata);
        self.append_audit_event(event)?;
        Ok(metadata)
    }

    pub fn list_keys(&self, token: SessionToken) -> Result<Vec<KeyMetadata>, VaultError> {
        self.sessions.validate(token)?;
        self.with_db(|db| db.select_all_metadata())
    }

    /// Decrypt and return a key value. Updates `last_used_at` on success.
    pub fn get_key_value(
        &self,
        token: SessionToken,
        id: Uuid,
    ) -> Result<Secret<String>, VaultError> {
        let aes_key = self.aes_key_for(token)?;
        let (ciphertext, nonce) = self.with_db(|db| {
            let rec = db.select_key_by_id(id)?;
            Ok::<_, VaultError>((rec.key_ciphertext, rec.key_nonce))
        })?;
        let plaintext = decrypt_key_value(&aes_key, &ciphertext, &nonce)?;
        // Best-effort: update last_used. Don't fail the read if this fails.
        let _ = self.with_db(|db| db.update_last_used(id, Utc::now()));
        // Touch session as well — extends idle timeout
        let _ = self.sessions.touch(token);
        Ok(plaintext)
    }

    pub fn delete_key(&self, token: SessionToken, id: Uuid) -> Result<(), VaultError> {
        self.sessions.validate(token)?;
        let metadata = self.with_db(|db| db.select_key_by_id(id).map(|rec| rec.metadata))?;
        self.with_db(|db| db.delete_key(id))?;
        let event = AuditEvent::from_metadata(EventKind::Delete, &metadata);
        self.append_audit_event(event)?;
        let _ = self.sessions.touch(token);
        Ok(())
    }

    pub fn audit_events(&self) -> Result<Vec<AuditEvent>, VaultError> {
        self.with_db(|db| db.select_audit_events())
    }

    pub fn append_audit_event(&self, event: AuditEvent) -> Result<(), VaultError> {
        self.with_db(|db| db.append_audit_event(&event))
    }

    pub fn mark_superseded(&self, old: Uuid, new: Uuid) -> Result<(), VaultError> {
        let old_metadata = self
            .with_db(|db| db.select_key_by_id(old).map(|rec| rec.metadata))
            .map_err(|_| VaultError::EntryNotFound(old))?;
        self.with_db(|db| db.mark_superseded(old, new))?;
        let event = AuditEvent::supersede(&old_metadata, new);
        self.append_audit_event(event)
    }

    /// Rotate the vault master password (v0.2.0).
    ///
    /// Verifies the old password by unlocking, then derives fresh keys from the
    /// new password + a new salt. Re-encrypts every entry's AES ciphertext under
    /// the new AES key and rekeys the underlying SQLCipher database with the
    /// new sqlcipher key — both under an exclusive SQLite transaction. On
    /// success, writes a new salt sidecar (overwriting the old) and appends a
    /// `MasterRotated` audit event.
    ///
    /// On any failure:
    ///   - Wrong old password → `VaultError::BadPassword`, no changes
    ///   - New password too short → `VaultError::WeakPassword`, no changes
    ///   - Crypto failure mid-rotation → transaction rolls back, old keys + salt
    ///     remain authoritative
    ///   - SQLCipher rekey failure (rare, after txn commit) → row updates are
    ///     persisted but DB stays under old SQLCipher key; caller can retry
    ///
    /// Returns the count of entries successfully re-encrypted.
    ///
    /// After successful rotation, any in-memory sessions held by callers are
    /// invalidated — callers must `unlock(new_password)` to do further work.
    pub fn rotate_master(
        &self,
        old_password: &str,
        new_password: &str,
    ) -> Result<usize, VaultError> {
        if new_password.len() < MIN_PASSWORD_LEN {
            return Err(VaultError::WeakPassword);
        }

        // 1. Verify old password by unlocking — this also opens (or refreshes)
        //    the SQLCipher connection that rotate_master_atomic will use.
        let session = self.unlock(old_password)?;
        let old_aes_key = self.aes_key_for(session)?;

        // 2. Derive new keys + salt from the new password.
        let new_salt = generate_salt();
        let new_keys = derive_keys(new_password, &new_salt)?;

        // 3. Atomic re-encrypt of every row + SQLCipher rekey + vault_meta.salt update.
        //    The re_encrypt closure is called once per entry inside db.rs;
        //    if it errors, the transaction rolls back and the vault is intact.
        let count = self.with_db(|db| {
            db.rotate_master_atomic(
                &new_salt,
                new_keys.sqlcipher_key.expose_secret(),
                |old_ct, old_nonce| {
                    let plaintext = decrypt_key_value(&old_aes_key, old_ct, old_nonce)?;
                    encrypt_key_value(&new_keys.aes_key, plaintext.expose_secret())
                },
            )
        })?;

        // 4. Overwrite the salt sidecar with the new salt so future opens()
        //    derive matching keys. Atomic via std::fs::write (single syscall;
        //    macOS/Linux are atomic-rename internally for same-fs writes).
        write_salt_sidecar(&self.db_path, &new_salt)?;

        // 5. Audit the rotation BEFORE invalidating the session. The SQLCipher
        //    connection is still valid (PRAGMA rekey rekeyed the open conn too)
        //    and append_audit_event uses with_db (no session lookup).
        let event = AuditEvent::master_rotated(count);
        self.append_audit_event(event)?;

        // 6. Invalidate the old session — its AES key matches nothing now.
        //    Caller must unlock(new_password) for any further data operations.
        self.lock(session).ok();

        Ok(count)
    }

    /// Fetch a key for an agent through a metadata allowlist and audit logger.
    ///
    /// This is the first runtime-safe path for fake-key testing. It never logs,
    /// prints, or serializes the plaintext key value. Callers get a
    /// `Secret<String>` only after the agent profile allows the metadata match.
    pub fn fetch_key_for_agent(
        &self,
        token: SessionToken,
        agent_id: &str,
        id: Uuid,
        profiles: &AgentProfileStore,
        audit: &AuditLogger,
    ) -> Result<Secret<String>, VaultError> {
        self.sessions.validate(token)?;

        let metadata = self.with_db(|db| db.select_key_by_id(id).map(|record| record.metadata))?;

        if !profiles.allows(agent_id, &metadata) {
            let event = FetchAuditEvent::fetch(
                agent_id,
                &metadata,
                AuditOutcome::Denied,
                Some("agent_profile_denied"),
            );
            audit.log(&event)?;
            return Err(VaultError::AccessDenied);
        }

        let secret = self.get_key_value(token, id)?;
        let event = FetchAuditEvent::fetch(agent_id, &metadata, AuditOutcome::Allowed, None);
        audit.log(&event)?;
        Ok(secret)
    }

    // ── Private helpers ──────────────────────────────────────────────────────

    /// Returns the AES key for a valid session, or an error.
    fn aes_key_for(&self, token: SessionToken) -> Result<Secret<[u8; 32]>, VaultError> {
        self.sessions.aes_key(token)
    }

    /// Run a closure with the open Database. Errors with Locked if no session
    /// has yet opened the db.
    fn with_db<R>(
        &self,
        f: impl FnOnce(&Database) -> Result<R, VaultError>,
    ) -> Result<R, VaultError> {
        let db_slot = self
            .db
            .lock()
            .map_err(|_| VaultError::Crypto("vault db mutex poisoned".into()))?;
        match db_slot.as_ref() {
            Some(db) => f(db),
            None => Err(VaultError::Locked),
        }
    }
}

// ── File-permission helpers ───────────────────────────────────────────────────

/// V-4 fix: ensure the vault DB file is 0600 (owner read/write only) on Unix.
/// SQLite creates the file under the user's umask (typically 0644 on macOS),
/// which leaves the ciphertext world-readable. The contents are still
/// SQLCipher-encrypted, but tightening perms is cheap defense in depth and
/// mirrors how the salt sidecar is handled.
///
/// Best-effort: failures are intentionally ignored (some filesystems do not
/// honour Unix mode bits). On non-Unix targets this is a no-op.
fn set_vault_file_perms(_vault_path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(_vault_path, std::fs::Permissions::from_mode(0o600));
    }
}

// ── Salt sidecar helpers ──────────────────────────────────────────────────────

fn salt_sidecar_path(vault_path: &Path) -> PathBuf {
    let mut p = vault_path.to_path_buf();
    let new_name = format!(
        "{}.salt",
        p.file_name().and_then(|s| s.to_str()).unwrap_or("vault")
    );
    p.set_file_name(new_name);
    p
}

fn write_salt_sidecar(vault_path: &Path, salt: &[u8; SALT_LEN]) -> Result<(), VaultError> {
    let path = salt_sidecar_path(vault_path);
    std::fs::write(&path, salt)?;
    // Tighten perms on Unix (best-effort; not fatal if it fails)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn read_salt_sidecar(vault_path: &Path) -> Result<[u8; SALT_LEN], VaultError> {
    let path = salt_sidecar_path(vault_path);
    let bytes = std::fs::read(&path)?;
    if bytes.len() != SALT_LEN {
        return Err(VaultError::Migration(format!(
            "salt sidecar at {path:?} has wrong length: got {}, want {SALT_LEN}",
            bytes.len()
        )));
    }
    let mut out = [0u8; SALT_LEN];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn open_or_create_mirror_vault(path: &Path, password: &str) -> Result<Vault, VaultError> {
    if path.exists() {
        Vault::open(path)
    } else {
        Vault::create(path, password)
    }
}

fn acquire_mirror_lock(target_path: &Path) -> Result<MirrorLock, MirrorError> {
    let mut lock_path = target_path.to_path_buf();
    let lock_name = format!(
        "{}.mirror-lock",
        lock_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("vault")
    );
    lock_path.set_file_name(lock_name);

    let started = Instant::now();
    loop {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(file) => {
                set_vault_file_perms(&lock_path);
                return Ok(MirrorLock {
                    path: lock_path,
                    _file: file,
                });
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                if started.elapsed() >= MIRROR_LOCK_TIMEOUT {
                    return Err(MirrorError::LockTimeout);
                }
                thread::sleep(MIRROR_LOCK_POLL);
            }
            Err(err) => return Err(MirrorError::Io(err)),
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::agent_profile::{AgentProfile, AgentProfileStore, AllowedKeyPattern};
    use crate::models::Provider;
    use tempfile::TempDir;

    const PWD: &str = "correct-horse-battery-staple";
    const WRONG_PWD: &str = "wrong-horse-battery-staple";

    fn fresh_vault() -> (TempDir, Vault) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.db");
        let v = Vault::create(&path, PWD).unwrap();
        (dir, v)
    }

    fn add_sample_key(vault: &Vault, token: SessionToken) -> Uuid {
        let input = AddKeyInput {
            provider: Provider::Anthropic,
            label: "primary".to_string(),
            key_value: "sk-ant-test-1111111111111111".to_string(),
            project_tag: Some("nauta".to_string()),
            expires_at: None,
            notes: None,
        };
        vault.add_key(token, input).unwrap().id
    }

    fn add_fake_key(vault: &Vault, token: SessionToken, label: &str, project_tag: &str) -> Uuid {
        let input = AddKeyInput {
            provider: Provider::Generic,
            label: label.to_string(),
            key_value: "sk-test-fake-agent-runtime-000000".to_string(),
            project_tag: Some(project_tag.to_string()),
            expires_at: None,
            notes: Some("fake runtime test key".to_string()),
        };
        vault.add_key(token, input).unwrap().id
    }

    fn add_key_with(
        vault: &Vault,
        token: SessionToken,
        provider: Provider,
        label: &str,
        project_tag: &str,
    ) -> Uuid {
        let input = AddKeyInput {
            provider,
            label: label.to_string(),
            key_value: format!("sk-test-{label}-000000000000"),
            project_tag: Some(project_tag.to_string()),
            expires_at: None,
            notes: None,
        };
        vault.add_key(token, input).unwrap().id
    }

    fn codex_fake_profiles() -> AgentProfileStore {
        AgentProfileStore::new(vec![AgentProfile::new(
            "codex",
            vec![AllowedKeyPattern::new(
                Some(Provider::Generic),
                Some("fake-codex".to_string()),
                Some("fake-*".to_string()),
            )],
        )])
    }

    #[test]
    fn create_then_open_then_unlock() {
        let (_dir, vault) = fresh_vault();
        let token = vault.unlock(PWD).unwrap();
        vault.list_keys(token).unwrap();
    }

    #[test]
    fn create_rejects_short_password() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.db");
        let err = Vault::create(&path, "short").unwrap_err();
        assert!(matches!(err, VaultError::WeakPassword));
    }

    #[test]
    fn create_rejects_existing_path() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.db");
        Vault::create(&path, PWD).unwrap();
        let err = Vault::create(&path, PWD).unwrap_err();
        assert!(matches!(err, VaultError::VaultAlreadyExists));
    }

    #[test]
    fn open_rejects_missing_path() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.db");
        let err = Vault::open(&path).unwrap_err();
        assert!(matches!(err, VaultError::VaultNotFound));
    }

    #[test]
    fn unlock_wrong_password_fails() {
        let (_dir, vault) = fresh_vault();
        let err = vault.unlock(WRONG_PWD).unwrap_err();
        // SQLCipher returns a Db error when key doesn't decrypt — surface as such
        assert!(matches!(err, VaultError::Db(_) | VaultError::BadPassword));
    }

    /// Regression test for V-1 (security review, 2026-04-26):
    /// a wrong-password unlock attempted *after* a prior successful unlock+lock
    /// (i.e., when the SQLCipher connection slot may already be populated)
    /// must still return BadPassword/Db and must NOT issue a session token.
    #[test]
    fn unlock_wrong_password_fails_after_prior_lock() {
        let (_dir, vault) = fresh_vault();

        // Phase 1: legitimate unlock + lock to populate any cached state.
        let token1 = vault.unlock(PWD).unwrap();
        vault.lock(token1).unwrap();

        // Phase 2: wrong password must be rejected, not silently issue a token.
        let err = vault.unlock(WRONG_PWD).unwrap_err();
        assert!(
            matches!(err, VaultError::Db(_) | VaultError::BadPassword),
            "expected BadPassword/Db on wrong re-unlock, got {err:?}"
        );

        // Phase 3: correct password still works after the failed attempt.
        let token2 = vault.unlock(PWD).unwrap();
        // And the session is actually usable (proves the AES key is correct).
        vault.list_keys(token2).unwrap();
    }

    #[test]
    fn add_get_roundtrips() {
        let (_dir, vault) = fresh_vault();
        let token = vault.unlock(PWD).unwrap();
        let id = add_sample_key(&vault, token);
        let secret = vault.get_key_value(token, id).unwrap();
        assert_eq!(secret.expose_secret(), "sk-ant-test-1111111111111111");
    }

    #[test]
    fn list_returns_metadata_without_plaintext() {
        let (_dir, vault) = fresh_vault();
        let token = vault.unlock(PWD).unwrap();
        add_sample_key(&vault, token);
        let metas = vault.list_keys(token).unwrap();
        assert_eq!(metas.len(), 1);
        // Dump via Debug — should never contain plaintext key value
        let dbg = format!("{:?}", &metas[0]);
        assert!(
            !dbg.contains("sk-ant"),
            "metadata Debug leaked plaintext: {dbg}"
        );
        assert!(
            !dbg.contains("1111"),
            "metadata Debug leaked plaintext: {dbg}"
        );
    }

    #[test]
    fn delete_removes_key() {
        let (_dir, vault) = fresh_vault();
        let token = vault.unlock(PWD).unwrap();
        let id = add_sample_key(&vault, token);
        vault.delete_key(token, id).unwrap();
        let err = vault.get_key_value(token, id).unwrap_err();
        assert!(matches!(err, VaultError::KeyNotFound(_)));
    }

    #[test]
    fn audit_event_round_trips_after_reopen() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.db");
        let vault = Vault::create(&path, PWD).unwrap();
        let token = vault.unlock(PWD).unwrap();

        let first = add_key_with(&vault, token, Provider::Anthropic, "first", "acct-a");
        let second = add_key_with(&vault, token, Provider::OpenAI, "second", "acct-b");
        let third = add_key_with(&vault, token, Provider::Stripe, "third", "acct-c");
        vault.delete_key(token, third).unwrap();
        vault.mark_superseded(first, second).unwrap();

        let events = vault.audit_events().unwrap();
        assert_eq!(events.len(), 5);
        assert_eq!(
            events.iter().map(|event| event.kind).collect::<Vec<_>>(),
            vec![
                EventKind::Add,
                EventKind::Add,
                EventKind::Add,
                EventKind::Delete,
                EventKind::Supersede,
            ]
        );
        drop(vault);

        let reopened = Vault::open(&path).unwrap();
        let _token = reopened.unlock(PWD).unwrap();
        let reopened_events = reopened.audit_events().unwrap();
        assert_eq!(reopened_events.len(), 5);
        assert_eq!(reopened_events[0].entry_id, first);
        assert_eq!(reopened_events[4].superseded_by, Some(second));
    }

    #[test]
    fn supersede_preserves_old_entry() {
        let (_dir, vault) = fresh_vault();
        let token = vault.unlock(PWD).unwrap();
        let old = add_key_with(&vault, token, Provider::GitHub, "old", "acct");
        let new = add_key_with(&vault, token, Provider::GitHub, "new", "acct");

        vault.mark_superseded(old, new).unwrap();
        let metas = vault.list_keys(token).unwrap();
        let old_meta = metas.iter().find(|meta| meta.id == old).unwrap();
        let new_meta = metas.iter().find(|meta| meta.id == new).unwrap();
        assert_eq!(old_meta.superseded_by, Some(new));
        assert_eq!(new_meta.superseded_by, None);
    }

    #[test]
    fn supersede_missing_entry_returns_entry_not_found() {
        let (_dir, vault) = fresh_vault();
        let token = vault.unlock(PWD).unwrap();
        let existing = add_sample_key(&vault, token);
        let bogus = Uuid::new_v4();

        let err = vault.mark_superseded(bogus, existing).unwrap_err();
        assert!(matches!(err, VaultError::EntryNotFound(id) if id == bogus));
    }

    #[test]
    fn backward_compat_empty_audit_log_defaults_to_no_events() {
        let (_dir, vault) = fresh_vault();
        let _token = vault.unlock(PWD).unwrap();
        assert!(vault.audit_events().unwrap().is_empty());
    }

    #[test]
    fn lock_invalidates_token() {
        let (_dir, vault) = fresh_vault();
        let token = vault.unlock(PWD).unwrap();
        let id = add_sample_key(&vault, token);
        vault.lock(token).unwrap();
        let err = vault.get_key_value(token, id).unwrap_err();
        assert!(matches!(err, VaultError::InvalidSession));
        let err2 = vault.list_keys(token).unwrap_err();
        assert!(matches!(err2, VaultError::InvalidSession));
    }

    #[test]
    fn add_without_session_fails() {
        let (_dir, vault) = fresh_vault();
        // Use a token never issued by this vault
        let bogus = SessionToken::new();
        let input = AddKeyInput {
            provider: Provider::Anthropic,
            label: "primary".to_string(),
            key_value: "sk-ant-test-1111".to_string(),
            project_tag: None,
            expires_at: None,
            notes: None,
        };
        let err = vault.add_key(bogus, input).unwrap_err();
        assert!(matches!(err, VaultError::InvalidSession));
    }

    #[test]
    fn full_lifecycle_create_unlock_add_get_lock_unlock_get() {
        let (_dir, vault) = fresh_vault();

        // Phase 1: unlock + add + get + lock
        let token1 = vault.unlock(PWD).unwrap();
        let id = add_sample_key(&vault, token1);
        let v1 = vault.get_key_value(token1, id).unwrap();
        assert_eq!(v1.expose_secret(), "sk-ant-test-1111111111111111");
        vault.lock(token1).unwrap();

        // Old token now invalid
        let err = vault.list_keys(token1).unwrap_err();
        assert!(matches!(err, VaultError::InvalidSession));

        // Phase 2: re-unlock with same password, retrieve same key
        let token2 = vault.unlock(PWD).unwrap();
        assert_ne!(token1, token2);
        let v2 = vault.get_key_value(token2, id).unwrap();
        assert_eq!(v2.expose_secret(), "sk-ant-test-1111111111111111");

        // Listing also still shows the persisted key
        let metas = vault.list_keys(token2).unwrap();
        assert_eq!(metas.len(), 1);
        assert_eq!(metas[0].id, id);
    }

    #[test]
    fn fetch_key_for_agent_allows_matching_fake_key_and_audits() {
        let (dir, vault) = fresh_vault();
        let token = vault.unlock(PWD).unwrap();
        let id = add_fake_key(&vault, token, "fake-openai-smoke", "fake-codex");
        let audit_path = dir.path().join("audit").join("fetch-events.jsonl");
        let audit = AuditLogger::new(&audit_path);

        let secret = vault
            .fetch_key_for_agent(token, "codex", id, &codex_fake_profiles(), &audit)
            .unwrap();
        assert_eq!(secret.expose_secret(), "sk-test-fake-agent-runtime-000000");

        let audit_text = std::fs::read_to_string(audit_path).unwrap();
        assert!(audit_text.contains("\"agent_id\":\"codex\""));
        assert!(audit_text.contains("\"outcome\":\"allowed\""));
        assert!(audit_text.contains("\"label\":\"fake-openai-smoke\""));
        assert!(
            !audit_text.contains("sk-test-fake-agent-runtime"),
            "audit log must not contain plaintext key"
        );
    }

    #[test]
    fn fetch_key_for_agent_denies_wrong_agent_and_audits_without_decrypting() {
        let (dir, vault) = fresh_vault();
        let token = vault.unlock(PWD).unwrap();
        let id = add_fake_key(&vault, token, "fake-openai-smoke", "fake-codex");
        let audit_path = dir.path().join("audit").join("fetch-events.jsonl");
        let audit = AuditLogger::new(&audit_path);

        let err = vault
            .fetch_key_for_agent(token, "aliza", id, &codex_fake_profiles(), &audit)
            .unwrap_err();
        assert!(matches!(err, VaultError::AccessDenied));

        let audit_text = std::fs::read_to_string(audit_path).unwrap();
        assert!(audit_text.contains("\"agent_id\":\"aliza\""));
        assert!(audit_text.contains("\"outcome\":\"denied\""));
        assert!(audit_text.contains("agent_profile_denied"));
        assert!(
            !audit_text.contains("sk-test-fake-agent-runtime"),
            "denied audit log must not contain plaintext key"
        );
    }

    #[test]
    fn salt_sidecar_path_uses_sibling_file() {
        let p = Path::new("/tmp/holster/vault.db");
        let salt_p = salt_sidecar_path(p);
        assert_eq!(salt_p, Path::new("/tmp/holster/vault.db.salt"));
    }

    /// Regression test for V-4 (security review, 2026-04-26):
    /// the vault DB file must be 0600 (owner rw only) after `Vault::create`,
    /// not the default 0644 inherited from the typical macOS umask.
    /// The salt sidecar must also be 0600 (existing invariant — pinned here).
    #[cfg(unix)]
    #[test]
    fn create_sets_vault_file_mode_0600() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.db");
        let _vault = Vault::create(&path, PWD).unwrap();

        let db_mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            db_mode, 0o600,
            "vault db file should be 0600, was {db_mode:o}"
        );

        let salt_path = salt_sidecar_path(&path);
        let salt_mode = std::fs::metadata(&salt_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            salt_mode, 0o600,
            "salt sidecar should be 0600, was {salt_mode:o}"
        );
    }

    // ── v0.2.0 / v0.7.0 — rotate_master tests ─────────────────────────────

    #[test]
    fn rotate_master_happy_path_preserves_all_values() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("vault.db");
        let vault = Vault::create(&path, PWD).unwrap();
        let token = vault.unlock(PWD).unwrap();

        // Add three entries with distinct values
        let id_a = add_key_with(&vault, token, Provider::Anthropic, "anth-1", "sk-anth-A");
        let id_b = add_key_with(&vault, token, Provider::OpenAI, "openai-1", "sk-openai-B");
        let id_c = add_key_with(&vault, token, Provider::Stripe, "stripe-1", "sk-stripe-C");

        // Capture original plaintexts for byte-perfect comparison after rotate
        let val_a_before = vault.get_key_value(token, id_a).unwrap();
        let val_b_before = vault.get_key_value(token, id_b).unwrap();
        let val_c_before = vault.get_key_value(token, id_c).unwrap();
        vault.lock(token).ok();

        // Rotate
        let new_pw = "Newpass-789!";
        let count = vault.rotate_master(PWD, new_pw).unwrap();
        assert_eq!(count, 3, "expected 3 entries re-encrypted, got {count}");

        // Old password must FAIL
        let old_unlock_err = vault.unlock(PWD).unwrap_err();
        // SQLCipher wrong-key error path: returns `Db(SqliteFailure(NotADatabase))`
        // because the rekeyed file no longer decrypts under the old key, so SQLite
        // sees garbage instead of a valid header. Accept all three "wrong-pw"
        // error variants depending on which layer detects first.
        assert!(
            matches!(
                old_unlock_err,
                VaultError::BadPassword | VaultError::Crypto(_) | VaultError::Db(_)
            ),
            "unlock with old pw should fail, got: {old_unlock_err:?}"
        );

        // New password must succeed AND return byte-identical plaintexts
        let new_token = vault.unlock(new_pw).unwrap();
        let val_a_after = vault.get_key_value(new_token, id_a).unwrap();
        let val_b_after = vault.get_key_value(new_token, id_b).unwrap();
        let val_c_after = vault.get_key_value(new_token, id_c).unwrap();
        assert_eq!(val_a_after.expose_secret(), val_a_before.expose_secret());
        assert_eq!(val_b_after.expose_secret(), val_b_before.expose_secret());
        assert_eq!(val_c_after.expose_secret(), val_c_before.expose_secret());
    }

    #[test]
    fn rotate_master_rejects_wrong_old_password() {
        let (_dir, vault) = fresh_vault();
        let err = vault.rotate_master("wrong-old-password", "Newpass-789!").unwrap_err();
        // See note in rotate_master_happy_path: SQLCipher rejects wrong keys
        // with NotADatabase at the rusqlite layer; that's wrapped in VaultError::Db.
        assert!(
            matches!(
                err,
                VaultError::BadPassword | VaultError::Crypto(_) | VaultError::Db(_)
            ),
            "expected wrong-password failure, got {err:?}"
        );
    }

    #[test]
    fn rotate_master_rejects_weak_new_password() {
        let (_dir, vault) = fresh_vault();
        let err = vault.rotate_master(PWD, "short").unwrap_err();
        assert!(
            matches!(err, VaultError::WeakPassword),
            "expected WeakPassword, got {err:?}"
        );
    }

    #[test]
    fn rotate_master_writes_audit_event() {
        let (_dir, vault) = fresh_vault();
        let token = vault.unlock(PWD).unwrap();
        add_sample_key(&vault, token);
        add_sample_key(&vault, token);
        vault.lock(token).ok();

        vault.rotate_master(PWD, "Newpass-789!").unwrap();

        let new_token = vault.unlock("Newpass-789!").unwrap();
        let events = vault.audit_events().unwrap();
        let rotate_events: Vec<_> = events
            .iter()
            .filter(|e| e.kind == EventKind::MasterRotated)
            .collect();
        assert_eq!(rotate_events.len(), 1, "expected 1 master_rotated event");
        let evt = rotate_events[0];
        assert_eq!(evt.entry_id, Uuid::nil());
        assert!(
            evt.label.as_deref().unwrap_or("").contains("2 entries"),
            "audit label should mention entry count, got: {:?}",
            evt.label
        );
        vault.lock(new_token).ok();
    }

    #[test]
    fn rotate_master_regenerates_salt() {
        let (dir, vault) = fresh_vault();
        let salt_path_buf = dir.path().join("vault.db.salt");
        let salt_before = std::fs::read(&salt_path_buf).unwrap();

        vault.rotate_master(PWD, "Newpass-789!").unwrap();

        let salt_after = std::fs::read(&salt_path_buf).unwrap();
        assert_eq!(salt_before.len(), 16);
        assert_eq!(salt_after.len(), 16);
        assert_ne!(
            salt_before, salt_after,
            "salt sidecar should be regenerated on rotation"
        );
    }

    #[test]
    fn rotate_master_empty_vault_succeeds() {
        let (_dir, vault) = fresh_vault();
        let count = vault.rotate_master(PWD, "Newpass-789!").unwrap();
        assert_eq!(count, 0, "empty vault rotation should report 0 entries");

        // Confirm new password unlocks even with no entries
        vault.unlock("Newpass-789!").unwrap();
    }

    #[test]
    fn rotate_master_can_be_rotated_again() {
        let (_dir, vault) = fresh_vault();
        let token = vault.unlock(PWD).unwrap();
        let id = add_sample_key(&vault, token);
        vault.lock(token).ok();

        vault.rotate_master(PWD, "Second-pass!1").unwrap();
        vault.rotate_master("Second-pass!1", "Third-pass!22").unwrap();

        let token = vault.unlock("Third-pass!22").unwrap();
        let value = vault.get_key_value(token, id).unwrap();
        assert_eq!(value.expose_secret(), "sk-ant-test-1111111111111111");
    }
}
