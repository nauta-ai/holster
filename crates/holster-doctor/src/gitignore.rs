//! Holster M3.1 T3.1.2 — Safe `.gitignore` helper.
//!
//! Audits a project folder's `.gitignore` against a curated safe-defaults
//! list and proposes additive patches. The user reviews a diff and confirms
//! before any write happens.
//!
//! Behavior contract:
//!   - **Audit is read-only.** No writes ever happen during an audit call.
//!   - **Apply is append-only.** Existing `.gitignore` lines are never
//!     removed or modified.
//!   - **Idempotent.** Running twice in a row is a no-op the second time
//!     (lines already present are deduped at apply time).
//!   - **Atomic write** via temp + rename, mirroring the runtime export
//!     pattern. chmod 0644 (gitignore is intentionally committable).
//!   - **No secrets cross IPC.** This module never reads, displays,
//!     exports, or logs the content of any file other than the
//!     project root's `.gitignore` and a small set of language-marker
//!     files (whose names — not values — are checked for existence).
//!   - **Path safety.** Refuses `/`, `$HOME`, `/etc`, `/var`, `/usr`,
//!     `/System`, `/Library`, `/private` as targets, mirroring the
//!     repo scanner.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const GITIGNORE_FILENAME: &str = ".gitignore";
const TEMP_SUFFIX: &str = ".holster-tmp";

// ── Public types ────────────────────────────────────────────────────────────

#[derive(Deserialize, Debug)]
pub struct GitignoreAuditArgs {
    pub path: String,
}

#[derive(Deserialize, Debug)]
pub struct GitignoreApplyArgs {
    pub path: String,
    /// Per-rule-set list of rule lines the user confirmed. Comments are
    /// added by the apply layer based on `rule_set_id`; the frontend
    /// never sends raw comment lines.
    pub selections: Vec<RuleSetSelection>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RuleSetSelection {
    pub rule_set_id: String,
    pub lines: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct RuleLine {
    pub line: String,
    pub already_present: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct RuleSet {
    pub id: String,
    pub label: String,
    pub description: String,
    pub default_on: bool,
    pub locked_on: bool,
    pub auto_detected: bool,
    pub header_comment: String,
    pub rules: Vec<RuleLine>,
}

#[derive(Serialize, Clone, Debug)]
pub struct GitignoreAuditReport {
    pub root_path: String,
    pub target_path: String,
    pub gitignore_exists: bool,
    pub project_types: Vec<String>,
    pub rule_sets: Vec<RuleSet>,
    pub existing_line_count: usize,
}

#[derive(Serialize, Clone, Debug)]
pub struct GitignoreApplyReport {
    pub target_path: String,
    pub created_new_file: bool,
    pub lines_added: usize,
    pub appended_block: String,
}

// ── Public entry points ─────────────────────────────────────────────────────

pub fn audit(args: GitignoreAuditArgs) -> Result<GitignoreAuditReport, String> {
    let canonical = resolve_safe_root(&args.path)?;
    let target = canonical.join(GITIGNORE_FILENAME);
    let gitignore_exists = target.is_file();
    let existing_lines = read_existing_lines(&target);
    let existing_count = existing_lines.len();
    let project_types = detect_project_types(&canonical);
    let rule_sets = build_rule_sets(&project_types, &existing_lines);

    Ok(GitignoreAuditReport {
        root_path: canonical.display().to_string(),
        target_path: target.display().to_string(),
        gitignore_exists,
        project_types,
        rule_sets,
        existing_line_count: existing_count,
    })
}

pub fn apply(args: GitignoreApplyArgs) -> Result<GitignoreApplyReport, String> {
    let canonical = resolve_safe_root(&args.path)?;
    let target = canonical.join(GITIGNORE_FILENAME);

    let known_sets = canonical_rule_set_catalog();
    let existing_lines = read_existing_lines(&target);
    let mut existing_set: std::collections::HashSet<String> = existing_lines
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    // Build the appended block in canonical rule-set order. Within each
    // selected set: emit the header comment if not already present, then
    // emit user-accepted lines that aren't already present. Skip any set
    // that contributes zero new lines.
    let mut appended: Vec<String> = Vec::new();
    let mut emitted_groups: usize = 0;

    for canonical_set in &known_sets {
        let Some(sel) = args
            .selections
            .iter()
            .find(|s| s.rule_set_id == canonical_set.id)
        else {
            continue;
        };

        // Only emit lines that (a) belong to this canonical set and (b) aren't
        // already in the file. We re-validate the set membership here to
        // prevent a hostile frontend from sneaking arbitrary lines in.
        let allowed: std::collections::HashSet<String> = canonical_set
            .rules
            .iter()
            .map(|r| r.line.to_string())
            .collect();

        let mut group_lines: Vec<String> = Vec::new();
        for raw in &sel.lines {
            let trimmed = raw.trim().to_string();
            if trimmed.is_empty() {
                continue;
            }
            if !allowed.contains(&trimmed) {
                // Line not part of this canonical set — refuse to write it.
                continue;
            }
            if existing_set.contains(&trimmed) {
                continue;
            }
            group_lines.push(trimmed.clone());
            existing_set.insert(trimmed);
        }

        if group_lines.is_empty() {
            continue;
        }

        // Spacer line between groups for readability
        if emitted_groups > 0 {
            appended.push(String::new());
        }

        let header = canonical_set.header_comment.trim().to_string();
        if !header.is_empty() && !existing_set.contains(&header) {
            appended.push(header.clone());
            existing_set.insert(header);
        }
        appended.extend(group_lines);
        emitted_groups += 1;
    }

    let lines_added = appended.iter().filter(|s| !s.is_empty()).count();
    let target_existed = target.is_file();

    if appended.is_empty() {
        // Nothing to write. If the file doesn't exist, do NOT create it
        // empty — the user's selections produced zero net additions.
        return Ok(GitignoreApplyReport {
            target_path: target.display().to_string(),
            created_new_file: false,
            lines_added: 0,
            appended_block: String::new(),
        });
    }

    let appended_block = format_appended_block(&appended);

    let new_body = if target_existed {
        let mut body = std::fs::read_to_string(&target)
            .map_err(|e| format!("could not read existing .gitignore: {e}"))?;
        if !body.is_empty() && !body.ends_with('\n') {
            body.push('\n');
        }
        body.push_str(&appended_block);
        body
    } else {
        appended_block.clone()
    };

    write_atomic(&target, &new_body)?;
    set_committable_perms(&target);

    Ok(GitignoreApplyReport {
        target_path: target.display().to_string(),
        created_new_file: !target_existed,
        lines_added,
        appended_block,
    })
}

// ── Path safety ─────────────────────────────────────────────────────────────

/// Expand `~/...` shorthand to `$HOME/...`. Leaves other paths untouched.
fn expand_home(input: &str) -> PathBuf {
    if input == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
        return PathBuf::from(input);
    }
    if let Some(rest) = input.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(input)
}

fn resolve_safe_root(raw: &str) -> Result<PathBuf, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("path is empty".into());
    }
    let p = expand_home(trimmed);
    if !p.exists() {
        return Err(format!("path does not exist: {}", p.display()));
    }
    if !p.is_dir() {
        return Err(format!("path is not a directory: {}", p.display()));
    }
    let canonical = p
        .canonicalize()
        .map_err(|e| format!("could not canonicalize path: {e}"))?;
    refuse_dangerous_root(&canonical)?;
    Ok(canonical)
}

fn refuse_dangerous_root(canonical: &Path) -> Result<(), String> {
    if canonical == Path::new("/") {
        return Err("refusing to operate on filesystem root '/'".into());
    }
    if let Some(home_path) = dirs::home_dir() {
        if canonical == home_path.as_path() {
            return Err(format!(
                "refusing to operate on $HOME ({}) directly. Pick a project subdirectory.",
                home_path.display()
            ));
        }
    }
    for forbidden in ["/etc", "/var", "/usr", "/System", "/Library", "/private"] {
        if canonical == Path::new(forbidden) {
            return Err(format!(
                "refusing to operate on system path {forbidden}. Pick a project directory."
            ));
        }
    }
    Ok(())
}

// ── Project type detection ──────────────────────────────────────────────────

pub fn detect_project_types(root: &Path) -> Vec<String> {
    let mut types = Vec::new();
    if root.join("package.json").is_file() {
        types.push("node".to_string());
    }
    if root.join("Cargo.toml").is_file() {
        types.push("rust".to_string());
    }
    if root.join("pyproject.toml").is_file()
        || root.join("setup.py").is_file()
        || root.join("setup.cfg").is_file()
        || python_requirements_file_exists(root)
    {
        types.push("python".to_string());
    }
    types
}

fn python_requirements_file_exists(root: &Path) -> bool {
    if root.join("requirements.txt").is_file() {
        return true;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return false;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(s) = name.to_str() else {
            continue;
        };
        if s.starts_with("requirements") && s.ends_with(".txt") {
            return true;
        }
    }
    false
}

// ── Rule sets ───────────────────────────────────────────────────────────────

#[derive(Clone)]
struct CanonicalRuleSet {
    id: &'static str,
    label: &'static str,
    description: &'static str,
    default_on: bool,
    locked_on: bool,
    header_comment: &'static str,
    rules: Vec<CanonicalRule>,
}

#[derive(Clone)]
struct CanonicalRule {
    line: &'static str,
}

fn canonical_rule_set_catalog() -> Vec<CanonicalRuleSet> {
    vec![
        CanonicalRuleSet {
            id: "universal_env",
            label: "Universal env (always on)",
            description: "Block environment files that commonly carry secrets. \
                 Keep .env.example tracked (with the negation rule) so \
                 collaborators see what variables are required.",
            default_on: true,
            locked_on: true,
            header_comment: "# Holster — Universal env (never commit secrets)",
            rules: vec![
                CanonicalRule { line: ".env" },
                CanonicalRule { line: ".env.*" },
                CanonicalRule { line: "*.env" },
                CanonicalRule {
                    line: "!.env.example",
                },
                CanonicalRule { line: ".env.local" },
                CanonicalRule {
                    line: ".env.*.local",
                },
                CanonicalRule {
                    line: "*.holster-tmp",
                },
            ],
        },
        CanonicalRuleSet {
            id: "holster",
            label: "Holster crash-safety",
            description: "Holster's atomic-write backup files.",
            default_on: true,
            locked_on: false,
            header_comment: "# Holster — backup files",
            rules: vec![CanonicalRule {
                line: ".holster-backup-*",
            }],
        },
        CanonicalRuleSet {
            id: "node",
            label: "Node / pnpm / npm (auto-detected)",
            description: "Build output and dep cache for Node-flavored projects.",
            default_on: true,
            locked_on: false,
            header_comment: "# Node / pnpm / npm",
            rules: vec![
                CanonicalRule {
                    line: "node_modules/",
                },
                CanonicalRule {
                    line: ".pnpm-store/",
                },
                CanonicalRule { line: ".next/" },
                CanonicalRule { line: ".turbo/" },
                CanonicalRule { line: ".vercel/" },
                CanonicalRule { line: "dist/" },
                CanonicalRule { line: "build/" },
            ],
        },
        CanonicalRuleSet {
            id: "python",
            label: "Python (auto-detected)",
            description: "Build, cache, and venv directories for Python projects.",
            default_on: true,
            locked_on: false,
            header_comment: "# Python",
            rules: vec![
                CanonicalRule {
                    line: "__pycache__/",
                },
                CanonicalRule { line: "*.pyc" },
                CanonicalRule { line: ".venv/" },
                CanonicalRule { line: "venv/" },
                CanonicalRule {
                    line: ".pytest_cache/",
                },
                CanonicalRule {
                    line: ".ruff_cache/",
                },
                CanonicalRule {
                    line: ".mypy_cache/",
                },
            ],
        },
        CanonicalRuleSet {
            id: "rust",
            label: "Rust (auto-detected)",
            description: "Cargo build output.",
            default_on: true,
            locked_on: false,
            header_comment: "# Rust",
            rules: vec![CanonicalRule { line: "target/" }],
        },
        CanonicalRuleSet {
            id: "macos_ide",
            label: "macOS / IDE noise (opt-in)",
            description: "Editor and OS metadata. Off by default — many teams want \
                 these tracked or have them in a global gitignore.",
            default_on: false,
            locked_on: false,
            header_comment: "# macOS / editor noise",
            rules: vec![
                CanonicalRule { line: ".DS_Store" },
                CanonicalRule { line: ".idea/" },
                CanonicalRule { line: ".vscode/" },
                CanonicalRule { line: "*.swp" },
            ],
        },
        CanonicalRuleSet {
            id: "cloud_creds",
            label: "Cloud credentials (default on, explicit safety)",
            description: "Block stray local cloud-credential files from being \
                 committed if they happen to live in the project tree.",
            default_on: true,
            locked_on: false,
            header_comment: "# Cloud credential files (never commit)",
            rules: vec![
                CanonicalRule {
                    line: ".aws/credentials",
                },
                CanonicalRule { line: ".gcp/" },
                CanonicalRule { line: ".azure/" },
            ],
        },
    ]
}

fn build_rule_sets(project_types: &[String], existing_lines: &[String]) -> Vec<RuleSet> {
    let existing_trimmed: std::collections::HashSet<String> = existing_lines
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let mut out = Vec::new();
    for set in canonical_rule_set_catalog() {
        let auto_detected = match set.id {
            "node" => project_types.iter().any(|t| t == "node"),
            "python" => project_types.iter().any(|t| t == "python"),
            "rust" => project_types.iter().any(|t| t == "rust"),
            _ => false,
        };
        // Language-specific sets only show up when their marker file is
        // present. The user can still toggle them off in the UI.
        let language_set = matches!(set.id, "node" | "python" | "rust");
        if language_set && !auto_detected {
            continue;
        }
        let rules = set
            .rules
            .iter()
            .map(|r| RuleLine {
                line: r.line.to_string(),
                already_present: existing_trimmed.contains(r.line),
            })
            .collect();
        out.push(RuleSet {
            id: set.id.to_string(),
            label: set.label.to_string(),
            description: set.description.to_string(),
            default_on: set.default_on,
            locked_on: set.locked_on,
            auto_detected,
            header_comment: set.header_comment.to_string(),
            rules,
        });
    }
    out
}

// ── Disk helpers ────────────────────────────────────────────────────────────

fn read_existing_lines(target: &Path) -> Vec<String> {
    let Ok(content) = std::fs::read_to_string(target) else {
        return Vec::new();
    };
    content.lines().map(|s| s.to_string()).collect()
}

fn format_appended_block(lines: &[String]) -> String {
    let mut out = String::new();
    for line in lines {
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn write_atomic(target: &Path, body: &str) -> Result<(), String> {
    let filename = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "target path has no filename".to_string())?;
    let temp_path = target.with_file_name(format!("{filename}{TEMP_SUFFIX}"));
    if let Err(e) = std::fs::write(&temp_path, body) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!("could not write .gitignore temp: {e}"));
    }
    if let Err(e) = std::fs::rename(&temp_path, target) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!("could not rename .gitignore temp into place: {e}"));
    }
    Ok(())
}

fn set_committable_perms(_target: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // .gitignore is intentionally committable; 0644 matches what most
        // editors create. Best-effort — chmod failure is not fatal here.
        let _ = std::fs::set_permissions(_target, std::fs::Permissions::from_mode(0o644));
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_tempdir(label: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let mut p = std::env::temp_dir();
        p.push(format!("holster-gitignore-{label}-{nanos}-{n}"));
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn audit_path(p: &Path) -> GitignoreAuditReport {
        audit(GitignoreAuditArgs {
            path: p.display().to_string(),
        })
        .expect("audit should succeed")
    }

    fn select_all_default_on(report: &GitignoreAuditReport) -> Vec<RuleSetSelection> {
        report
            .rule_sets
            .iter()
            .filter(|rs| rs.default_on)
            .map(|rs| RuleSetSelection {
                rule_set_id: rs.id.clone(),
                lines: rs.rules.iter().map(|r| r.line.clone()).collect(),
            })
            .collect()
    }

    // ── Project type detection ─────────────────────────────────────────────

    #[test]
    fn detect_node_via_package_json() {
        let dir = unique_tempdir("detect-node");
        fs::write(dir.join("package.json"), "{}").unwrap();
        let report = audit_path(&dir);
        assert!(report.project_types.contains(&"node".to_string()));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn detect_rust_via_cargo_toml() {
        let dir = unique_tempdir("detect-rust");
        fs::write(dir.join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();
        let report = audit_path(&dir);
        assert!(report.project_types.contains(&"rust".to_string()));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn detect_python_via_pyproject_toml() {
        let dir = unique_tempdir("detect-py-pyproj");
        fs::write(dir.join("pyproject.toml"), "[project]\n").unwrap();
        let report = audit_path(&dir);
        assert!(report.project_types.contains(&"python".to_string()));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn detect_python_via_requirements_txt() {
        let dir = unique_tempdir("detect-py-req");
        fs::write(dir.join("requirements.txt"), "requests==2.31.0\n").unwrap();
        let report = audit_path(&dir);
        assert!(report.project_types.contains(&"python".to_string()));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn detect_python_via_dev_requirements() {
        let dir = unique_tempdir("detect-py-devreq");
        fs::write(dir.join("requirements-dev.txt"), "pytest\n").unwrap();
        let report = audit_path(&dir);
        assert!(report.project_types.contains(&"python".to_string()));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn detect_python_via_setup_py() {
        let dir = unique_tempdir("detect-py-setup");
        fs::write(dir.join("setup.py"), "from setuptools import setup\n").unwrap();
        let report = audit_path(&dir);
        assert!(report.project_types.contains(&"python".to_string()));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn generic_fallback_no_language_sets_offered() {
        let dir = unique_tempdir("generic");
        fs::write(dir.join("README.md"), "# anything\n").unwrap();
        let report = audit_path(&dir);
        assert!(
            report.project_types.is_empty(),
            "expected no project types detected, got {:?}",
            report.project_types
        );
        let ids: Vec<String> = report.rule_sets.iter().map(|r| r.id.clone()).collect();
        assert!(!ids.contains(&"node".to_string()));
        assert!(!ids.contains(&"python".to_string()));
        assert!(!ids.contains(&"rust".to_string()));
        // Universal env is still offered
        assert!(ids.contains(&"universal_env".to_string()));
        let _ = fs::remove_dir_all(&dir);
    }

    // ── Apply: create / append / idempotent ────────────────────────────────

    #[test]
    fn create_missing_gitignore() {
        let dir = unique_tempdir("create-missing");
        let report = audit_path(&dir);
        assert!(!report.gitignore_exists);
        let selections = select_all_default_on(&report);
        let applied = apply(GitignoreApplyArgs {
            path: dir.display().to_string(),
            selections,
        })
        .unwrap();
        assert!(applied.created_new_file);
        assert!(applied.lines_added > 0);
        let body = fs::read_to_string(dir.join(".gitignore")).unwrap();
        assert!(body.contains(".env\n"));
        assert!(body.contains("!.env.example"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn append_to_existing_file() {
        let dir = unique_tempdir("append-existing");
        fs::write(dir.join(".gitignore"), "# my project\nlocal-only/\n").unwrap();
        let report = audit_path(&dir);
        assert!(report.gitignore_exists);
        let selections = select_all_default_on(&report);
        let applied = apply(GitignoreApplyArgs {
            path: dir.display().to_string(),
            selections,
        })
        .unwrap();
        assert!(!applied.created_new_file);
        let body = fs::read_to_string(dir.join(".gitignore")).unwrap();
        // existing content is preserved
        assert!(body.contains("# my project"));
        assert!(body.contains("local-only/"));
        // additions present
        assert!(body.contains(".env\n"));
        assert!(body.contains("*.holster-tmp\n"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn idempotent_second_run_adds_nothing() {
        let dir = unique_tempdir("idempotent");
        let report = audit_path(&dir);
        let selections = select_all_default_on(&report);
        // Run 1
        apply(GitignoreApplyArgs {
            path: dir.display().to_string(),
            selections: selections.clone(),
        })
        .unwrap();
        let after_first = fs::read_to_string(dir.join(".gitignore")).unwrap();
        // Run 2
        let report2 = audit_path(&dir);
        let selections2 = select_all_default_on(&report2);
        let applied2 = apply(GitignoreApplyArgs {
            path: dir.display().to_string(),
            selections: selections2,
        })
        .unwrap();
        assert_eq!(applied2.lines_added, 0, "second run should add nothing");
        let after_second = fs::read_to_string(dir.join(".gitignore")).unwrap();
        assert_eq!(
            after_first, after_second,
            "file content should be byte-identical after second run"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn preserve_existing_content() {
        let dir = unique_tempdir("preserve");
        let original = "# top of file\n# notes\nignore-me-only\n# bottom\n";
        fs::write(dir.join(".gitignore"), original).unwrap();
        let report = audit_path(&dir);
        let selections = select_all_default_on(&report);
        apply(GitignoreApplyArgs {
            path: dir.display().to_string(),
            selections,
        })
        .unwrap();
        let body = fs::read_to_string(dir.join(".gitignore")).unwrap();
        assert!(
            body.starts_with(original),
            "expected new body to start with original content; got:\n{body}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn handles_missing_trailing_newline() {
        let dir = unique_tempdir("no-trailing-nl");
        // Note: NO trailing newline
        fs::write(dir.join(".gitignore"), "node_modules").unwrap();
        let report = audit_path(&dir);
        let selections = select_all_default_on(&report);
        apply(GitignoreApplyArgs {
            path: dir.display().to_string(),
            selections,
        })
        .unwrap();
        let body = fs::read_to_string(dir.join(".gitignore")).unwrap();
        assert!(body.starts_with("node_modules\n"));
        // .env should be appended cleanly, not glued onto "node_modules"
        assert!(body.contains("\n.env\n") || body.contains("\n# Holster"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn does_not_duplicate_env_example_negation() {
        let dir = unique_tempdir("env-example-dedupe");
        fs::write(dir.join(".gitignore"), "!.env.example\n").unwrap();
        let report = audit_path(&dir);
        // Confirm the audit marks it already_present
        let universal = report
            .rule_sets
            .iter()
            .find(|r| r.id == "universal_env")
            .unwrap();
        let env_example = universal
            .rules
            .iter()
            .find(|r| r.line == "!.env.example")
            .unwrap();
        assert!(env_example.already_present);
        let selections = select_all_default_on(&report);
        apply(GitignoreApplyArgs {
            path: dir.display().to_string(),
            selections,
        })
        .unwrap();
        let body = fs::read_to_string(dir.join(".gitignore")).unwrap();
        let count = body.matches("!.env.example").count();
        assert_eq!(count, 1, "expected !.env.example exactly once, got {count}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn does_not_duplicate_already_present_rules_across_sets() {
        let dir = unique_tempdir("dedupe-cross");
        fs::write(dir.join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();
        fs::write(dir.join(".gitignore"), "target/\n.env\n").unwrap();
        let report = audit_path(&dir);
        let selections = select_all_default_on(&report);
        apply(GitignoreApplyArgs {
            path: dir.display().to_string(),
            selections,
        })
        .unwrap();
        let body = fs::read_to_string(dir.join(".gitignore")).unwrap();
        assert_eq!(body.matches("target/").count(), 1);
        assert_eq!(
            body.matches("\n.env\n").count() + body.matches("^.env\n").count(),
            1
        );
        let _ = fs::remove_dir_all(&dir);
    }

    // ── Apply: refusing edge cases ─────────────────────────────────────────

    #[test]
    fn empty_selections_with_no_existing_file_does_not_create() {
        let dir = unique_tempdir("empty-sel-no-file");
        let applied = apply(GitignoreApplyArgs {
            path: dir.display().to_string(),
            selections: vec![],
        })
        .unwrap();
        assert!(!applied.created_new_file);
        assert_eq!(applied.lines_added, 0);
        assert!(!dir.join(".gitignore").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn refuses_unknown_rule_set_id() {
        let dir = unique_tempdir("unknown-set");
        let applied = apply(GitignoreApplyArgs {
            path: dir.display().to_string(),
            selections: vec![RuleSetSelection {
                rule_set_id: "totally-made-up".into(),
                lines: vec![".env".into()],
            }],
        })
        .unwrap();
        assert_eq!(applied.lines_added, 0);
        assert!(!dir.join(".gitignore").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn refuses_lines_outside_canonical_set_membership() {
        let dir = unique_tempdir("foreign-line");
        // Try to sneak in an arbitrary path through universal_env
        let _applied = apply(GitignoreApplyArgs {
            path: dir.display().to_string(),
            selections: vec![RuleSetSelection {
                rule_set_id: "universal_env".into(),
                lines: vec![
                    ".env".into(),
                    "/home/admin/.ssh/id_rsa".into(), // not in canonical set
                ],
            }],
        })
        .unwrap();
        let body = fs::read_to_string(dir.join(".gitignore")).unwrap_or_default();
        assert!(body.contains(".env\n"));
        assert!(
            !body.contains("/home/admin/.ssh/id_rsa"),
            "frontend-injected non-canonical line must not be written"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    // ── Path safety ────────────────────────────────────────────────────────

    #[test]
    fn refuses_filesystem_root() {
        let r = audit(GitignoreAuditArgs { path: "/".into() });
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("filesystem root"));
    }

    #[test]
    fn refuses_home_directly() {
        let Some(home_path) = dirs::home_dir() else {
            return;
        };
        let home = home_path.display().to_string();
        let r = audit(GitignoreAuditArgs { path: home });
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("HOME"));
    }

    #[test]
    fn refuses_nonexistent_path() {
        let r = audit(GitignoreAuditArgs {
            path: "/this-path-should-not-exist-zzz".into(),
        });
        assert!(r.is_err());
    }

    // ── Audit is read-only ─────────────────────────────────────────────────

    #[test]
    fn audit_does_not_create_gitignore() {
        let dir = unique_tempdir("audit-readonly");
        let _ = audit_path(&dir);
        assert!(
            !dir.join(".gitignore").exists(),
            "audit must not create .gitignore"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    // ── Dedupe semantics: trim whitespace, ignore blank lines ──────────────

    #[test]
    fn dedupe_treats_trimmed_lines_as_equal() {
        let dir = unique_tempdir("trim-dedupe");
        fs::write(dir.join(".gitignore"), "  .env  \n\n").unwrap();
        let selections = select_all_default_on(&audit_path(&dir));
        apply(GitignoreApplyArgs {
            path: dir.display().to_string(),
            selections,
        })
        .unwrap();
        let body = fs::read_to_string(dir.join(".gitignore")).unwrap();
        // Should not add a second .env line just because the original had
        // surrounding spaces.
        let env_lines: Vec<&str> = body.lines().filter(|l| l.trim() == ".env").collect();
        assert_eq!(env_lines.len(), 1, "expected one .env line, body:\n{body}");
        let _ = fs::remove_dir_all(&dir);
    }

    // ── Atomic write: temp file gone after success ─────────────────────────

    #[test]
    fn atomic_write_leaves_no_temp_behind() {
        let dir = unique_tempdir("atomic");
        let selections = select_all_default_on(&audit_path(&dir));
        apply(GitignoreApplyArgs {
            path: dir.display().to_string(),
            selections,
        })
        .unwrap();
        let temp = dir.join(format!(".gitignore{TEMP_SUFFIX}"));
        assert!(!temp.exists(), "temp should be cleaned up after rename");
        let _ = fs::remove_dir_all(&dir);
    }

    // ── Multi-language project: rule sets cumulative ───────────────────────

    #[test]
    fn multi_language_project_offers_all_detected_sets() {
        let dir = unique_tempdir("multi-lang");
        fs::write(dir.join("Cargo.toml"), "[package]\nname=\"x\"\n").unwrap();
        fs::write(dir.join("package.json"), "{}").unwrap();
        let report = audit_path(&dir);
        assert!(report.project_types.contains(&"node".to_string()));
        assert!(report.project_types.contains(&"rust".to_string()));
        let ids: Vec<String> = report.rule_sets.iter().map(|r| r.id.clone()).collect();
        assert!(ids.contains(&"node".to_string()));
        assert!(ids.contains(&"rust".to_string()));
        // Python should NOT show up — no markers
        assert!(!ids.contains(&"python".to_string()));
        let _ = fs::remove_dir_all(&dir);
    }

    // ── Universal env locked-on flag is true ───────────────────────────────

    #[test]
    fn universal_env_is_locked_on() {
        let dir = unique_tempdir("locked");
        let report = audit_path(&dir);
        let universal = report
            .rule_sets
            .iter()
            .find(|r| r.id == "universal_env")
            .unwrap();
        assert!(universal.locked_on);
        assert!(universal.default_on);
        let _ = fs::remove_dir_all(&dir);
    }

    // ── macos_ide is opt-in (default off) ──────────────────────────────────

    #[test]
    fn macos_ide_is_default_off() {
        let dir = unique_tempdir("macos-off");
        let report = audit_path(&dir);
        let macos = report
            .rule_sets
            .iter()
            .find(|r| r.id == "macos_ide")
            .unwrap();
        assert!(!macos.default_on);
    }

    // ── No-secrets-in-output guarantee ─────────────────────────────────────

    #[test]
    fn audit_report_does_not_carry_file_contents_beyond_gitignore() {
        // The audit report exposes existing_line_count but NOT the actual
        // existing lines. This test enforces that contract — if the type
        // ever grows a field that carries .gitignore body content (or any
        // other file content), this test fails so we re-evaluate.
        let dir = unique_tempdir("no-leak");
        fs::write(dir.join(".gitignore"), "secret-folder/\n").unwrap();
        let report = audit_path(&dir);
        let json = serde_json::to_string(&report).unwrap();
        assert!(
            !json.contains("secret-folder"),
            "audit JSON must not echo existing file content; got: {json}"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
