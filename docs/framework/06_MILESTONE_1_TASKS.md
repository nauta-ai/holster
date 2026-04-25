# 06 — Milestone 1 Tasks (Vault Foundation)

This is the first milestone. Every other milestone depends on this being right.

**Estimated effort:** 12-18 hours of focused build time.
**Branch:** `milestone/M1-vault-foundation` off `dev`.
**Reviewer:** CC must sign off before merge to `dev`.

---

## T1.0 — Repo scaffolding (1h)

```bash
# On .203, in Dave's home or a workspace dir:
mkdir -p ~/holster && cd ~/holster
git init
git checkout -b dev

# Monorepo skeleton
cat > pnpm-workspace.yaml <<EOF
packages:
  - "apps/*"
EOF

mkdir -p apps/desktop apps/cli crates/holster-vault docs tests scripts

# Initialize Cargo workspace at root
cat > Cargo.toml <<EOF
[workspace]
resolver = "2"
members = [
    "apps/desktop/src-tauri",
    "apps/cli",
    "crates/holster-vault",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
authors = ["Dave Nauta <dave@nautaai.com>"]
license = "Source-Available"
repository = "private"

[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1", features = ["full"] }
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.10", features = ["v4", "serde"] }
thiserror = "1.0"
anyhow = "1.0"
tracing = "0.1"
zeroize = { version = "1.7", features = ["derive"] }
secrecy = { version = "0.8", features = ["serde"] }
EOF

# .gitignore
cat > .gitignore <<EOF
target/
dist/
node_modules/
*.db
*.db-journal
*.db-shm
*.db-wal
.env
.env.local
.DS_Store
*.log
logs/
.vscode/
.idea/
EOF

git add . && git commit -m "chore: initial repo scaffold"
git checkout -b milestone/M1-vault-foundation
```

**Acceptance:** `cargo check --workspace` passes (will be empty but should not error).

---

## T1.1 — `holster-vault` crate skeleton (1h)

Create `crates/holster-vault/Cargo.toml`:

```toml
[package]
name = "holster-vault"
version.workspace = true
edition.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
chrono.workspace = true
uuid.workspace = true
thiserror.workspace = true
tracing.workspace = true
zeroize.workspace = true
secrecy.workspace = true
rusqlite = { version = "0.31", features = ["bundled-sqlcipher"] }
argon2 = "0.5"
aes-gcm = "0.10"
rand = "0.8"
hkdf = "0.12"
sha2 = "0.10"

[dev-dependencies]
tempfile = "3.10"
proptest = "1.5"
```

Create `crates/holster-vault/src/lib.rs`:

```rust
//! Holster vault: encrypted key storage.
//!
//! All public types and functions guard against accidental leakage of
//! plaintext key material via `Debug`, logging, or serialization.

#![warn(clippy::all)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
#![warn(clippy::panic)]

pub mod crypto;
pub mod db;
pub mod error;
pub mod models;
pub mod session;
pub mod vault;

pub use error::VaultError;
pub use models::{KeyMetadata, KeyStatus, Provider};
pub use session::SessionToken;
pub use vault::Vault;
```

**Acceptance:** `cargo check -p holster-vault` succeeds.

---

## T1.2 — Error types (30m)

Create `crates/holster-vault/src/error.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("vault not unlocked")]
    Locked,

    #[error("invalid session token")]
    InvalidSession,

    #[error("session expired (idle timeout)")]
    SessionExpired,

    #[error("incorrect master password")]
    BadPassword,

    #[error("master password too weak (zxcvbn score < 3)")]
    WeakPassword,

    #[error("vault file not found")]
    VaultNotFound,

    #[error("vault already exists")]
    VaultAlreadyExists,

    #[error("key not found: {0}")]
    KeyNotFound(uuid::Uuid),

    #[error("schema migration failed: {0}")]
    Migration(String),

    #[error("crypto error: {0}")]
    Crypto(String),

    #[error("database error")]
    Db(#[from] rusqlite::Error),

    #[error("io error")]
    Io(#[from] std::io::Error),
}

// IMPORTANT: never wrap a key value into a VaultError variant.
// Errors are logged; key values are not.
```

**Acceptance:** Compiles. CC verifies no error variant carries plaintext key material.

---

## T1.3 — Crypto module (3-4h, security-critical)

Create `crates/holster-vault/src/crypto.rs`:

```rust
//! Cryptographic primitives for Holster.
//!
//! Argon2id parameters are LOCKED. Do not change without security review.
//! Per OWASP 2024 recommendations for interactive use.

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng as AeadRng},
    Aes256Gcm, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;
use rand::rngs::OsRng;
use secrecy::{ExposeSecret, Secret};
use zeroize::Zeroize;

use crate::error::VaultError;

/// Argon2id parameters. LOCKED.
const ARGON2_MEMORY_KB: u32 = 65_536;  // 64 MB
const ARGON2_TIME_COST: u32 = 3;
const ARGON2_PARALLELISM: u32 = 4;
const ARGON2_OUTPUT_LEN: usize = 64;   // 64 bytes → split into two 32-byte keys

const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

pub struct DerivedKeys {
    pub sqlcipher_key: Secret<[u8; 32]>,
    pub aes_key: Secret<[u8; 32]>,
}

/// Generate a new random salt for vault creation.
pub fn generate_salt() -> [u8; SALT_LEN] {
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    salt
}

/// Generate a fresh nonce for AES-GCM.
/// MUST be called once per encryption — never reuse with the same key.
pub fn generate_nonce() -> [u8; NONCE_LEN] {
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);
    nonce
}

/// Derive SQLCipher key + AES key from a master password and salt.
pub fn derive_keys(password: &str, salt: &[u8]) -> Result<DerivedKeys, VaultError> {
    if salt.len() != SALT_LEN {
        return Err(VaultError::Crypto("invalid salt length".into()));
    }

    let params = Params::new(
        ARGON2_MEMORY_KB,
        ARGON2_TIME_COST,
        ARGON2_PARALLELISM,
        Some(ARGON2_OUTPUT_LEN),
    )
    .map_err(|e| VaultError::Crypto(format!("argon2 params: {e}")))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut output = [0u8; ARGON2_OUTPUT_LEN];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut output)
        .map_err(|e| VaultError::Crypto(format!("argon2 hash: {e}")))?;

    let mut sqlcipher_key = [0u8; 32];
    let mut aes_key = [0u8; 32];
    sqlcipher_key.copy_from_slice(&output[..32]);
    aes_key.copy_from_slice(&output[32..]);
    output.zeroize();

    Ok(DerivedKeys {
        sqlcipher_key: Secret::new(sqlcipher_key),
        aes_key: Secret::new(aes_key),
    })
}

/// Encrypt a plaintext key value. Returns (ciphertext, nonce).
/// Each call generates a fresh nonce. Never reuse nonces.
pub fn encrypt_key_value(
    aes_key: &Secret<[u8; 32]>,
    plaintext: &str,
) -> Result<(Vec<u8>, [u8; NONCE_LEN]), VaultError> {
    let cipher = Aes256Gcm::new_from_slice(aes_key.expose_secret())
        .map_err(|e| VaultError::Crypto(format!("aes init: {e}")))?;

    let nonce_bytes = generate_nonce();
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| VaultError::Crypto(format!("aes encrypt: {e}")))?;

    Ok((ciphertext, nonce_bytes))
}

/// Decrypt a key value. Returns plaintext wrapped in Secret.
pub fn decrypt_key_value(
    aes_key: &Secret<[u8; 32]>,
    ciphertext: &[u8],
    nonce_bytes: &[u8; NONCE_LEN],
) -> Result<Secret<String>, VaultError> {
    let cipher = Aes256Gcm::new_from_slice(aes_key.expose_secret())
        .map_err(|e| VaultError::Crypto(format!("aes init: {e}")))?;

    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext_bytes = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| VaultError::Crypto(format!("aes decrypt: {e}")))?;

    let plaintext = String::from_utf8(plaintext_bytes)
        .map_err(|e| VaultError::Crypto(format!("utf8: {e}")))?;

    Ok(Secret::new(plaintext))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_encryption() {
        let salt = generate_salt();
        let keys = derive_keys("correct horse battery staple", &salt).unwrap();
        let plaintext = "sk-ant-test-1111111111111111";
        let (ct, nonce) = encrypt_key_value(&keys.aes_key, plaintext).unwrap();
        let recovered = decrypt_key_value(&keys.aes_key, &ct, &nonce).unwrap();
        assert_eq!(recovered.expose_secret(), plaintext);
    }

    #[test]
    fn wrong_password_fails_decrypt() {
        let salt = generate_salt();
        let good = derive_keys("correct horse battery staple", &salt).unwrap();
        let bad = derive_keys("wrong horse battery staple", &salt).unwrap();
        let plaintext = "sk-ant-test-2222222222222222";
        let (ct, nonce) = encrypt_key_value(&good.aes_key, plaintext).unwrap();
        let result = decrypt_key_value(&bad.aes_key, &ct, &nonce);
        assert!(result.is_err());
    }

    #[test]
    fn nonces_are_unique() {
        let n1 = generate_nonce();
        let n2 = generate_nonce();
        assert_ne!(n1, n2);
    }

    #[test]
    fn salt_is_correct_length() {
        let salt = generate_salt();
        assert_eq!(salt.len(), SALT_LEN);
    }

    #[test]
    fn same_password_same_salt_same_key() {
        let salt = [42u8; SALT_LEN];
        let k1 = derive_keys("test", &salt).unwrap();
        let k2 = derive_keys("test", &salt).unwrap();
        assert_eq!(k1.aes_key.expose_secret(), k2.aes_key.expose_secret());
    }

    #[test]
    fn different_salt_different_key() {
        let s1 = generate_salt();
        let s2 = generate_salt();
        let k1 = derive_keys("test", &s1).unwrap();
        let k2 = derive_keys("test", &s2).unwrap();
        assert_ne!(k1.aes_key.expose_secret(), k2.aes_key.expose_secret());
    }
}
```

**Acceptance:**
- `cargo test -p holster-vault crypto` — all tests pass
- CC verifies: Argon2 params match spec exactly, no `unwrap()` outside tests, `secrecy::Secret` used consistently

---

## T1.4 — Models (1h)

Create `crates/holster-vault/src/models.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Anthropic,
    OpenAI,
    Google,
    Replicate,
    ElevenLabs,
    Pinecone,
    Stripe,
    Cloudflare,
    Generic,
}

impl Provider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Provider::Anthropic => "anthropic",
            Provider::OpenAI => "openai",
            Provider::Google => "google",
            Provider::Replicate => "replicate",
            Provider::ElevenLabs => "elevenlabs",
            Provider::Pinecone => "pinecone",
            Provider::Stripe => "stripe",
            Provider::Cloudflare => "cloudflare",
            Provider::Generic => "generic",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "anthropic" => Some(Self::Anthropic),
            "openai" => Some(Self::OpenAI),
            "google" => Some(Self::Google),
            "replicate" => Some(Self::Replicate),
            "elevenlabs" => Some(Self::ElevenLabs),
            "pinecone" => Some(Self::Pinecone),
            "stripe" => Some(Self::Stripe),
            "cloudflare" => Some(Self::Cloudflare),
            "generic" => Some(Self::Generic),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyStatus {
    Active,
    ExpiringSoon,
    Expired,
    Stale,
    Revoked,
}

/// Key metadata — safe to render and serialize. Never contains plaintext.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMetadata {
    pub id: Uuid,
    pub provider: Provider,
    pub label: String,
    pub project_tag: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_rotated_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub status: KeyStatus,
    pub notes: Option<String>,
    pub key_format_valid: bool,
}

#[derive(Debug, Clone)]
pub struct AddKeyInput {
    pub provider: Provider,
    pub label: String,
    pub key_value: String,        // raw plaintext — encrypted before storage
    pub project_tag: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

// Custom Debug to redact key_value
impl std::fmt::Debug for AddKeyInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AddKeyInput")
            .field("provider", &self.provider)
            .field("label", &self.label)
            .field("key_value", &"<redacted>")
            .field("project_tag", &self.project_tag)
            .field("expires_at", &self.expires_at)
            .field("notes", &self.notes)
            .finish()
    }
}
```

Note: We do NOT derive Debug on `AddKeyInput` directly because it contains `key_value`. We hand-roll a redacting impl.

**Acceptance:** `cargo test -p holster-vault models` passes. CC verifies `key_value` is not visible in any auto-derived Debug output.

---

## T1.5 — DB module (3-4h)

Create `crates/holster-vault/src/db.rs` with:
- Connection setup that calls `PRAGMA key = '<hex>'` to unlock SQLCipher
- Schema creation (use exact DDL from `02_ARCHITECTURE.md`)
- Migration runner with version table
- Helper functions: `insert_key`, `select_key_by_id`, `select_all_metadata`, `update_key`, `delete_key`, `update_last_used`

Key implementation notes:
- SQLCipher key format: pass as hex via `PRAGMA key = "x'<64-hex-chars>'"`
- Always parameterize queries (never string-concat user input)
- Use `rusqlite::Connection::open_in_memory()` for tests
- Wrap connection in `std::sync::Mutex` since `Connection` is `!Sync`

(Detailed code skeleton omitted for brevity — Alex implements following the schema and the pattern shown in T1.3.)

**Acceptance:**
- `cargo test -p holster-vault db` covers: schema migration runs, parameterized queries work, in-memory DB works for tests
- CC verifies: no string-concat SQL, `PRAGMA key` only logged at debug level with key value masked

---

## T1.6 — Session module (2h)

Create `crates/holster-vault/src/session.rs`:
- `SessionToken` is a UUID v4 wrapped in a newtype
- `SessionState` holds: token, created_at, last_activity_at, derived AES key
- `validate_token(token)` checks existence + idle timeout
- `touch_token(token)` updates `last_activity_at`
- `revoke_token(token)` removes session and zeroizes key material

**Acceptance:**
- Token validation rejects expired tokens
- `revoke_token` actually zeroes the AES key in memory (test with controlled drop)

---

## T1.7 — Vault facade (2h)

Create `crates/holster-vault/src/vault.rs` — the public API that ties crypto + db + session together.

```rust
pub struct Vault {
    db_path: PathBuf,
    sessions: Mutex<HashMap<SessionToken, SessionState>>,
}

impl Vault {
    pub fn create(path: &Path, password: &str) -> Result<Self, VaultError> { ... }
    pub fn open(path: &Path) -> Result<Self, VaultError> { ... }
    pub fn unlock(&self, password: &str) -> Result<SessionToken, VaultError> { ... }
    pub fn lock(&self, token: SessionToken) -> Result<(), VaultError> { ... }
    pub fn add_key(&self, token: SessionToken, input: AddKeyInput) -> Result<KeyMetadata, VaultError> { ... }
    pub fn list_keys(&self, token: SessionToken) -> Result<Vec<KeyMetadata>, VaultError> { ... }
    pub fn get_key_value(&self, token: SessionToken, id: Uuid) -> Result<Secret<String>, VaultError> { ... }
    // ... etc
}
```

**Acceptance:**
- Integration tests in `tests/integration/vault_lifecycle.rs` cover full create → unlock → add → get → lock → unlock → get cycle
- CC verifies: every public method validates session token before doing anything

---

## T1.8 — Test harness CLI (1h)

Create a minimal `apps/cli/src/main.rs` with subcommands `create`, `unlock`, `add`, `list`, `get` that exercise the full vault API. Used for manual smoke tests and as the second consumer (alongside the eventual desktop app) of `holster-vault`.

**Acceptance:**
- `cargo run -p holster-cli -- create /tmp/test.db` prompts for password, creates vault
- Add a fake key, list it, get its value → roundtrip works

---

## T1.9 — CC Review Pass (1-2h, blocking)

Before merging M1 to `dev`, CC runs the security review checklist (see `07_SECURITY_REVIEW_CHECKLIST.md`) and produces a written report. Dave reviews report and approves merge.

**M1 done when:**
- All tests pass
- CC review report says "approved"
- Dave can run `cargo test --workspace` clean
- Dave has manually exercised the test harness CLI with a real (test) Anthropic key, confirmed encryption roundtrip
