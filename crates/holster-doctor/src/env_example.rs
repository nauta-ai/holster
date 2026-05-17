//! Holster M3.1 T3.1.1 — `.env.example` generator.
//!
//! Produces a committable `.env.example` template from one of two sources:
//!
//!   1. **Vault metadata** — a list of vault key ids; Holster derives env
//!      var NAMES from each key's provider + label. Optional Holster
//!      source comments reference provider/label, never values.
//!   2. **Existing `.env*` file** — the file is parsed line-by-line.
//!      For each `KEY=…` line, Holster extracts only `KEY` (the parser
//!      stops at the first `=` and discards the remainder of the line).
//!      The source path's basename must match `.env*`.
//!
//! Hard guardrails (per the M3.1 scope doc):
//!   - **Always-redacted output.** The apply body contains lines of the
//!     shape `NAME=` (trailing equals, empty value). Any line whose
//!     name contains `=` is rejected before write.
//!   - **Never overwrites silently.** Apply refuses if the target file
//!     exists and `overwrite=false`.
//!   - **Atomic write** via `<filename>.holster-tmp` + rename.
//!   - **chmod 0644.** The output is intentionally committable — 0600
//!     would surprise users who try to share it.
//!   - **Refuses target paths inside `.git/`, `node_modules/`, etc.**
//!     Same skip-dir list M3 repo scanner uses.
//!   - **Audit log** entry written to `runtime-export-audit.jsonl` with
//!     `kind: "env_example_generated"` — names + path only, never any
//!     value content.
//!   - **Path safety.** Same `refuse_dangerous_root` rules as
//!     repo_scanner / gitignore_helper.

use std::path::{Path, PathBuf};

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

use serde::{Deserialize, Serialize};

const TEMP_SUFFIX: &str = ".holster-tmp";
const DEFAULT_FILENAME: &str = ".env.example";
const MAX_SOURCE_FILE_SIZE: u64 = 5_000_000; // 5 MB

/// Project subdirectories we refuse to write into. Mirrors the repo
/// scanner's always-skip list. Reading a `.env*` file from inside one
/// of these is also refused — almost certainly a mis-pick.
const ALWAYS_SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".next",
    "vendor",
    ".venv",
    "venv",
    "__pycache__",
    ".pytest_cache",
    ".cache",
    ".turbo",
    ".pnpm-store",
];

// ── Public types ────────────────────────────────────────────────────────────

/// One line in the proposed `.env.example`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EnvExampleLine {
    /// Env var NAME only. No `=`, no value.
    pub name: String,
    /// Optional Holster source comment (e.g.,
    /// `"stored in Holster as openai / Personal"`). May be `None`.
    /// Always written above the `NAME=` line, prefixed with `# `.
    pub comment: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct EnvExampleProposal {
    /// `"vault"` or `"env_file"`.
    pub source_kind: String,
    /// Human-readable label for the UI.
    pub source_label: String,
    /// The lines that will be written, in the order they were proposed.
    pub lines: Vec<EnvExampleLine>,
    /// How many input items were successfully turned into lines.
    pub parsed_count: usize,
    /// How many input items were skipped (malformed, blank, comment).
    /// Pure blank lines and comment-only lines do NOT count toward this.
    pub skipped_count: usize,
}

#[derive(Deserialize, Debug)]
pub struct EnvExampleFromFileArgs {
    pub source_path: String,
}

#[derive(Deserialize, Debug)]
pub struct EnvExampleApplyArgs {
    pub target_dir: String,
    /// Defaults to `.env.example` when `None` or empty.
    pub filename: Option<String>,
    pub lines: Vec<EnvExampleLine>,
    /// If `false` and the target exists, apply refuses with an error.
    /// The frontend must re-call with `true` after explicit confirmation.
    pub overwrite: bool,
    /// If true, prepend an explanatory header comment block to the file.
    /// Defaults to true via the frontend.
    pub include_header_comments: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct EnvExampleApplyReport {
    pub target_path: String,
    pub file_existed: bool,
    pub overwrote: bool,
    pub line_count: usize,
    pub audit_log_path: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct EnvExamplePreviewBody {
    pub body: String,
}

// ── Public functions ────────────────────────────────────────────────────────

/// Parse a `.env*`-style file and return only env-var NAMES. The parser
/// stops at the first `=` of each line; everything to the right is
/// discarded and never returned. Lines that don't parse as a `KEY=…`
/// pattern are skipped (counted in `skipped_count`).
///
/// Read-only, no vault required. The basename validation lives in the
/// public Tauri command wrapper.
pub fn parse_env_file(content: &str) -> ParseResult {
    let mut names = Vec::new();
    let mut skipped = 0usize;

    for raw_line in content.lines() {
        let mut line = raw_line.trim_start_matches('\u{FEFF}'); // strip BOM
        line = line.trim();

        // Pure blank or comment-only — expected, not a "skip"
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Strip optional `export ` prefix used in shell-style env files
        if let Some(rest) = line.strip_prefix("export ") {
            line = rest.trim_start();
        }

        // Find the first `=` and take only what comes before it.
        let Some(eq_idx) = line.find('=') else {
            // Line has no `=` and isn't a comment/blank — malformed
            skipped += 1;
            continue;
        };
        let name = line[..eq_idx].trim().to_string();

        if !is_valid_env_var_name(&name) {
            skipped += 1;
            continue;
        }

        // Dedupe: if the same name appears twice in the source, only
        // record it once.
        if names.iter().any(|n: &String| n == &name) {
            continue;
        }
        names.push(name);
    }

    ParseResult { names, skipped }
}

pub struct ParseResult {
    pub names: Vec<String>,
    pub skipped: usize,
}

/// Validate that a string is a legal env var name:
///   - non-empty
///   - first char is letter or underscore (POSIX)
///   - all chars are ASCII letter / digit / underscore
pub fn is_valid_env_var_name(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Build the file body from a list of lines + an optional header.
pub fn build_apply_body(lines: &[EnvExampleLine], include_header_comments: bool) -> String {
    let mut out = String::new();
    if include_header_comments {
        out.push_str(
            "# Generated by Holster — committable template, no values.\n\
             # Each variable below is required by this project. Set values\n\
             # in .env.local (which should be gitignored — see Holster's\n\
             # gitignore helper) and never commit them.\n\n",
        );
    }
    for (i, line) in lines.iter().enumerate() {
        if let Some(comment) = &line.comment {
            // Each comment line: prefix with "# " and strip embedded
            // newlines defensively (validate_lines should already have
            // refused them, but belt-and-suspenders).
            for sub in comment.split(['\n', '\r']) {
                let sub = sub.trim();
                if sub.is_empty() {
                    continue;
                }
                out.push_str("# ");
                out.push_str(sub);
                out.push('\n');
            }
        }
        out.push_str(&line.name);
        out.push('=');
        out.push('\n');
        if i + 1 < lines.len() {
            out.push('\n');
        }
    }
    out
}

/// Validate a basename matches the `.env*` pattern (e.g., `.env`,
/// `.env.local`, `.env.production`). Used as a guard before reading.
pub fn is_env_filename(basename: &str) -> bool {
    if basename.is_empty() {
        return false;
    }
    if basename == ".env" {
        return true;
    }
    basename.starts_with(".env.") || basename.ends_with(".env")
}

/// Validate a target filename for `.env.example` writes. Allowed:
///   - `.env.example` exactly
///   - `*.env.example` (e.g., `prod.env.example`)
///
/// REJECTED on purpose: `.env`, `.env.local`, anything that could
/// accidentally overwrite a real runtime env file.
pub fn is_safe_env_example_filename(filename: &str) -> bool {
    if filename.trim().is_empty() {
        return false;
    }
    let path = Path::new(filename);
    if path.components().count() != 1 {
        return false;
    }
    if filename == ".env.example" {
        return true;
    }
    if filename.ends_with(".env.example") && filename != ".env.example" {
        // require a non-empty stem before the `.env.example`
        let stem = &filename[..filename.len() - ".env.example".len()];
        return !stem.is_empty();
    }
    false
}

/// Validate every line in a proposed apply payload. Catches a hostile
/// frontend that tries to inject a value into the name field or sneak
/// newlines into a comment.
pub fn validate_lines(lines: &[EnvExampleLine]) -> Result<(), String> {
    for (i, line) in lines.iter().enumerate() {
        if !is_valid_env_var_name(&line.name) {
            return Err(format!(
                "line {}: '{}' is not a valid env var name",
                i + 1,
                line.name
            ));
        }
        if let Some(c) = &line.comment {
            // Reject control bytes that would corrupt the file layout.
            // Newlines are OK in the comment — we'll split on them in
            // build_apply_body — but NUL is never OK.
            if c.contains('\0') {
                return Err(format!("line {}: comment contains a NUL byte", i + 1));
            }
        }
    }
    Ok(())
}

/// Path safety: refuse `/`, `$HOME`, and common system-level paths.
pub fn refuse_dangerous_root(canonical: &Path) -> Result<(), String> {
    if canonical == Path::new("/") {
        return Err("refusing to write into filesystem root '/'".into());
    }
    if let Some(home_path) = dirs::home_dir() {
        if canonical == home_path.as_path() {
            return Err(format!(
                "refusing to write into $HOME ({}) directly. Pick a project subdirectory.",
                home_path.display()
            ));
        }
    }
    for forbidden in ["/etc", "/var", "/usr", "/System", "/Library", "/private"] {
        if canonical == Path::new(forbidden) {
            return Err(format!(
                "refusing to write into system path {forbidden}. Pick a project directory."
            ));
        }
    }
    Ok(())
}

/// Returns true if any path component matches an always-skip dir.
pub fn path_in_skip_dir(p: &Path) -> bool {
    let skip: std::collections::HashSet<&str> = ALWAYS_SKIP_DIRS.iter().copied().collect();
    p.components().any(|c| {
        if let std::path::Component::Normal(name) = c {
            name.to_str().map(|n| skip.contains(n)).unwrap_or(false)
        } else {
            false
        }
    })
}

/// Read a `.env*` file from disk and return the list of extracted
/// env-var names. Refuses non-`.env*` basenames. Refuses files larger
/// than MAX_SOURCE_FILE_SIZE.
pub fn read_env_file_for_proposal(
    args: &EnvExampleFromFileArgs,
) -> Result<EnvExampleProposal, String> {
    let p = Path::new(args.source_path.trim());
    if !p.exists() {
        return Err(format!("source path does not exist: {}", p.display()));
    }
    if !p.is_file() {
        return Err(format!("source path is not a file: {}", p.display()));
    }
    let basename = p
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "source path has no filename".to_string())?;
    if !is_env_filename(basename) {
        return Err(format!(
            "source filename {basename:?} does not match `.env*` pattern. Refusing to read."
        ));
    }
    let metadata = p
        .metadata()
        .map_err(|e| format!("could not stat source file: {e}"))?;
    if metadata.len() > MAX_SOURCE_FILE_SIZE {
        return Err(format!(
            "source file is larger than {} bytes; refusing to read",
            MAX_SOURCE_FILE_SIZE
        ));
    }
    let content =
        std::fs::read_to_string(p).map_err(|e| format!("could not read source file: {e}"))?;
    let parsed = parse_env_file(&content);
    drop(content); // release the buffer that contained values

    let lines = parsed
        .names
        .iter()
        .map(|n| EnvExampleLine {
            name: n.clone(),
            comment: None,
        })
        .collect::<Vec<_>>();

    Ok(EnvExampleProposal {
        source_kind: "env_file".into(),
        source_label: p.display().to_string(),
        parsed_count: lines.len(),
        skipped_count: parsed.skipped,
        lines,
    })
}

/// Apply a proposed `.env.example` to disk. Atomic write, append-audit.
/// `audit_writer` is an injection point so the Tauri command can wire
/// the existing audit-log path (lives in the OS app-data dir, owned by
/// lib.rs).
pub fn apply_to_disk(
    args: &EnvExampleApplyArgs,
    audit_writer: &mut dyn FnMut(&serde_json::Value) -> Result<Option<String>, String>,
) -> Result<EnvExampleApplyReport, String> {
    let target_dir = expand_home(args.target_dir.trim());
    if !target_dir.exists() {
        return Err(format!(
            "target folder does not exist: {}",
            target_dir.display()
        ));
    }
    if !target_dir.is_dir() {
        return Err(format!("target is not a folder: {}", target_dir.display()));
    }
    let canonical_dir = target_dir
        .canonicalize()
        .map_err(|e| format!("could not canonicalize target folder: {e}"))?;
    refuse_dangerous_root(&canonical_dir)?;
    if path_in_skip_dir(&canonical_dir) {
        return Err(
            "target folder is inside a skip directory (.git, node_modules, etc.). \
             Pick a project root."
                .into(),
        );
    }

    let filename = args
        .filename
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_FILENAME)
        .to_string();
    if !is_safe_env_example_filename(&filename) {
        return Err(format!(
            "filename {filename:?} is not a safe `.env.example` name. \
             Allowed: `.env.example` or `<stem>.env.example`."
        ));
    }
    let target_path = canonical_dir.join(&filename);
    if path_in_skip_dir(&target_path) {
        return Err("target path is inside a skip directory. Refusing to write.".into());
    }

    if args.lines.is_empty() {
        return Err("no lines selected — nothing to write".into());
    }
    validate_lines(&args.lines)?;

    let file_existed = target_path.exists();
    if file_existed && !args.overwrite {
        return Err(format!(
            "target {} already exists. Re-call with overwrite=true to replace it.",
            target_path.display()
        ));
    }

    let body = build_apply_body(&args.lines, args.include_header_comments);

    write_atomic(&target_path, &body)?;
    set_committable_perms(&target_path);

    let audit_payload = serde_json::json!({
        "ts": chrono::Utc::now().to_rfc3339(),
        "kind": "env_example_generated",
        "target_path": target_path.display().to_string(),
        "filename": filename,
        "line_count": args.lines.len(),
        "names": args.lines.iter().map(|l| l.name.clone()).collect::<Vec<_>>(),
        "overwrote_existing": file_existed,
    });
    let audit_log_path = audit_writer(&audit_payload)?;

    Ok(EnvExampleApplyReport {
        target_path: target_path.display().to_string(),
        file_existed,
        overwrote: file_existed,
        line_count: args.lines.len(),
        audit_log_path,
    })
}

// ── Internal helpers ────────────────────────────────────────────────────────

fn write_atomic(target: &Path, body: &str) -> Result<(), String> {
    let filename = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "target path has no filename".to_string())?;
    let temp_path = target.with_file_name(format!("{filename}{TEMP_SUFFIX}"));
    if let Err(e) = std::fs::write(&temp_path, body) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!("could not write .env.example temp: {e}"));
    }
    if let Err(e) = std::fs::rename(&temp_path, target) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!(
            "could not rename .env.example temp into place: {e}"
        ));
    }
    Ok(())
}

fn set_committable_perms(_target: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
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
        p.push(format!("holster-envex-{label}-{nanos}-{n}"));
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn null_audit() -> impl FnMut(&serde_json::Value) -> Result<Option<String>, String> {
        |_| Ok(None)
    }

    // ── parse_env_file ─────────────────────────────────────────────────────

    #[test]
    fn parse_basic_env_extracts_names_only() {
        let input = "FOO=hello\nBAR=world\n";
        let p = parse_env_file(input);
        assert_eq!(p.names, vec!["FOO".to_string(), "BAR".to_string()]);
        assert_eq!(p.skipped, 0);
    }

    #[test]
    fn parse_stops_at_first_equals_when_value_contains_equals() {
        // The CRITICAL test: if a value contains `=`, the parser must
        // not include any of it in the extracted name.
        let input = "WEIRD=this=value=has=many=equals=signs\n";
        let p = parse_env_file(input);
        assert_eq!(p.names, vec!["WEIRD".to_string()]);
    }

    #[test]
    fn parse_handles_empty_value() {
        let input = "EMPTY=\n";
        let p = parse_env_file(input);
        assert_eq!(p.names, vec!["EMPTY".to_string()]);
    }

    #[test]
    fn parse_skips_blanks_and_comments_without_counting_them() {
        let input = "\n# this is a comment\n\n   \nFOO=bar\n# another\n";
        let p = parse_env_file(input);
        assert_eq!(p.names, vec!["FOO".to_string()]);
        assert_eq!(p.skipped, 0);
    }

    #[test]
    fn parse_skips_lines_without_equals() {
        let input = "this-is-not-an-env-line\nFOO=bar\n";
        let p = parse_env_file(input);
        assert_eq!(p.names, vec!["FOO".to_string()]);
        assert_eq!(p.skipped, 1);
    }

    #[test]
    fn parse_handles_export_prefix() {
        let input = "export FOO=bar\nexport BAZ=qux\n";
        let p = parse_env_file(input);
        assert_eq!(p.names, vec!["FOO".to_string(), "BAZ".to_string()]);
    }

    #[test]
    fn parse_skips_invalid_var_names() {
        // 123FOO starts with a digit — invalid POSIX env var name
        // BAR-DASH contains a dash — invalid
        let input = "123FOO=bar\nBAR-DASH=val\nVALID_ONE=ok\n";
        let p = parse_env_file(input);
        assert_eq!(p.names, vec!["VALID_ONE".to_string()]);
        assert_eq!(p.skipped, 2);
    }

    #[test]
    fn parse_handles_dos_line_endings() {
        let input = "FOO=bar\r\nBAZ=qux\r\n";
        let p = parse_env_file(input);
        assert_eq!(p.names, vec!["FOO".to_string(), "BAZ".to_string()]);
    }

    #[test]
    fn parse_strips_bom() {
        let input = "\u{FEFF}FOO=bar\n";
        let p = parse_env_file(input);
        assert_eq!(p.names, vec!["FOO".to_string()]);
    }

    #[test]
    fn parse_dedupes_repeated_names() {
        let input = "FOO=first\nFOO=second\n";
        let p = parse_env_file(input);
        assert_eq!(p.names, vec!["FOO".to_string()]);
    }

    #[test]
    fn parse_trims_whitespace_around_name() {
        let input = "  FOO  =bar\n";
        let p = parse_env_file(input);
        assert_eq!(p.names, vec!["FOO".to_string()]);
    }

    // ── is_valid_env_var_name ──────────────────────────────────────────────

    #[test]
    fn valid_names_pass() {
        assert!(is_valid_env_var_name("FOO"));
        assert!(is_valid_env_var_name("FOO_BAR"));
        assert!(is_valid_env_var_name("_PRIVATE"));
        assert!(is_valid_env_var_name("FOO123"));
    }

    #[test]
    fn invalid_names_fail() {
        assert!(!is_valid_env_var_name(""));
        assert!(!is_valid_env_var_name("123FOO"));
        assert!(!is_valid_env_var_name("FOO-BAR"));
        assert!(!is_valid_env_var_name("foo.bar"));
        assert!(!is_valid_env_var_name(" FOO"));
        assert!(!is_valid_env_var_name("FOO BAR"));
    }

    // ── is_safe_env_example_filename ───────────────────────────────────────

    #[test]
    fn safe_example_filename_accepts_canonical_and_prefixed() {
        assert!(is_safe_env_example_filename(".env.example"));
        assert!(is_safe_env_example_filename("prod.env.example"));
        assert!(is_safe_env_example_filename("staging.env.example"));
    }

    #[test]
    fn safe_example_filename_refuses_real_env_filenames() {
        // The whole point of this validator: don't let the user
        // accidentally overwrite their actual runtime env file.
        assert!(!is_safe_env_example_filename(".env"));
        assert!(!is_safe_env_example_filename(".env.local"));
        assert!(!is_safe_env_example_filename("prod.env"));
    }

    #[test]
    fn safe_example_filename_refuses_path_traversal() {
        assert!(!is_safe_env_example_filename("../.env.example"));
        assert!(!is_safe_env_example_filename("/etc/.env.example"));
        assert!(!is_safe_env_example_filename("subdir/.env.example"));
    }

    #[test]
    fn safe_example_filename_refuses_empty_or_unrelated() {
        assert!(!is_safe_env_example_filename(""));
        assert!(!is_safe_env_example_filename("   "));
        assert!(!is_safe_env_example_filename("README.md"));
    }

    // ── is_env_filename (source-file basename validator) ───────────────────

    #[test]
    fn env_filename_accepts_dot_env_variants() {
        assert!(is_env_filename(".env"));
        assert!(is_env_filename(".env.local"));
        assert!(is_env_filename(".env.production"));
        assert!(is_env_filename("prod.env"));
    }

    #[test]
    fn env_filename_refuses_unrelated() {
        assert!(!is_env_filename("config.json"));
        assert!(!is_env_filename(""));
        assert!(!is_env_filename("envfile"));
    }

    // ── build_apply_body ───────────────────────────────────────────────────

    #[test]
    fn body_emits_name_equals_with_no_value() {
        let lines = vec![
            EnvExampleLine {
                name: "FOO".into(),
                comment: None,
            },
            EnvExampleLine {
                name: "BAR".into(),
                comment: None,
            },
        ];
        let body = build_apply_body(&lines, false);
        assert!(body.contains("FOO=\n"));
        assert!(body.contains("BAR=\n"));
        assert!(
            !body.contains("FOO=fake")
                && !body.contains("FOO=value")
                && !body.contains("FOO=anything"),
            "body must contain only the trailing equals"
        );
    }

    #[test]
    fn body_includes_optional_comments_above_each_line() {
        let lines = vec![EnvExampleLine {
            name: "OPENAI_API_KEY".into(),
            comment: Some("stored in Holster as openai / Personal".into()),
        }];
        let body = build_apply_body(&lines, false);
        assert!(body.contains("# stored in Holster as openai / Personal\n"));
        assert!(body.contains("OPENAI_API_KEY=\n"));
        assert!(
            body.find("# stored").unwrap() < body.find("OPENAI_API_KEY=").unwrap(),
            "comment must come before the env line"
        );
    }

    #[test]
    fn body_with_header_includes_explanatory_block() {
        let lines = vec![EnvExampleLine {
            name: "FOO".into(),
            comment: None,
        }];
        let body = build_apply_body(&lines, true);
        assert!(body.contains("Generated by Holster"));
        assert!(body.contains("never commit"));
    }

    #[test]
    fn body_splits_multiline_comments_into_multiple_comment_lines() {
        let lines = vec![EnvExampleLine {
            name: "FOO".into(),
            comment: Some("line one\nline two".into()),
        }];
        let body = build_apply_body(&lines, false);
        assert!(body.contains("# line one\n"));
        assert!(body.contains("# line two\n"));
    }

    // ── validate_lines ─────────────────────────────────────────────────────

    #[test]
    fn validate_rejects_invalid_name() {
        let lines = vec![EnvExampleLine {
            name: "BAD-NAME".into(),
            comment: None,
        }];
        let r = validate_lines(&lines);
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("not a valid env var"));
    }

    #[test]
    fn validate_rejects_name_with_equals() {
        // A hostile frontend could try to sneak `FOO=secretvalue` into
        // the name field. is_valid_env_var_name rejects `=` so this
        // fails.
        let lines = vec![EnvExampleLine {
            name: "FOO=sk-FAKE-injected-value".into(),
            comment: None,
        }];
        assert!(validate_lines(&lines).is_err());
    }

    #[test]
    fn validate_rejects_comment_with_nul() {
        let lines = vec![EnvExampleLine {
            name: "FOO".into(),
            comment: Some("contains\0NUL".into()),
        }];
        assert!(validate_lines(&lines).is_err());
    }

    #[test]
    fn validate_passes_clean_lines() {
        let lines = vec![EnvExampleLine {
            name: "VALID".into(),
            comment: Some("a fine comment".into()),
        }];
        assert!(validate_lines(&lines).is_ok());
    }

    // ── apply_to_disk ──────────────────────────────────────────────────────

    #[test]
    fn apply_writes_file_with_zero_values() {
        let dir = unique_tempdir("apply-basic");
        let mut audit = null_audit();
        let r = apply_to_disk(
            &EnvExampleApplyArgs {
                target_dir: dir.display().to_string(),
                filename: None,
                lines: vec![
                    EnvExampleLine {
                        name: "FOO".into(),
                        comment: None,
                    },
                    EnvExampleLine {
                        name: "BAR".into(),
                        comment: Some("test comment".into()),
                    },
                ],
                overwrite: false,
                include_header_comments: true,
            },
            &mut audit,
        )
        .unwrap();
        assert!(!r.file_existed);
        assert!(!r.overwrote);
        assert_eq!(r.line_count, 2);

        let body = fs::read_to_string(dir.join(".env.example")).unwrap();
        assert!(body.contains("FOO=\n"));
        assert!(body.contains("BAR=\n"));
        assert!(body.contains("# test comment\n"));
        // No values anywhere
        for forbidden in ["FOO=foo", "FOO=bar", "FOO=anything", "FOO=val"] {
            assert!(
                !body.contains(forbidden),
                "body must not contain {forbidden}"
            );
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn apply_refuses_to_overwrite_without_flag() {
        let dir = unique_tempdir("apply-no-overwrite");
        let target = dir.join(".env.example");
        fs::write(&target, "EXISTING=value\n").unwrap();

        let mut audit = null_audit();
        let r = apply_to_disk(
            &EnvExampleApplyArgs {
                target_dir: dir.display().to_string(),
                filename: None,
                lines: vec![EnvExampleLine {
                    name: "FOO".into(),
                    comment: None,
                }],
                overwrite: false,
                include_header_comments: false,
            },
            &mut audit,
        );
        assert!(r.is_err(), "expected refusal when target exists");
        assert!(r.unwrap_err().contains("already exists"));

        // Original file untouched
        let body = fs::read_to_string(&target).unwrap();
        assert_eq!(body, "EXISTING=value\n");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn apply_overwrites_when_flag_set() {
        let dir = unique_tempdir("apply-overwrite");
        let target = dir.join(".env.example");
        fs::write(&target, "OLD=value\n").unwrap();

        let mut audit = null_audit();
        let r = apply_to_disk(
            &EnvExampleApplyArgs {
                target_dir: dir.display().to_string(),
                filename: None,
                lines: vec![EnvExampleLine {
                    name: "NEW".into(),
                    comment: None,
                }],
                overwrite: true,
                include_header_comments: false,
            },
            &mut audit,
        )
        .unwrap();
        assert!(r.file_existed);
        assert!(r.overwrote);

        let body = fs::read_to_string(&target).unwrap();
        assert!(body.contains("NEW=\n"));
        assert!(!body.contains("OLD=value"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn apply_refuses_inside_skip_dir() {
        let dir = unique_tempdir("apply-skip");
        let bad_dir = dir.join("node_modules");
        fs::create_dir_all(&bad_dir).unwrap();

        let mut audit = null_audit();
        let r = apply_to_disk(
            &EnvExampleApplyArgs {
                target_dir: bad_dir.display().to_string(),
                filename: None,
                lines: vec![EnvExampleLine {
                    name: "FOO".into(),
                    comment: None,
                }],
                overwrite: false,
                include_header_comments: false,
            },
            &mut audit,
        );
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("skip directory"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn apply_refuses_real_env_filename() {
        let dir = unique_tempdir("apply-real-env");
        let mut audit = null_audit();
        let r = apply_to_disk(
            &EnvExampleApplyArgs {
                target_dir: dir.display().to_string(),
                filename: Some(".env".into()), // deliberately wrong
                lines: vec![EnvExampleLine {
                    name: "FOO".into(),
                    comment: None,
                }],
                overwrite: false,
                include_header_comments: false,
            },
            &mut audit,
        );
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("safe `.env.example`"));
        // No file was created
        assert!(!dir.join(".env").exists());
        assert!(!dir.join(".env.example").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn apply_accepts_prefixed_env_example_filenames() {
        let dir = unique_tempdir("apply-prefix");
        let mut audit = null_audit();
        let _r = apply_to_disk(
            &EnvExampleApplyArgs {
                target_dir: dir.display().to_string(),
                filename: Some("prod.env.example".into()),
                lines: vec![EnvExampleLine {
                    name: "FOO".into(),
                    comment: None,
                }],
                overwrite: false,
                include_header_comments: false,
            },
            &mut audit,
        )
        .unwrap();
        assert!(dir.join("prod.env.example").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn apply_atomic_write_leaves_no_temp_behind() {
        let dir = unique_tempdir("apply-atomic");
        let mut audit = null_audit();
        let _r = apply_to_disk(
            &EnvExampleApplyArgs {
                target_dir: dir.display().to_string(),
                filename: None,
                lines: vec![EnvExampleLine {
                    name: "FOO".into(),
                    comment: None,
                }],
                overwrite: false,
                include_header_comments: false,
            },
            &mut audit,
        )
        .unwrap();
        let temp = dir.join(format!(".env.example{TEMP_SUFFIX}"));
        assert!(!temp.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn apply_sets_committable_perms_0644() {
        use std::os::unix::fs::PermissionsExt;
        let dir = unique_tempdir("apply-perms");
        let mut audit = null_audit();
        apply_to_disk(
            &EnvExampleApplyArgs {
                target_dir: dir.display().to_string(),
                filename: None,
                lines: vec![EnvExampleLine {
                    name: "FOO".into(),
                    comment: None,
                }],
                overwrite: false,
                include_header_comments: false,
            },
            &mut audit,
        )
        .unwrap();
        let mode = fs::metadata(dir.join(".env.example"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o644, "expected 0644, got {mode:o}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn apply_refuses_empty_lines() {
        let dir = unique_tempdir("apply-empty");
        let mut audit = null_audit();
        let r = apply_to_disk(
            &EnvExampleApplyArgs {
                target_dir: dir.display().to_string(),
                filename: None,
                lines: vec![],
                overwrite: false,
                include_header_comments: false,
            },
            &mut audit,
        );
        assert!(r.is_err());
        assert!(!dir.join(".env.example").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    // ── read_env_file_for_proposal ─────────────────────────────────────────

    #[test]
    fn read_refuses_non_env_basename() {
        let dir = unique_tempdir("read-bad-name");
        fs::write(dir.join("config.json"), "{}").unwrap();
        let r = read_env_file_for_proposal(&EnvExampleFromFileArgs {
            source_path: dir.join("config.json").display().to_string(),
        });
        assert!(r.is_err());
        assert!(r.unwrap_err().contains(".env*"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_extracts_names_from_real_env() {
        let dir = unique_tempdir("read-env");
        // Real env file with FAKE values that contain `=` characters
        // and assorted edge cases.
        let body = "OPENAI_API_KEY=sk-FAKE-equals=internal=value\n\
                    # a comment\n\
                    \n\
                    export ANTHROPIC_API_KEY=ant-FAKE-token\n\
                    GROQ_API_KEY=gsk_FAKE_token\n";
        fs::write(dir.join(".env.local"), body).unwrap();
        let p = read_env_file_for_proposal(&EnvExampleFromFileArgs {
            source_path: dir.join(".env.local").display().to_string(),
        })
        .unwrap();
        assert_eq!(
            p.lines.iter().map(|l| l.name.clone()).collect::<Vec<_>>(),
            vec![
                "OPENAI_API_KEY".to_string(),
                "ANTHROPIC_API_KEY".to_string(),
                "GROQ_API_KEY".to_string(),
            ]
        );
        let _ = fs::remove_dir_all(&dir);
    }

    // ── The CRITICAL security tests ────────────────────────────────────────

    #[test]
    fn proposal_from_real_env_file_never_carries_value_substring() {
        // Build a fake env file with a unique-marker fake secret, run
        // the from-file path, JSON-serialize the proposal, and assert
        // the marker doesn't appear anywhere in the JSON.
        let dir = unique_tempdir("proposal-no-leak");
        let unique_marker = "FAKEUNIQUEMARKER_12345_DO_NOT_LEAK";
        fs::write(
            dir.join(".env"),
            format!("OPENAI_API_KEY=sk-FAKE-{unique_marker}-tail\n"),
        )
        .unwrap();
        let proposal = read_env_file_for_proposal(&EnvExampleFromFileArgs {
            source_path: dir.join(".env").display().to_string(),
        })
        .unwrap();
        let json = serde_json::to_string(&proposal).unwrap();
        assert!(
            !json.contains(unique_marker),
            "LEAK: proposal JSON contained value marker {unique_marker}; full JSON: {json}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn applied_body_from_real_env_file_never_contains_value_substring() {
        // Same as above, but for the WRITTEN body. Build proposal,
        // hand-translate to apply args, write, then read the on-disk
        // file and assert the marker is absent.
        let dir_src = unique_tempdir("apply-no-leak-src");
        let dir_dst = unique_tempdir("apply-no-leak-dst");
        let unique_marker = "FAKEUNIQUEMARKER_apply_67890";
        fs::write(
            dir_src.join(".env"),
            format!(
                "OPENAI_API_KEY=sk-FAKE-{unique_marker}-tail\n\
                     ANTHROPIC_API_KEY=ant-FAKE-{unique_marker}-also\n"
            ),
        )
        .unwrap();
        let proposal = read_env_file_for_proposal(&EnvExampleFromFileArgs {
            source_path: dir_src.join(".env").display().to_string(),
        })
        .unwrap();

        let mut audit = null_audit();
        apply_to_disk(
            &EnvExampleApplyArgs {
                target_dir: dir_dst.display().to_string(),
                filename: None,
                lines: proposal.lines.clone(),
                overwrite: false,
                include_header_comments: true,
            },
            &mut audit,
        )
        .unwrap();

        let body = fs::read_to_string(dir_dst.join(".env.example")).unwrap();
        assert!(
            !body.contains(unique_marker),
            "LEAK: written .env.example contained value marker {unique_marker}; body:\n{body}"
        );
        // And confirm the names DID make it through
        assert!(body.contains("OPENAI_API_KEY=\n"));
        assert!(body.contains("ANTHROPIC_API_KEY=\n"));
        let _ = fs::remove_dir_all(&dir_src);
        let _ = fs::remove_dir_all(&dir_dst);
    }

    #[test]
    fn audit_payload_never_contains_value_substring() {
        // Even the audit log payload must not carry value content.
        let dir_src = unique_tempdir("audit-no-leak-src");
        let dir_dst = unique_tempdir("audit-no-leak-dst");
        let unique_marker = "FAKEUNIQUEMARKER_audit_AAAA";
        fs::write(
            dir_src.join(".env"),
            format!("OPENAI_API_KEY=sk-FAKE-{unique_marker}\n"),
        )
        .unwrap();
        let proposal = read_env_file_for_proposal(&EnvExampleFromFileArgs {
            source_path: dir_src.join(".env").display().to_string(),
        })
        .unwrap();

        let mut captured: Option<String> = None;
        {
            let mut audit_writer = |payload: &serde_json::Value| -> Result<Option<String>, String> {
                captured = Some(serde_json::to_string(payload).unwrap());
                Ok(Some("/dev/null".into()))
            };
            apply_to_disk(
                &EnvExampleApplyArgs {
                    target_dir: dir_dst.display().to_string(),
                    filename: None,
                    lines: proposal.lines.clone(),
                    overwrite: false,
                    include_header_comments: false,
                },
                &mut audit_writer,
            )
            .unwrap();
        }
        let audit_json = captured.expect("audit writer must have been called");
        assert!(
            !audit_json.contains(unique_marker),
            "LEAK: audit JSON contained value marker {unique_marker}; audit:\n{audit_json}"
        );
        // Audit MUST contain the name we wrote
        assert!(audit_json.contains("OPENAI_API_KEY"));
        let _ = fs::remove_dir_all(&dir_src);
        let _ = fs::remove_dir_all(&dir_dst);
    }
}
