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

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::Utc;
use secrecy::{ExposeSecret, Secret};
use uuid::Uuid;

use crate::crypto::{decrypt_key_value, derive_keys, encrypt_key_value, generate_salt};
use crate::db::{Database, InsertKeyParams};
use crate::error::VaultError;
use crate::models::{AddKeyInput, KeyMetadata};
use crate::session::{SessionStore, SessionToken};

const SALT_LEN: usize = 16;
const MIN_PASSWORD_LEN: usize = 8;

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
        let aes_key = self.aes_key_for(token)?;
        let (ciphertext, nonce) = encrypt_key_value(&aes_key, &input.key_value)?;

        let id = Uuid::new_v4();
        let now = Utc::now();
        let params = InsertKeyParams {
            id,
            provider: input.provider,
            label: input.label,
            project_tag: input.project_tag,
            key_ciphertext: ciphertext,
            key_nonce: nonce,
            created_at: now,
            expires_at: input.expires_at,
            notes: input.notes,
        };
        self.with_db(|db| db.insert_key(params))
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
        self.with_db(|db| db.delete_key(id))?;
        let _ = self.sessions.touch(token);
        Ok(())
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

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
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
    fn salt_sidecar_path_uses_sibling_file() {
        let p = Path::new("/tmp/holster/vault.db");
        let salt_p = salt_sidecar_path(p);
        assert_eq!(salt_p, Path::new("/tmp/holster/vault.db.salt"));
    }
}
