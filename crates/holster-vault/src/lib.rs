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
