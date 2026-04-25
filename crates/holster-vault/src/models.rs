//! Vault data models.
//!
//! T1.4: real Provider/KeyStatus enums + KeyMetadata struct + AddKeyInput
//! with a hand-rolled Debug impl that redacts the plaintext key value.
//!
//! Invariants:
//! - `KeyMetadata` is safe to render anywhere — never contains plaintext.
//! - `AddKeyInput` carries `key_value` (plaintext), so Debug is hand-rolled
//!   to redact. NEVER #[derive(Debug)] on AddKeyInput.
//! - Provider serializes to lowercase snake_case for stable on-disk format.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Provider ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Anthropic,
    // Override: snake_case would mangle `OpenAI` → `open_a_i`.
    #[serde(rename = "openai")]
    OpenAI,
    Google,
    Replicate,
    // Override: snake_case would produce `eleven_labs`; keep canonical lowercase.
    #[serde(rename = "elevenlabs")]
    ElevenLabs,
    Pinecone,
    Stripe,
    Cloudflare,
    Generic,
}

impl Provider {
    /// Stable string label for storage and display. Matches `serde(rename_all="snake_case")`.
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

    /// Parse a Provider from its lowercase label. Returns None on unknown input.
    ///
    /// Deliberately returns `Option<Self>` rather than implementing
    /// `std::str::FromStr` (which forces `Result`). Callers want a clean
    /// "known provider or not" signal — a `Result<_, ParseProviderError>`
    /// would just wrap a tag with no useful error context.
    #[allow(clippy::should_implement_trait)]
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

// ── KeyStatus ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyStatus {
    Active,
    ExpiringSoon,
    Expired,
    Stale,
    Revoked,
}

// ── KeyMetadata ──────────────────────────────────────────────────────────────

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

// ── AddKeyInput ──────────────────────────────────────────────────────────────

/// Input for adding a new key. Carries plaintext — `Debug` is hand-rolled to
/// redact `key_value` so it never accidentally lands in logs or panic messages.
#[derive(Clone)]
pub struct AddKeyInput {
    pub provider: Provider,
    pub label: String,
    pub key_value: String,        // raw plaintext — encrypted before storage
    pub project_tag: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

// Hand-rolled Debug — DO NOT replace with #[derive(Debug)].
// Tested in tests::add_key_input_debug_redacts_value below.
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn provider_roundtrips_through_string() {
        for p in [
            Provider::Anthropic, Provider::OpenAI, Provider::Google,
            Provider::Replicate, Provider::ElevenLabs, Provider::Pinecone,
            Provider::Stripe, Provider::Cloudflare, Provider::Generic,
        ] {
            let s = p.as_str();
            assert_eq!(Provider::from_str(s), Some(p), "roundtrip failed for {s}");
        }
    }

    #[test]
    fn provider_from_str_rejects_unknown() {
        assert_eq!(Provider::from_str("unknown"), None);
        assert_eq!(Provider::from_str(""), None);
        assert_eq!(Provider::from_str("ANTHROPIC"), None, "case-sensitive");
    }

    #[test]
    fn provider_serializes_snake_case() {
        let s = serde_json::to_string(&Provider::OpenAI).unwrap();
        assert_eq!(s, r#""openai""#);
    }

    #[test]
    fn key_status_serializes_snake_case() {
        let s = serde_json::to_string(&KeyStatus::ExpiringSoon).unwrap();
        assert_eq!(s, r#""expiring_soon""#);
    }

    #[test]
    fn key_metadata_serializes_without_plaintext() {
        let m = KeyMetadata {
            id: Uuid::new_v4(),
            provider: Provider::Anthropic,
            label: "primary".to_string(),
            project_tag: Some("nauta".to_string()),
            created_at: Utc.with_ymd_and_hms(2026, 4, 25, 0, 0, 0).unwrap(),
            expires_at: None,
            last_rotated_at: None,
            last_used_at: None,
            status: KeyStatus::Active,
            notes: None,
            key_format_valid: true,
        };
        let s = serde_json::to_string(&m).unwrap();
        // KeyMetadata never contains plaintext, but verify the field shape
        assert!(s.contains(r#""label":"primary""#));
        assert!(s.contains(r#""provider":"anthropic""#));
        assert!(s.contains(r#""status":"active""#));
    }

    #[test]
    fn add_key_input_debug_redacts_value() {
        let input = AddKeyInput {
            provider: Provider::Anthropic,
            label: "primary".to_string(),
            key_value: "sk-ant-test-1111111111111111".to_string(),
            project_tag: None,
            expires_at: None,
            notes: None,
        };
        let dbg = format!("{input:?}");
        assert!(dbg.contains("<redacted>"),
                "expected redacted marker in Debug output, got: {dbg}");
        assert!(!dbg.contains("sk-ant"),
                "Debug output leaked plaintext key value: {dbg}");
        assert!(!dbg.contains("1111111111111111"),
                "Debug output leaked plaintext key value: {dbg}");
    }

    #[test]
    fn add_key_input_clone_does_not_corrupt() {
        let input = AddKeyInput {
            provider: Provider::OpenAI,
            label: "test".to_string(),
            key_value: "sk-openai-test-2222".to_string(),
            project_tag: Some("proj".to_string()),
            expires_at: None,
            notes: None,
        };
        let cloned = input.clone();
        assert_eq!(cloned.label, input.label);
        assert_eq!(cloned.key_value, input.key_value);
        assert_eq!(cloned.provider, input.provider);
    }
}
