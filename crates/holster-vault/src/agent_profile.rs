//! Agent-scoped access profiles for Holster.
//!
//! Phase 1 is intentionally small: the store is built in memory by callers or
//! tests, and it matches only metadata. It never handles plaintext key values.

use crate::error::VaultError;
use crate::models::{KeyMetadata, Provider};
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// A single allowed metadata pattern for an agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllowedKeyPattern {
    pub provider: Option<Provider>,
    pub project_tag: Option<String>,
    pub label_pattern: Option<String>,
}

impl AllowedKeyPattern {
    pub fn new(
        provider: Option<Provider>,
        project_tag: Option<String>,
        label_pattern: Option<String>,
    ) -> Self {
        Self {
            provider,
            project_tag,
            label_pattern,
        }
    }

    fn matches(&self, metadata: &KeyMetadata) -> bool {
        if let Some(provider) = self.provider {
            if metadata.provider != provider {
                return false;
            }
        }

        if let Some(expected_project) = &self.project_tag {
            if metadata.project_tag.as_deref() != Some(expected_project.as_str()) {
                return false;
            }
        }

        if let Some(pattern) = &self.label_pattern {
            if !matches_label_pattern(pattern, &metadata.label) {
                return false;
            }
        }

        true
    }
}

/// Allowlist for one agent identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentProfile {
    pub agent_id: String,
    pub allowed_patterns: Vec<AllowedKeyPattern>,
}

impl AgentProfile {
    pub fn new(agent_id: impl Into<String>, allowed_patterns: Vec<AllowedKeyPattern>) -> Self {
        Self {
            agent_id: agent_id.into(),
            allowed_patterns,
        }
    }

    fn allows(&self, metadata: &KeyMetadata) -> bool {
        self.allowed_patterns
            .iter()
            .any(|pattern| pattern.matches(metadata))
    }
}

/// In-memory profile store. Unknown agents fail closed.
#[derive(Debug, Clone, Default)]
pub struct AgentProfileStore {
    profiles: Vec<AgentProfile>,
}

impl AgentProfileStore {
    pub fn new(profiles: Vec<AgentProfile>) -> Self {
        Self { profiles }
    }

    pub fn from_json_file(path: &Path) -> Result<Self, VaultError> {
        ensure_owner_only_file(path)?;
        let text = std::fs::read_to_string(path)?;
        let profiles: Vec<AgentProfile> = serde_json::from_str(&text)
            .map_err(|e| VaultError::Migration(format!("agent profile json parse failed: {e}")))?;
        Ok(Self::new(profiles))
    }

    pub fn from_json_dir(path: &Path) -> Result<Self, VaultError> {
        ensure_owner_only_dir(path)?;
        let mut profiles = Vec::new();
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let profile_path = entry.path();
            if profile_path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            ensure_owner_only_file(&profile_path)?;
            let text = std::fs::read_to_string(&profile_path)?;
            let mut file_profiles: Vec<AgentProfile> =
                serde_json::from_str(&text).map_err(|e| {
                    VaultError::Migration(format!(
                        "agent profile json parse failed for {}: {e}",
                        profile_path.display()
                    ))
                })?;
            profiles.append(&mut file_profiles);
        }
        Ok(Self::new(profiles))
    }

    pub fn allows(&self, agent_id: &str, metadata: &KeyMetadata) -> bool {
        self.profiles
            .iter()
            .find(|profile| profile.agent_id == agent_id)
            .map(|profile| profile.allows(metadata))
            .unwrap_or(false)
    }
}

fn ensure_owner_only_file(_path: &Path) -> Result<(), VaultError> {
    #[cfg(unix)]
    {
        let mode = std::fs::metadata(_path)?.permissions().mode() & 0o777;
        if mode & 0o077 != 0 {
            return Err(VaultError::Migration(format!(
                "agent profile file {} has insecure permissions {mode:o}; want owner-only",
                _path.display()
            )));
        }
    }
    Ok(())
}

fn ensure_owner_only_dir(_path: &Path) -> Result<(), VaultError> {
    #[cfg(unix)]
    {
        let mode = std::fs::metadata(_path)?.permissions().mode() & 0o777;
        if mode & 0o077 != 0 {
            return Err(VaultError::Migration(format!(
                "agent profile directory {} has insecure permissions {mode:o}; want owner-only",
                _path.display()
            )));
        }
    }
    Ok(())
}

/// Minimal wildcard matcher for labels. Supports a single or multiple `*`
/// segments; no regex engine needed for the fake-key access gate.
fn matches_label_pattern(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return pattern == value;
    }

    let anchored_start = !pattern.starts_with('*');
    let anchored_end = !pattern.ends_with('*');
    let parts: Vec<&str> = pattern.split('*').filter(|part| !part.is_empty()).collect();

    if parts.is_empty() {
        return true;
    }

    let mut cursor = 0usize;
    for (idx, part) in parts.iter().enumerate() {
        let haystack = &value[cursor..];
        let Some(found) = haystack.find(part) else {
            return false;
        };
        if idx == 0 && anchored_start && found != 0 {
            return false;
        }
        cursor += found + part.len();
    }

    if anchored_end {
        if let Some(last) = parts.last() {
            return value.ends_with(last);
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{KeyStatus, Provider};
    use chrono::Utc;
    use tempfile::TempDir;
    use uuid::Uuid;

    fn metadata(label: &str, project_tag: Option<&str>) -> KeyMetadata {
        KeyMetadata {
            id: Uuid::new_v4(),
            provider: Provider::Generic,
            label: label.to_string(),
            project_tag: project_tag.map(str::to_string),
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
    fn profile_allows_matching_fake_key() {
        let store = AgentProfileStore::new(vec![AgentProfile::new(
            "codex",
            vec![AllowedKeyPattern::new(
                Some(Provider::Generic),
                Some("fake-codex".into()),
                Some("fake-*".into()),
            )],
        )]);

        assert!(store.allows("codex", &metadata("fake-openai-smoke", Some("fake-codex"))));
    }

    #[test]
    fn profile_denies_wrong_project() {
        let store = AgentProfileStore::new(vec![AgentProfile::new(
            "codex",
            vec![AllowedKeyPattern::new(
                Some(Provider::Generic),
                Some("fake-codex".into()),
                Some("fake-*".into()),
            )],
        )]);

        assert!(!store.allows("codex", &metadata("fake-openai-smoke", Some("fake-aliza"))));
    }

    #[test]
    fn profile_denies_unknown_agent() {
        let store = AgentProfileStore::new(vec![AgentProfile::new(
            "codex",
            vec![AllowedKeyPattern::new(None, None, Some("fake-*".into()))],
        )]);

        assert!(!store.allows("aliza", &metadata("fake-openai-smoke", None)));
    }

    #[test]
    fn profile_store_loads_from_json_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("agents.json");
        std::fs::write(
            &path,
            r#"[
              {
                "agent_id": "codex",
                "allowed_patterns": [
                  {
                    "provider": "generic",
                    "project_tag": "fake-codex",
                    "label_pattern": "fake-*"
                  }
                ]
              }
            ]"#,
        )
        .unwrap();
        set_owner_only_file(&path);

        let store = AgentProfileStore::from_json_file(&path).unwrap();
        assert!(store.allows("codex", &metadata("fake-openai-smoke", Some("fake-codex"))));
    }

    #[test]
    fn profile_store_loads_from_json_dir_and_ignores_non_json() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("README.md"), "not a profile").unwrap();
        let profile_path = dir.path().join("codex.json");
        std::fs::write(
            &profile_path,
            r#"[
              {
                "agent_id": "codex",
                "allowed_patterns": [
                  {
                    "provider": "generic",
                    "project_tag": "fake-codex",
                    "label_pattern": "fake-*"
                  }
                ]
              }
            ]"#,
        )
        .unwrap();
        set_owner_only_file(&profile_path);
        set_owner_only_dir(dir.path());

        let store = AgentProfileStore::from_json_dir(dir.path()).unwrap();
        assert!(store.allows("codex", &metadata("fake-openai-smoke", Some("fake-codex"))));
    }

    #[test]
    fn profile_store_rejects_malformed_json() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("agents.json");
        std::fs::write(&path, "{not json").unwrap();
        set_owner_only_file(&path);
        let err = AgentProfileStore::from_json_file(&path).unwrap_err();
        assert!(matches!(err, VaultError::Migration(_)));
    }

    #[cfg(unix)]
    #[test]
    fn profile_store_rejects_group_or_world_readable_file() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new().unwrap();
        let path = dir.path().join("agents.json");
        std::fs::write(&path, "[]").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let err = AgentProfileStore::from_json_file(&path).unwrap_err();
        assert!(matches!(err, VaultError::Migration(_)));
    }

    #[cfg(unix)]
    #[test]
    fn profile_store_rejects_group_or_world_searchable_dir() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new().unwrap();
        let profile_path = dir.path().join("agents.json");
        std::fs::write(&profile_path, "[]").unwrap();
        set_owner_only_file(&profile_path);
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        let err = AgentProfileStore::from_json_dir(dir.path()).unwrap_err();
        assert!(matches!(err, VaultError::Migration(_)));
    }

    #[test]
    fn wildcard_matcher_handles_prefix_suffix_and_middle() {
        assert!(matches_label_pattern("fake-*", "fake-openai"));
        assert!(matches_label_pattern("*-smoke", "fake-openai-smoke"));
        assert!(matches_label_pattern("fake-*smoke", "fake-openai-smoke"));
        assert!(!matches_label_pattern(
            "fake-*smoke",
            "prefix-fake-openai-smoke"
        ));
    }

    fn set_owner_only_file(_path: &Path) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(_path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
    }

    fn set_owner_only_dir(_path: &Path) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(_path, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
    }
}
