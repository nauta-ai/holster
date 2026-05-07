//! Metadata-only audit events for agent secret fetch attempts.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::VaultError;
use crate::models::KeyMetadata;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditOutcome {
    Allowed,
    Denied,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub ts: DateTime<Utc>,
    pub agent_id: String,
    pub key_id: Uuid,
    pub provider: String,
    pub label: String,
    pub project_tag: Option<String>,
    pub outcome: AuditOutcome,
    pub reason: Option<String>,
}

impl AuditEvent {
    pub fn fetch(
        agent_id: &str,
        metadata: &KeyMetadata,
        outcome: AuditOutcome,
        reason: Option<&str>,
    ) -> Self {
        Self {
            ts: Utc::now(),
            agent_id: agent_id.to_string(),
            key_id: metadata.id,
            provider: metadata.provider.as_str().to_string(),
            label: metadata.label.clone(),
            project_tag: metadata.project_tag.clone(),
            outcome,
            reason: reason.map(str::to_string),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuditLogger {
    path: PathBuf,
}

impl AuditLogger {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn log(&self, event: &AuditEvent) -> Result<(), VaultError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
            set_owner_only_dir(parent);
        }

        let line = serde_json::to_string(event)
            .map_err(|e| VaultError::Crypto(format!("audit json serialize: {e}")))?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        set_owner_only_file(&self.path);
        writeln!(file, "{line}")?;
        Ok(())
    }
}

fn set_owner_only_dir(_path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(_path, std::fs::Permissions::from_mode(0o700));
    }
}

fn set_owner_only_file(_path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(_path, std::fs::Permissions::from_mode(0o600));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{KeyStatus, Provider};
    use chrono::Utc;
    use tempfile::TempDir;

    fn metadata() -> KeyMetadata {
        KeyMetadata {
            id: Uuid::new_v4(),
            provider: Provider::Generic,
            label: "fake-openai-smoke".to_string(),
            project_tag: Some("fake-codex".to_string()),
            created_at: Utc::now(),
            expires_at: None,
            last_rotated_at: None,
            last_used_at: None,
            status: KeyStatus::Active,
            notes: None,
            key_format_valid: true,
        }
    }

    #[test]
    fn audit_event_does_not_include_plaintext() {
        let event = AuditEvent::fetch("codex", &metadata(), AuditOutcome::Allowed, None);
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("fake-openai-smoke"));
        assert!(!json.contains("sk-"));
        assert!(!json.contains("secret"));
    }

    #[test]
    fn logger_appends_jsonl() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("audit").join("fetch-events.jsonl");
        let logger = AuditLogger::new(&path);
        let event = AuditEvent::fetch("codex", &metadata(), AuditOutcome::Denied, Some("test"));
        logger.log(&event).unwrap();
        let text = std::fs::read_to_string(path).unwrap();
        assert!(text.contains("\"agent_id\":\"codex\""));
        assert!(text.contains("\"outcome\":\"denied\""));
    }
}
