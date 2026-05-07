//! Holster M3.1 T3.1.3 — Agent runtime profiles.
//!
//! Pure UX layer over the existing hardened `export_runtime_profile`
//! Tauri command. This module contains only a static catalogue of named
//! presets (filename + suggested env-var names + description). The
//! frontend reads the catalogue, prefills the export dialog when a
//! profile is picked, and the user remains free to override every field
//! before submitting.
//!
//! Hard guardrails (per the M3.1 scope doc):
//!   - **Picks only — never enforces.** The user can uncheck any key,
//!     change the filename, or change the target dir before confirming.
//!   - **No remote profile updates.** Every profile lives in the
//!     binary; new profiles ship in a new Holster release.
//!   - **No auto-detect of which profile to use.** The user picks.
//!   - **No real credentials in this file.** Suggested env-var NAMES
//!     only. Tests assert the catalogue carries no plausible-key-shaped
//!     strings.
//!   - **No behaviour change to `export_runtime_profile`.** The export
//!     command is reused as-is. All T2.12 / T2.12.1 / T2.12.2 hardening
//!     remains in force.

use serde::Serialize;
use std::sync::OnceLock;

#[derive(Serialize, Clone, Debug)]
pub struct AgentProfile {
    /// Stable id used in the runtime-export `profile_name` field and the
    /// audit log. Snake_case, no spaces.
    pub id: &'static str,
    /// Display name shown in the dropdown.
    pub name: &'static str,
    /// One- or two-sentence explanation of what the profile is for and
    /// when to use it. Shown under the dropdown in the dialog.
    pub description: &'static str,
    /// The default filename Holster proposes when this profile is
    /// picked. Always passes `is_safe_env_filename` (see lib.rs).
    pub default_filename: &'static str,
    /// Env-var NAMES this profile typically wants populated. Names
    /// only — never values. Used by the UI to highlight suggested
    /// keys; the user is free to ignore them.
    pub suggested_env_vars: &'static [&'static str],
    /// Optional caveat shown in the dialog (e.g., "names not yet
    /// confirmed for Hermes — verify with your install"). `None` for
    /// fully-pinned profiles.
    pub todo_note: Option<&'static str>,
}

/// Lazily-built static catalogue. Stable across calls.
pub fn agent_profile_catalog() -> &'static [AgentProfile] {
    static CATALOG: OnceLock<Vec<AgentProfile>> = OnceLock::new();
    CATALOG.get_or_init(build_catalog)
}

fn build_catalog() -> Vec<AgentProfile> {
    vec![
        AgentProfile {
            id: "generic",
            name: "Generic .env",
            description: "Plain .env file with no preset opinions. \
                Holster derives env-var names from each key's provider \
                using the existing default mapping. Use this when none \
                of the named profiles fits your project.",
            default_filename: ".env",
            suggested_env_vars: &[],
            todo_note: None,
        },
        AgentProfile {
            id: "openclaw",
            name: "OpenClaw",
            description: "Runtime env file for an OpenClaw agent install. \
                Picks .env.local at the project root and surfaces the \
                Telegram + model + tool variables OpenClaw typically \
                reads.",
            default_filename: ".env.local",
            suggested_env_vars: &[
                "TELEGRAM_BOT_TOKEN",
                "OPENAI_API_KEY",
                "ANTHROPIC_API_KEY",
                "GROQ_API_KEY",
            ],
            todo_note: None,
        },
        AgentProfile {
            id: "claude_code",
            name: "Claude Code",
            description: "Project-level env file for the Claude Code CLI. \
                Lives at .env in the project root (NOT in ~/.claude/). \
                Anthropic's API key is the only required variable for \
                most Claude Code flows.",
            default_filename: ".env",
            suggested_env_vars: &["ANTHROPIC_API_KEY"],
            todo_note: None,
        },
        AgentProfile {
            id: "codex",
            name: "Codex (OpenAI CLI)",
            description: "Runtime env for the Codex CLI. Defaults to .env \
                at the project root. If your install reads .env.local \
                instead, change the filename in the field below before \
                exporting.",
            default_filename: ".env",
            suggested_env_vars: &["OPENAI_API_KEY", "OPENAI_ORG_ID"],
            todo_note: None,
        },
        AgentProfile {
            id: "hermes",
            name: "Hermes",
            description: "Conservative runtime env for a Hermes install. \
                Lives at .env.local at the project root. Includes the \
                most-common provider keys; tweak the selection to match \
                what your specific Hermes config actually consumes.",
            default_filename: ".env.local",
            // Conservative set: only providers we KNOW are common. We
            // deliberately avoid inventing Hermes-specific env-var
            // names until Dave confirms the actual list. Per the M3.1
            // scope doc, "do not block the whole feature on Hermes
            // perfection."
            suggested_env_vars: &["OPENAI_API_KEY", "ANTHROPIC_API_KEY"],
            todo_note: Some(
                "Hermes-specific env-var names are not pinned in this \
                 release. Confirm the exact variable names against your \
                 Hermes install before relying on this profile in \
                 production. Treat this preset as a starting shape, not \
                 a contract.",
            ),
        },
    ]
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn catalog_has_five_profiles() {
        let cat = agent_profile_catalog();
        assert_eq!(cat.len(), 5, "expected 5 V0 profiles, got {}", cat.len());
    }

    #[test]
    fn catalog_is_stable_across_calls() {
        let a = agent_profile_catalog();
        let b = agent_profile_catalog();
        assert_eq!(a.len(), b.len());
        // Same backing slice — OnceLock guarantees pointer stability.
        assert_eq!(a.as_ptr(), b.as_ptr());
    }

    #[test]
    fn ids_are_unique() {
        let cat = agent_profile_catalog();
        let mut seen: HashSet<&str> = HashSet::new();
        for p in cat {
            assert!(seen.insert(p.id), "duplicate profile id: {}", p.id);
        }
    }

    #[test]
    fn ids_are_snake_case_no_spaces() {
        let cat = agent_profile_catalog();
        for p in cat {
            assert!(!p.id.contains(' '), "profile id {} contains a space", p.id);
            assert!(
                p.id.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
                "profile id {} should be snake_case ASCII lowercase",
                p.id
            );
        }
    }

    #[test]
    fn names_and_descriptions_are_non_empty() {
        let cat = agent_profile_catalog();
        for p in cat {
            assert!(!p.name.trim().is_empty(), "profile {} has empty name", p.id);
            assert!(
                !p.description.trim().is_empty(),
                "profile {} has empty description",
                p.id
            );
        }
    }

    #[test]
    fn default_filenames_are_safe_env_filenames() {
        // Mirror the predicate from lib.rs without depending on it.
        // A safe env filename is .env, .env.local, or ends with .env,
        // and contains no path separators.
        let cat = agent_profile_catalog();
        for p in cat {
            let f = p.default_filename;
            assert!(
                !f.contains('/') && !f.contains('\\'),
                "profile {} default_filename {} must not contain path separators",
                p.id,
                f
            );
            assert!(
                f == ".env" || f == ".env.local" || f.ends_with(".env"),
                "profile {} default_filename {} must be .env, .env.local, or end with .env",
                p.id,
                f
            );
        }
    }

    #[test]
    fn generic_profile_has_no_suggested_vars() {
        let cat = agent_profile_catalog();
        let generic = cat.iter().find(|p| p.id == "generic").unwrap();
        assert!(
            generic.suggested_env_vars.is_empty(),
            "generic profile is the no-preset option; it must not suggest vars"
        );
    }

    #[test]
    fn hermes_carries_a_todo_note() {
        let cat = agent_profile_catalog();
        let hermes = cat.iter().find(|p| p.id == "hermes").unwrap();
        assert!(
            hermes.todo_note.is_some(),
            "hermes must carry a todo_note explaining its conservative nature"
        );
        let note = hermes.todo_note.unwrap();
        assert!(
            note.to_lowercase().contains("confirm")
                || note.to_lowercase().contains("not pinned")
                || note.to_lowercase().contains("not yet"),
            "hermes todo_note should signal uncertainty: {note}"
        );
    }

    #[test]
    fn pinned_profiles_have_no_todo_note() {
        let cat = agent_profile_catalog();
        for id in ["generic", "openclaw", "claude_code", "codex"] {
            let p = cat.iter().find(|p| p.id == id).unwrap();
            assert!(
                p.todo_note.is_none(),
                "profile {} should not carry a todo_note",
                id
            );
        }
    }

    #[test]
    fn all_required_profiles_present() {
        let cat = agent_profile_catalog();
        let ids: HashSet<&str> = cat.iter().map(|p| p.id).collect();
        for required in ["generic", "openclaw", "claude_code", "codex", "hermes"] {
            assert!(
                ids.contains(required),
                "required profile id '{required}' missing from catalogue"
            );
        }
    }

    #[test]
    fn suggested_env_vars_are_ascii_uppercase_with_underscores_only() {
        // Standard env-var name convention. Catches typos and stops
        // anyone from accidentally leaking lowercase secret values
        // through this field.
        let cat = agent_profile_catalog();
        for p in cat {
            for var in p.suggested_env_vars {
                assert!(
                    !var.is_empty(),
                    "profile {} has an empty suggested_env_var",
                    p.id
                );
                for ch in var.chars() {
                    assert!(
                        (ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_'),
                        "profile {} env var {} contains illegal char {:?}",
                        p.id,
                        var,
                        ch
                    );
                }
            }
        }
    }

    #[test]
    fn no_field_carries_a_plausible_key_shaped_string() {
        // Sanity guard: nothing in this catalog should look like a real
        // API key. Heuristic: no field should contain a 25+ char run of
        // [A-Za-z0-9_-] (the shape of OpenAI / Anthropic / GitHub PATs
        // etc.). If this test ever fires, someone leaked a value.
        let cat = agent_profile_catalog();
        let plausible = |s: &str| -> bool {
            let mut run = 0usize;
            for ch in s.chars() {
                if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                    run += 1;
                    if run >= 25 {
                        return true;
                    }
                } else {
                    run = 0;
                }
            }
            false
        };
        for p in cat {
            assert!(!plausible(p.id), "profile id {} looks key-shaped", p.id);
            assert!(
                !plausible(p.name),
                "profile name {} looks key-shaped",
                p.name
            );
            assert!(
                !plausible(p.description),
                "profile description for {} looks key-shaped",
                p.id
            );
            assert!(
                !plausible(p.default_filename),
                "profile default_filename {} looks key-shaped",
                p.default_filename
            );
            for v in p.suggested_env_vars {
                assert!(
                    !plausible(v),
                    "profile {} suggested_env_var {} looks key-shaped",
                    p.id,
                    v
                );
            }
            if let Some(note) = p.todo_note {
                assert!(
                    !plausible(note),
                    "profile {} todo_note looks key-shaped",
                    p.id
                );
            }
        }
    }

    #[test]
    fn serializes_to_clean_json() {
        // Each profile must be JSON-encodable for the Tauri IPC
        // boundary. Smoke test: round-trip through serde_json and
        // assert the output contains the id and name.
        for p in agent_profile_catalog() {
            let json = serde_json::to_string(p).unwrap();
            assert!(json.contains(p.id), "json missing id for {}", p.id);
            assert!(json.contains(p.name), "json missing name for {}", p.id);
        }
    }

    #[test]
    fn default_filenames_per_scope_doc() {
        // Lock in the exact defaults from the M3.1 scope doc so a
        // future drive-by edit can't silently change them.
        let cat = agent_profile_catalog();
        let by_id = |id: &str| cat.iter().find(|p| p.id == id).unwrap();
        assert_eq!(by_id("generic").default_filename, ".env");
        assert_eq!(by_id("openclaw").default_filename, ".env.local");
        assert_eq!(by_id("claude_code").default_filename, ".env");
        assert_eq!(by_id("codex").default_filename, ".env");
        assert_eq!(by_id("hermes").default_filename, ".env.local");
    }
}
