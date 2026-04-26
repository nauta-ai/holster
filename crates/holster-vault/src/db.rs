//! SQLCipher-backed storage layer.
//!
//! T1.5: encrypted SQLite via the bundled SQLCipher build of rusqlite.
//!
//! Key design points:
//! - `Connection` is `!Sync`, so we wrap it in `Mutex<Connection>`. All public
//!   methods take `&self` and acquire the lock briefly.
//! - PRAGMA key uses hex format `"x\'...\'"` — the only SQL we string-format
//!   ourselves, because PRAGMA values cannot be parameterized. The input is
//!   a fixed-length 32-byte key from `derive_keys`, so there is no injection
//!   vector. All other queries are `?`-parameterized.
//! - Schema bootstraps on first open. Future migrations land here.

use std::path::Path;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::error::VaultError;
use crate::models::{KeyMetadata, KeyStatus, Provider};

const SCHEMA_VERSION: i64 = 1;

const INIT_DDL: &str = "CREATE TABLE IF NOT EXISTS vault_meta (
    schema_version INTEGER NOT NULL,
    salt BLOB NOT NULL,
    created_at TEXT NOT NULL,
    auto_lock_minutes INTEGER NOT NULL DEFAULT 15
);

CREATE TABLE IF NOT EXISTS keys (
    id TEXT PRIMARY KEY,
    provider TEXT NOT NULL,
    label TEXT NOT NULL,
    project_tag TEXT,
    key_ciphertext BLOB NOT NULL,
    key_nonce BLOB NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT,
    last_rotated_at TEXT,
    last_used_at TEXT,
    notes TEXT,
    revoked INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_keys_provider ON keys(provider);
CREATE INDEX IF NOT EXISTS idx_keys_project_tag ON keys(project_tag);
CREATE INDEX IF NOT EXISTS idx_keys_expires_at ON keys(expires_at);

CREATE TABLE IF NOT EXISTS usage_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key_id TEXT NOT NULL,
    fetched_at TEXT NOT NULL,
    period_start TEXT NOT NULL,
    period_end TEXT NOT NULL,
    input_tokens INTEGER,
    output_tokens INTEGER,
    cost_usd_cents INTEGER,
    raw_response_json TEXT,
    FOREIGN KEY (key_id) REFERENCES keys(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_usage_key ON usage_snapshots(key_id, fetched_at);

CREATE TABLE IF NOT EXISTS leak_scan_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scanned_at TEXT NOT NULL,
    repo_path TEXT NOT NULL,
    matches_found INTEGER NOT NULL,
    matches_json TEXT NOT NULL
);
";

// ── Public Database wrapper ──────────────────────────────────────────────────

pub struct Database {
    conn: Mutex<Connection>,
}

/// A row from the `keys` table — includes ciphertext + nonce. Only consumed
/// by the vault facade when actually decrypting; never returned to UI.
///
/// `Debug` is hand-rolled to redact the ciphertext + nonce so a stray log
/// or panic never leaks crypto material. Do NOT `#[derive(Debug)]`.
pub struct KeyRecord {
    pub metadata: KeyMetadata,
    pub key_ciphertext: Vec<u8>,
    pub key_nonce: [u8; 12],
}

impl std::fmt::Debug for KeyRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyRecord")
            .field("metadata", &self.metadata)
            .field(
                "key_ciphertext",
                &format_args!("<{} bytes redacted>", self.key_ciphertext.len()),
            )
            .field("key_nonce", &"<redacted>")
            .finish()
    }
}

/// Inputs for inserting a new key. Already-encrypted ciphertext + nonce —
/// the plaintext path is in `vault.rs` calling `crypto::encrypt_key_value`.
pub struct InsertKeyParams {
    pub id: Uuid,
    pub provider: Provider,
    pub label: String,
    pub project_tag: Option<String>,
    pub key_ciphertext: Vec<u8>,
    pub key_nonce: [u8; 12],
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

impl Database {
    /// Open an encrypted vault at `path`. Creates and migrates schema if missing.
    pub fn open(path: &Path, sqlcipher_key: &[u8; 32]) -> Result<Self, VaultError> {
        let conn = Connection::open(path)?;
        Self::unlock_and_migrate(conn, sqlcipher_key)
    }

    /// In-memory database for tests. Same code path; the file just lives in RAM.
    pub fn open_in_memory(sqlcipher_key: &[u8; 32]) -> Result<Self, VaultError> {
        let conn = Connection::open_in_memory()?;
        Self::unlock_and_migrate(conn, sqlcipher_key)
    }

    fn unlock_and_migrate(conn: Connection, sqlcipher_key: &[u8; 32]) -> Result<Self, VaultError> {
        // PRAGMA key — only place we format SQL. Hex encoding is a fixed
        // 64-char string from a 32-byte fixed-length input → no injection vector.
        let hex_key: String = sqlcipher_key.iter().map(|b| format!("{b:02x}")).collect();
        let pragma = format!("PRAGMA key = \"x\'{hex_key}\'\"");
        conn.execute_batch(&pragma)?;
        // Foreign keys are off by default in SQLite — turn on so usage_snapshots cascade works.
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        let db = Database {
            conn: Mutex::new(conn),
        };
        db.run_migrations()?;
        Ok(db)
    }

    fn run_migrations(&self) -> Result<(), VaultError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| VaultError::Crypto("db mutex poisoned during migration".into()))?;
        conn.execute_batch(INIT_DDL)?;
        // Bootstrap vault_meta if empty
        let meta_row: Option<i64> = conn
            .query_row("SELECT schema_version FROM vault_meta LIMIT 1", [], |r| {
                r.get(0)
            })
            .optional()?;
        if meta_row.is_none() {
            conn.execute(
                "INSERT INTO vault_meta (schema_version, salt, created_at, auto_lock_minutes)
                 VALUES (?1, ?2, ?3, 15)",
                params![SCHEMA_VERSION, &[0u8; 16][..], Utc::now().to_rfc3339()],
            )?;
        }
        Ok(())
    }

    /// Update the salt stored in vault_meta. Called once during vault creation
    /// (after generating the salt for Argon2).
    pub fn set_salt(&self, salt: &[u8; 16]) -> Result<(), VaultError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| VaultError::Crypto("db mutex poisoned".into()))?;
        conn.execute("UPDATE vault_meta SET salt = ?1", params![&salt[..]])?;
        Ok(())
    }

    /// Read the stored salt for Argon2 re-derivation on unlock.
    pub fn get_salt(&self) -> Result<[u8; 16], VaultError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| VaultError::Crypto("db mutex poisoned".into()))?;
        let blob: Vec<u8> =
            conn.query_row("SELECT salt FROM vault_meta LIMIT 1", [], |r| r.get(0))?;
        if blob.len() != 16 {
            return Err(VaultError::Migration(format!(
                "salt has wrong length: got {}, want 16",
                blob.len()
            )));
        }
        let mut out = [0u8; 16];
        out.copy_from_slice(&blob);
        Ok(out)
    }

    // ── Key CRUD ─────────────────────────────────────────────────────────────-

    pub fn insert_key(&self, p: InsertKeyParams) -> Result<KeyMetadata, VaultError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| VaultError::Crypto("db mutex poisoned".into()))?;
        conn.execute(
            "INSERT INTO keys (
                id, provider, label, project_tag,
                key_ciphertext, key_nonce,
                created_at, expires_at, last_rotated_at, last_used_at, notes, revoked
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, NULL, ?9, 0)",
            params![
                p.id.to_string(),
                p.provider.as_str(),
                p.label,
                p.project_tag,
                p.key_ciphertext,
                &p.key_nonce[..],
                p.created_at.to_rfc3339(),
                p.expires_at.map(|t| t.to_rfc3339()),
                p.notes,
            ],
        )?;
        Ok(KeyMetadata {
            id: p.id,
            provider: p.provider,
            label: p.label,
            project_tag: p.project_tag,
            created_at: p.created_at,
            expires_at: p.expires_at,
            last_rotated_at: None,
            last_used_at: None,
            status: KeyStatus::Active,
            notes: p.notes,
            key_format_valid: true,
        })
    }

    pub fn select_key_by_id(&self, id: Uuid) -> Result<KeyRecord, VaultError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| VaultError::Crypto("db mutex poisoned".into()))?;
        let row = conn
            .query_row(
                "SELECT provider, label, project_tag, key_ciphertext, key_nonce,
                    created_at, expires_at, last_rotated_at, last_used_at, notes, revoked
             FROM keys WHERE id = ?1",
                params![id.to_string()],
                |row| {
                    let provider: String = row.get(0)?;
                    let label: String = row.get(1)?;
                    let project_tag: Option<String> = row.get(2)?;
                    let ct: Vec<u8> = row.get(3)?;
                    let nonce_blob: Vec<u8> = row.get(4)?;
                    let created_at: String = row.get(5)?;
                    let expires_at: Option<String> = row.get(6)?;
                    let last_rotated_at: Option<String> = row.get(7)?;
                    let last_used_at: Option<String> = row.get(8)?;
                    let notes: Option<String> = row.get(9)?;
                    let revoked: i64 = row.get(10)?;
                    Ok((
                        provider,
                        label,
                        project_tag,
                        ct,
                        nonce_blob,
                        created_at,
                        expires_at,
                        last_rotated_at,
                        last_used_at,
                        notes,
                        revoked,
                    ))
                },
            )
            .optional()?;

        let row = row.ok_or(VaultError::KeyNotFound(id))?;
        let (
            provider_s,
            label,
            project_tag,
            ct,
            nonce_blob,
            created_s,
            expires_s,
            rotated_s,
            used_s,
            notes,
            revoked,
        ) = row;

        let provider = Provider::from_str(&provider_s).ok_or_else(|| {
            VaultError::Migration(format!("unknown provider in db: {provider_s}"))
        })?;
        if nonce_blob.len() != 12 {
            return Err(VaultError::Crypto(format!(
                "nonce has wrong length: got {}, want 12",
                nonce_blob.len()
            )));
        }
        let mut nonce = [0u8; 12];
        nonce.copy_from_slice(&nonce_blob);

        let metadata = KeyMetadata {
            id,
            provider,
            label,
            project_tag,
            created_at: parse_iso(&created_s)?,
            expires_at: expires_s.as_deref().map(parse_iso).transpose()?,
            last_rotated_at: rotated_s.as_deref().map(parse_iso).transpose()?,
            last_used_at: used_s.as_deref().map(parse_iso).transpose()?,
            status: if revoked != 0 {
                KeyStatus::Revoked
            } else {
                KeyStatus::Active
            },
            notes,
            key_format_valid: true,
        };
        Ok(KeyRecord {
            metadata,
            key_ciphertext: ct,
            key_nonce: nonce,
        })
    }

    pub fn select_all_metadata(&self) -> Result<Vec<KeyMetadata>, VaultError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| VaultError::Crypto("db mutex poisoned".into()))?;
        let mut stmt = conn.prepare(
            "SELECT id, provider, label, project_tag, created_at, expires_at,
                    last_rotated_at, last_used_at, notes, revoked
             FROM keys ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let id_s: String = row.get(0)?;
            let provider_s: String = row.get(1)?;
            let label: String = row.get(2)?;
            let project_tag: Option<String> = row.get(3)?;
            let created_s: String = row.get(4)?;
            let expires_s: Option<String> = row.get(5)?;
            let rotated_s: Option<String> = row.get(6)?;
            let used_s: Option<String> = row.get(7)?;
            let notes: Option<String> = row.get(8)?;
            let revoked: i64 = row.get(9)?;
            Ok((
                id_s,
                provider_s,
                label,
                project_tag,
                created_s,
                expires_s,
                rotated_s,
                used_s,
                notes,
                revoked,
            ))
        })?;

        let mut out = Vec::new();
        for r in rows {
            let (
                id_s,
                provider_s,
                label,
                project_tag,
                created_s,
                expires_s,
                rotated_s,
                used_s,
                notes,
                revoked,
            ) = r?;
            let id = Uuid::parse_str(&id_s)
                .map_err(|e| VaultError::Migration(format!("bad UUID in db: {e}")))?;
            let provider = Provider::from_str(&provider_s)
                .ok_or_else(|| VaultError::Migration(format!("unknown provider: {provider_s}")))?;
            out.push(KeyMetadata {
                id,
                provider,
                label,
                project_tag,
                created_at: parse_iso(&created_s)?,
                expires_at: expires_s.as_deref().map(parse_iso).transpose()?,
                last_rotated_at: rotated_s.as_deref().map(parse_iso).transpose()?,
                last_used_at: used_s.as_deref().map(parse_iso).transpose()?,
                status: if revoked != 0 {
                    KeyStatus::Revoked
                } else {
                    KeyStatus::Active
                },
                notes,
                key_format_valid: true,
            });
        }
        Ok(out)
    }

    pub fn update_last_used(&self, id: Uuid, ts: DateTime<Utc>) -> Result<(), VaultError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| VaultError::Crypto("db mutex poisoned".into()))?;
        let n = conn.execute(
            "UPDATE keys SET last_used_at = ?1 WHERE id = ?2",
            params![ts.to_rfc3339(), id.to_string()],
        )?;
        if n == 0 {
            return Err(VaultError::KeyNotFound(id));
        }
        Ok(())
    }

    pub fn delete_key(&self, id: Uuid) -> Result<(), VaultError> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| VaultError::Crypto("db mutex poisoned".into()))?;
        let n = conn.execute("DELETE FROM keys WHERE id = ?1", params![id.to_string()])?;
        if n == 0 {
            return Err(VaultError::KeyNotFound(id));
        }
        Ok(())
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn parse_iso(s: &str) -> Result<DateTime<Utc>, VaultError> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| VaultError::Migration(format!("bad timestamp {s:?}: {e}")))
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn fresh_db() -> Database {
        let key = [7u8; 32];
        Database::open_in_memory(&key).expect("open_in_memory")
    }

    fn sample_insert(label: &str) -> InsertKeyParams {
        InsertKeyParams {
            id: Uuid::new_v4(),
            provider: Provider::Anthropic,
            label: label.to_string(),
            project_tag: Some("nauta".to_string()),
            key_ciphertext: vec![0xAB; 64],
            key_nonce: [0xCD; 12],
            created_at: Utc.with_ymd_and_hms(2026, 4, 25, 12, 0, 0).unwrap(),
            expires_at: None,
            notes: None,
        }
    }

    #[test]
    fn open_and_migrate() {
        let db = fresh_db();
        let salt = db.get_salt().unwrap();
        // Bootstrapped to all-zeros until set_salt is called
        assert_eq!(salt, [0u8; 16]);
    }

    #[test]
    fn set_and_get_salt() {
        let db = fresh_db();
        let new_salt = [42u8; 16];
        db.set_salt(&new_salt).unwrap();
        assert_eq!(db.get_salt().unwrap(), new_salt);
    }

    #[test]
    fn insert_then_select_by_id_roundtrips() {
        let db = fresh_db();
        let params = sample_insert("primary");
        let id = params.id;
        db.insert_key(params).unwrap();
        let rec = db.select_key_by_id(id).unwrap();
        assert_eq!(rec.metadata.id, id);
        assert_eq!(rec.metadata.label, "primary");
        assert_eq!(rec.metadata.provider, Provider::Anthropic);
        assert_eq!(rec.key_ciphertext, vec![0xAB; 64]);
        assert_eq!(rec.key_nonce, [0xCD; 12]);
    }

    #[test]
    fn select_missing_returns_key_not_found() {
        let db = fresh_db();
        let bogus = Uuid::new_v4();
        let err = db.select_key_by_id(bogus).unwrap_err();
        assert!(matches!(err, VaultError::KeyNotFound(id) if id == bogus));
    }

    #[test]
    fn select_all_metadata_lists_all_inserted() {
        let db = fresh_db();
        for i in 0..3 {
            db.insert_key(sample_insert(&format!("key_{i}"))).unwrap();
        }
        let all = db.select_all_metadata().unwrap();
        assert_eq!(all.len(), 3);
        // Critical: ensure no plaintext leaks via metadata path
        for m in &all {
            let dbg = format!("{m:?}");
            assert!(
                !dbg.contains("AB"),
                "metadata Debug should never contain ciphertext bytes"
            );
        }
    }

    #[test]
    fn update_last_used_persists() {
        let db = fresh_db();
        let params = sample_insert("primary");
        let id = params.id;
        db.insert_key(params).unwrap();
        let ts = Utc.with_ymd_and_hms(2026, 4, 26, 9, 0, 0).unwrap();
        db.update_last_used(id, ts).unwrap();
        let rec = db.select_key_by_id(id).unwrap();
        assert_eq!(rec.metadata.last_used_at, Some(ts));
    }

    #[test]
    fn update_last_used_missing_id_errors() {
        let db = fresh_db();
        let bogus = Uuid::new_v4();
        let err = db.update_last_used(bogus, Utc::now()).unwrap_err();
        assert!(matches!(err, VaultError::KeyNotFound(_)));
    }

    #[test]
    fn delete_removes_row() {
        let db = fresh_db();
        let params = sample_insert("primary");
        let id = params.id;
        db.insert_key(params).unwrap();
        db.delete_key(id).unwrap();
        let err = db.select_key_by_id(id).unwrap_err();
        assert!(matches!(err, VaultError::KeyNotFound(_)));
    }

    #[test]
    fn delete_missing_errors() {
        let db = fresh_db();
        let err = db.delete_key(Uuid::new_v4()).unwrap_err();
        assert!(matches!(err, VaultError::KeyNotFound(_)));
    }

    #[test]
    fn insert_with_duplicate_id_fails() {
        let db = fresh_db();
        let params = sample_insert("primary");
        let id = params.id;
        db.insert_key(params).unwrap();

        let dupe = InsertKeyParams {
            id,
            provider: Provider::OpenAI,
            label: "second".to_string(),
            project_tag: None,
            key_ciphertext: vec![0; 64],
            key_nonce: [0; 12],
            created_at: Utc::now(),
            expires_at: None,
            notes: None,
        };
        let err = db.insert_key(dupe).unwrap_err();
        // Surface as Db error; constraint violation
        assert!(matches!(err, VaultError::Db(_)));
    }

    #[test]
    fn parameterized_query_resists_injection_in_label() {
        // Demonstrates that string args don't break out of parameterization.
        let db = fresh_db();
        let mut params = sample_insert("normal");
        let id = params.id;
        params.label = "); DROP TABLE keys; --".to_string();
        db.insert_key(params).unwrap();
        // If injection had worked, the keys table would be gone. Verify it isn't:
        let rec = db.select_key_by_id(id).unwrap();
        assert_eq!(rec.metadata.label, "); DROP TABLE keys; --");
        // And we can still list:
        let all = db.select_all_metadata().unwrap();
        assert_eq!(all.len(), 1);
    }
}
