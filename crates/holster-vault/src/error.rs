//! Vault error types.
//!
//! T1.2: real `thiserror` enum.
//!
//! Invariants:
//! - No error variant carries plaintext key material (label, value, or session
//!   token). Errors are routinely logged; key data must never leak through them.
//! - `KeyNotFound` carries only the UUID, never the key value.
//! - `Crypto` and `Migration` carry strings — those strings are constructed at
//!   call sites and must not include `Secret<...>` contents (caller responsibility).

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
