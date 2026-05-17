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

    #[error("entry_not_found: {0}")]
    EntryNotFound(uuid::Uuid),

    #[error("access denied")]
    AccessDenied,

    #[error("schema migration failed: {0}")]
    Migration(String),

    #[error("crypto error: {0}")]
    Crypto(String),

    #[error("database error")]
    Db(#[from] rusqlite::Error),

    #[error("io error")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_vault_error_display() {
        // Locked
        let err = VaultError::Locked;
        assert_eq!(format!("{}", err), "vault not unlocked");

        // InvalidSession
        let err = VaultError::InvalidSession;
        assert_eq!(format!("{}", err), "invalid session token");

        // SessionExpired
        let err = VaultError::SessionExpired;
        assert_eq!(format!("{}", err), "session expired (idle timeout)");

        // BadPassword
        let err = VaultError::BadPassword;
        assert_eq!(format!("{}", err), "incorrect master password");

        // WeakPassword
        let err = VaultError::WeakPassword;
        assert_eq!(
            format!("{}", err),
            "master password too weak (zxcvbn score < 3)"
        );

        // VaultNotFound
        let err = VaultError::VaultNotFound;
        assert_eq!(format!("{}", err), "vault file not found");

        // VaultAlreadyExists
        let err = VaultError::VaultAlreadyExists;
        assert_eq!(format!("{}", err), "vault already exists");

        // KeyNotFound
        let err = VaultError::KeyNotFound(Uuid::nil());
        assert_eq!(
            format!("{}", err),
            format!("key not found: {}", Uuid::nil())
        );

        // EntryNotFound
        let err = VaultError::EntryNotFound(Uuid::nil());
        assert_eq!(
            format!("{}", err),
            format!("entry_not_found: {}", Uuid::nil())
        );

        // AccessDenied
        let err = VaultError::AccessDenied;
        assert_eq!(format!("{}", err), "access denied");

        // Migration
        let err = VaultError::Migration("test migration failure".to_string());
        assert_eq!(
            format!("{}", err),
            "schema migration failed: test migration failure"
        );

        // Crypto
        let err = VaultError::Crypto("test crypto error".to_string());
        assert_eq!(format!("{}", err), "crypto error: test crypto error");
    }
}
