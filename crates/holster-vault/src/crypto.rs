//! Cryptographic primitives for Holster.
//!
//! T1.3: Argon2id KDF + AES-256-GCM authenticated encryption.
//!
//! Argon2id parameters are LOCKED. Per OWASP 2024 recommendations for
//! interactive use. Do not change without security review.
//!
//! Invariants:
//! - All key material is wrapped in `secrecy::Secret<[u8; 32]>` so it never
//!   `Debug`-prints or `Display`s by accident.
//! - Every encryption gets a fresh nonce — never reuse one with the same key.
//! - Salts are 16 bytes from `OsRng`; nonces are 12 bytes from `OsRng`.
//! - The 64-byte Argon2 output is split into two 32-byte keys (SQLCipher + AES).

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::rngs::OsRng;
use rand::RngCore;
use secrecy::{ExposeSecret, Secret};
use zeroize::Zeroize;

use crate::error::VaultError;

// ── LOCKED Argon2id parameters ────────────────────────────────────────────────

const ARGON2_MEMORY_KB: u32 = 65_536; // 64 MB
const ARGON2_TIME_COST: u32 = 3;
const ARGON2_PARALLELISM: u32 = 4;
const ARGON2_OUTPUT_LEN: usize = 64; // 64 bytes → two 32-byte keys

const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

// ── Public API ────────────────────────────────────────────────────────────────

/// Derived keys: one for SQLCipher unlock, one for AES-GCM key-value encryption.
pub struct DerivedKeys {
    pub sqlcipher_key: Secret<[u8; 32]>,
    pub aes_key: Secret<[u8; 32]>,
}

/// Generate a fresh 16-byte salt for vault creation.
pub fn generate_salt() -> [u8; SALT_LEN] {
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    salt
}

/// Generate a fresh 12-byte nonce for AES-GCM encryption.
/// MUST be called once per encryption — never reuse with the same key.
pub fn generate_nonce() -> [u8; NONCE_LEN] {
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);
    nonce
}

/// Derive SQLCipher key + AES key from a master password and salt.
pub fn derive_keys(password: &str, salt: &[u8]) -> Result<DerivedKeys, VaultError> {
    if salt.len() != SALT_LEN {
        return Err(VaultError::Crypto(format!(
            "invalid salt length: got {}, want {}",
            salt.len(),
            SALT_LEN
        )));
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
/// Each call generates a fresh nonce. Never reuse nonces with the same key.
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

/// Decrypt a key value. Returns plaintext wrapped in `Secret`.
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
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
        assert!(result.is_err(), "decrypt with wrong key should fail");
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
        assert_eq!(
            k1.aes_key.expose_secret(),
            k2.aes_key.expose_secret(),
            "deterministic KDF: same input must yield same key"
        );
    }

    #[test]
    fn different_salt_different_key() {
        let s1 = generate_salt();
        let s2 = generate_salt();
        let k1 = derive_keys("test", &s1).unwrap();
        let k2 = derive_keys("test", &s2).unwrap();
        assert_ne!(
            k1.aes_key.expose_secret(),
            k2.aes_key.expose_secret(),
            "different salts must yield different keys (anti-rainbow-table)"
        );
    }
}
