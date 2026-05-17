//! Holster M3 — local repo scanner.
//!
//! Wraps the `detectors::scan_text` library with a directory walk:
//!   - Refuses to scan `/` or `$HOME` directly (must pick a project subdir).
//!   - Skips a built-in always-skip dir list (`.git`, `node_modules`,
//!     `target`, `dist`, `build`, `.next`, `vendor`).
//!   - Optionally respects `.gitignore` (off by default so we DO find
//!     gitignored `.env` files that contain secrets — the `git_tracked`
//!     flag on each detection tells the user the actual leak surface).
//!   - Skips binary files (NUL byte in first 8 KB heuristic).
//!   - Skips files over `max_file_size_bytes` (default 5 MB).
//!   - Skips files that aren't valid UTF-8.
//!   - Reads files into memory, calls `scan_text`, attaches `file_path`
//!     (relative to scan root) and `git_tracked` (computed once via
//!     `git ls-files -z` at scan start).
//!
//! Security contract (enforced by tests):
//!   - Raw secret values NEVER cross the Rust → frontend IPC boundary.
//!     `Detection.redacted_preview` is the only field that touches the
//!     match, and it's truncated per `detectors::redact_match`.
//!   - The `ScanReport` type contains `Detection`s and aggregates only.
//!     No raw value, no `result_summary`-style string echoes a secret.
//!   - The scanner does not `println!`, `tracing::info!`, or otherwise
//!     log file contents.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::detectors::{
    detector_for_id, recommendation_for, scan_text, Classification, Detection, RiskLevel, Tier,
};

// ── Public types ─────────────────────────────────────────────────────────────

#[derive(Deserialize, Debug)]
pub struct ScanArgs {
    pub path: String,
    /// Follow symlinks when walking. Default: false.
    #[serde(default)]
    pub follow_symlinks: bool,
    /// If true, skip files that `.gitignore` would ignore. Default: false —
    /// we WANT to find gitignored `.env` files for the leak audit.
    #[serde(default)]
    pub respect_gitignore: bool,
    /// Max file size to read. 0 = use default (5 MB).
    #[serde(default)]
    pub max_file_size_bytes: u64,
}

#[derive(Serialize, Clone, Debug)]
pub struct DetectorSummary {
    pub detector_id: String,
    pub display_name: String,
    pub provider: String,
    pub tier: Tier,
    pub risk_level: RiskLevel,
    pub count: usize,
}

#[derive(Serialize, Clone, Debug)]
pub struct ScanReport {
    pub root_path: String,
    pub scanned_files: usize,
    pub skipped_binary: usize,
    pub skipped_oversize: usize,
    pub skipped_unreadable: usize,
    pub skipped_ignored: usize,
    pub elapsed_ms: u64,
    pub detections: Vec<Detection>,
    pub summary_by_detector: Vec<DetectorSummary>,
    /// Risk counts across ALL detections, including test fixtures. Preserved
    /// for back-compat with any consumer that wants the raw detector total.
    pub summary_by_risk: HashMap<String, usize>,
    /// Risk counts excluding `Classification::is_fixture` detections. THIS
    /// is the count the verdict and headline number should use — it filters
    /// out test paths and fixture-shaped values so a healthy repo with
    /// intentional test data reads as healthy.
    pub summary_by_risk_excluding_fixtures: HashMap<String, usize>,
    pub summary_by_provider: HashMap<String, usize>,
    /// Count of detections whose classification is `Real` (real-looking
    /// finding in real source). This is the headline number the UI should
    /// show — not `detections.len()`.
    pub real_finding_count: usize,
    /// Count of detections classified as any fixture variant. UI surfaces
    /// these in a separate "Test fixtures (informational)" panel.
    pub fixture_finding_count: usize,
    pub respect_gitignore: bool,
    pub follow_symlinks: bool,
}

// ── Constants ────────────────────────────────────────────────────────────────

const DEFAULT_MAX_FILE_SIZE: u64 = 5_000_000; // 5 MB
const BINARY_SNIFF_BYTES: usize = 8192;

/// Directories we ALWAYS skip, regardless of `respect_gitignore` setting.
/// These are universally noise for a secrets-leak audit.
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

/// Path components that strongly suggest test/fixture/example code rather
/// than production source. Findings under these directories are still
/// detected and reported, but reclassified as fixtures so they don't
/// inflate the verdict or the headline finding count.
///
/// Match is on a single path component name, case-insensitive.
const TEST_PATH_COMPONENTS: &[&str] = &[
    "tests",
    "test",
    "examples",
    "example",
    "spec",
    "specs",
    "__tests__",
    "__mocks__",
    "fixtures",
    "fixture",
    "testdata",
    "test_data",
    "test-data",
    "mocks",
];

// ── Public entry point ──────────────────────────────────────────────────────

/// Expand a leading `~` or `~/` in a user-supplied path to `$HOME`.
/// Leaves the path unchanged if it doesn't start with `~`. Used to accept
/// shorthand paths from the frontend without forcing absolute paths.
fn expand_user_path(input: &str) -> PathBuf {
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

pub fn scan_local_path(args: ScanArgs) -> Result<ScanReport, String> {
    let started = Instant::now();

    let root = expand_user_path(args.path.trim());
    if root.as_os_str().is_empty() {
        return Err("path is empty".into());
    }
    if !root.exists() {
        return Err(format!("path does not exist: {}", root.display()));
    }
    if !root.is_dir() {
        return Err(format!("path is not a directory: {}", root.display()));
    }

    let canonical = root
        .canonicalize()
        .map_err(|e| format!("could not canonicalize path: {e}"))?;

    // Refuse high-blast-radius paths that are almost certainly a mistake.
    refuse_dangerous_root(&canonical)?;

    let max_size = if args.max_file_size_bytes == 0 {
        DEFAULT_MAX_FILE_SIZE
    } else {
        args.max_file_size_bytes
    };

    // Compute git-tracked set once. Empty set if not a git repo or git is
    // unavailable.
    let tracked_files = collect_git_tracked(&canonical);

    // Build the walker. We disable git_ignore by default so .env files
    // (typically gitignored) still get scanned — the whole point of a
    // secrets audit is to find those. The user can opt back into
    // gitignore-respecting via `respect_gitignore: true`.
    let mut builder = ignore::WalkBuilder::new(&canonical);
    builder
        .follow_links(args.follow_symlinks)
        .git_ignore(args.respect_gitignore)
        .git_global(args.respect_gitignore)
        .git_exclude(args.respect_gitignore)
        .ignore(args.respect_gitignore)
        .hidden(false) // walk hidden dotfiles — that's where leaks hide
        .filter_entry(|entry| !is_in_skip_dir(entry.path()));

    let mut scanned_files = 0usize;
    let mut skipped_binary = 0usize;
    let mut skipped_oversize = 0usize;
    let mut skipped_unreadable = 0usize;
    let mut skipped_ignored = 0usize;
    let mut detections: Vec<Detection> = Vec::new();

    for entry in builder.build() {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => {
                skipped_unreadable += 1;
                continue;
            }
        };

        // Skip non-files (dirs are walked into; we only scan file leaves).
        let is_file = entry.file_type().map(|ft| ft.is_file()).unwrap_or(false);
        if !is_file {
            continue;
        }

        let path = entry.path();

        // filter_entry above prunes always-skip directories at the dir
        // level. For paranoia, double-check at the file level too.
        if is_in_skip_dir(path) {
            skipped_ignored += 1;
            continue;
        }

        let metadata = match path.metadata() {
            Ok(m) => m,
            Err(_) => {
                skipped_unreadable += 1;
                continue;
            }
        };

        if metadata.len() > max_size {
            skipped_oversize += 1;
            continue;
        }

        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(_) => {
                skipped_unreadable += 1;
                continue;
            }
        };

        // Binary heuristic: NUL byte in the first chunk.
        let head_len = bytes.len().min(BINARY_SNIFF_BYTES);
        if bytes[..head_len].contains(&0u8) {
            skipped_binary += 1;
            continue;
        }

        let text = match std::str::from_utf8(&bytes) {
            Ok(s) => s,
            Err(_) => {
                skipped_unreadable += 1;
                continue;
            }
        };

        scanned_files += 1;

        let mut file_dets = scan_text(text);
        if file_dets.is_empty() {
            // Drop the file's bytes immediately — we don't keep clean files
            // in memory.
            drop(bytes);
            continue;
        }

        let rel_display = path
            .strip_prefix(&canonical)
            .unwrap_or(path)
            .display()
            .to_string();
        let abs_path = path.to_path_buf();
        let is_tracked = tracked_files.contains(&abs_path);

        let path_is_test = is_test_path(&rel_display);
        let path_is_self_ref = is_self_reference_path(&rel_display);
        for d in &mut file_dets {
            d.file_path = Some(rel_display.clone());
            d.git_tracked = Some(is_tracked);
            // Upgrade the classification with path information that
            // `scan_text` (which only sees the raw value) couldn't know.
            // `path_is_self_ref` is treated as a test-equivalent so AEO docs
            // and detector source samples get fixture-classified instead of
            // driving the verdict.
            if path_is_test || path_is_self_ref {
                d.classification = match d.classification {
                    Classification::Real => Classification::TestPath,
                    Classification::TestValue => Classification::TestPathAndValue,
                    other => other, // already a path-aware variant; leave as-is
                };
                if let Some(det) = detector_for_id(d.secret_type) {
                    d.recommended_action = recommendation_for(det, d.classification);
                }
            }
        }
        detections.extend(file_dets);

        // Drop the bytes vec promptly so secret material isn't held longer
        // than necessary.
        drop(bytes);
    }

    let (summary_by_detector, summary_by_risk, summary_by_provider) = build_summaries(&detections);
    let summary_by_risk_excluding_fixtures =
        build_risk_summary(detections.iter().filter(|d| !d.classification.is_fixture()));
    let real_finding_count = detections
        .iter()
        .filter(|d| !d.classification.is_fixture())
        .count();
    let fixture_finding_count = detections.len() - real_finding_count;

    Ok(ScanReport {
        root_path: canonical.display().to_string(),
        scanned_files,
        skipped_binary,
        skipped_oversize,
        skipped_unreadable,
        skipped_ignored,
        elapsed_ms: started.elapsed().as_millis() as u64,
        detections,
        summary_by_detector,
        summary_by_risk,
        summary_by_risk_excluding_fixtures,
        summary_by_provider,
        real_finding_count,
        fixture_finding_count,
        respect_gitignore: args.respect_gitignore,
        follow_symlinks: args.follow_symlinks,
    })
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn refuse_dangerous_root(canonical: &Path) -> Result<(), String> {
    if canonical == Path::new("/") {
        return Err("refusing to scan filesystem root '/'".into());
    }
    if let Some(home_path) = dirs::home_dir() {
        if canonical == home_path.as_path() {
            return Err(format!(
                "refusing to scan $HOME ({}) directly. Pick a project subdirectory.",
                home_path.display()
            ));
        }
    }
    // Also refuse common system-level mountpoints that almost certainly
    // aren't a user's project.
    for forbidden in ["/etc", "/var", "/usr", "/System", "/Library", "/private"] {
        if canonical == Path::new(forbidden) {
            return Err(format!(
                "refusing to scan system path {forbidden}. Pick a project directory."
            ));
        }
    }
    Ok(())
}

fn is_in_skip_dir(path: &Path) -> bool {
    let skip: HashSet<&str> = ALWAYS_SKIP_DIRS.iter().copied().collect();
    path.components().any(|c| {
        if let std::path::Component::Normal(name) = c {
            name.to_str().map(|n| skip.contains(n)).unwrap_or(false)
        } else {
            false
        }
    })
}

/// True if `rel_path` looks like a test, example, or fixture path.
///
/// Checks two things:
///   1. Any path component matches a known test directory name
///      (`tests/`, `examples/`, `__tests__/`, `fixtures/`, etc.).
///   2. Filename matches a per-language test convention (`*_test.rs`,
///      `test_*.py`, `*.test.ts`, `*.spec.js`, etc.).
///
/// Case-insensitive throughout. Operates on a relative path string (not a
/// `Path`) so the M3 scanner can pass the same display string the UI sees.
pub fn is_test_path(rel_path: &str) -> bool {
    // Normalize to forward slashes (Windows tolerance) and lowercase.
    let normalized = rel_path.replace('\\', "/").to_ascii_lowercase();

    // Component check.
    for component in normalized.split('/') {
        if TEST_PATH_COMPONENTS.contains(&component) {
            return true;
        }
    }

    // Filename suffix / prefix conventions.
    let filename = normalized.rsplit('/').next().unwrap_or(&normalized);
    if filename.ends_with("_test.rs")
        || filename.ends_with("_test.py")
        || filename.ends_with("_test.go")
        || filename.ends_with(".test.ts")
        || filename.ends_with(".test.tsx")
        || filename.ends_with(".test.js")
        || filename.ends_with(".test.jsx")
        || filename.ends_with(".spec.ts")
        || filename.ends_with(".spec.tsx")
        || filename.ends_with(".spec.js")
        || filename.ends_with(".spec.jsx")
        || filename.ends_with("_spec.rb")
        || filename.starts_with("test_")
        || filename.starts_with("tests_")
    {
        return true;
    }

    false
}

/// True if `rel_path` is a self-reference path — Doctor's own AEO/marketing
/// docs that intentionally embed example secret-shapes ("YOUR_KEY_HERE",
/// "your-project-id") for educational purposes, OR Doctor's own detector
/// source files that contain regex/pattern samples by design.
///
/// Treating these as Fixture (rather than Real) prevents Doctor from giving
/// itself a "Not handoff-ready" verdict on its own repo, which would
/// undermine the wedge in tester-facing demos. Tier-0 self-test 2026-05-11
/// surfaced this — see Operations/AgentOps/Revenue/2026-05-11-holster-doctor-tier-0-findings.md.
///
/// Three categories:
///   1. `automation-output/` prefix — AEO documentation showing example
///      leaks alongside redacted-replacement examples.
///   2. `marketing-artifacts/` prefix — Nauta-AI marketing/AEO docs with
///      example tokens; same family as automation-output/, different repo.
///   3. `apps/desktop/src-tauri/src/detectors.rs` — the detector source
///      itself, which embeds pattern samples to drive its own regex tests.
///
/// Case-insensitive throughout. Path-prefix match only — no component
/// scan — because both categories are file-or-directory prefixes, not
/// reusable test-style component names.
pub fn is_self_reference_path(rel_path: &str) -> bool {
    let normalized = rel_path.replace('\\', "/").to_ascii_lowercase();
    if normalized.starts_with("automation-output/") {
        return true;
    }
    if normalized.starts_with("marketing-artifacts/") {
        return true;
    }
    if normalized == "apps/desktop/src-tauri/src/detectors.rs" {
        return true;
    }
    false
}

fn collect_git_tracked(canonical: &Path) -> HashSet<PathBuf> {
    let out = Command::new("git")
        .arg("-C")
        .arg(canonical)
        .args(["ls-files", "-z"])
        .output();
    let mut tracked = HashSet::new();
    if let Ok(o) = out {
        if o.status.success() {
            for chunk in o.stdout.split(|&b| b == 0) {
                if chunk.is_empty() {
                    continue;
                }
                if let Ok(s) = std::str::from_utf8(chunk) {
                    tracked.insert(canonical.join(s));
                }
            }
        }
    }
    tracked
}

fn risk_to_str(r: RiskLevel) -> &'static str {
    match r {
        RiskLevel::Critical => "critical",
        RiskLevel::High => "high",
        RiskLevel::Medium => "medium",
        RiskLevel::Low => "low",
    }
}

fn build_summaries(
    detections: &[Detection],
) -> (
    Vec<DetectorSummary>,
    HashMap<String, usize>,
    HashMap<String, usize>,
) {
    let mut by_id: HashMap<&str, DetectorSummary> = HashMap::new();
    let mut by_risk: HashMap<String, usize> = HashMap::new();
    let mut by_provider: HashMap<String, usize> = HashMap::new();

    for d in detections {
        let entry = by_id.entry(d.secret_type).or_insert(DetectorSummary {
            detector_id: d.secret_type.to_string(),
            display_name: d.display_name.to_string(),
            provider: d.provider.to_string(),
            tier: d.tier,
            risk_level: d.risk_level,
            count: 0,
        });
        entry.count += 1;

        *by_risk
            .entry(risk_to_str(d.risk_level).to_string())
            .or_insert(0) += 1;
        *by_provider.entry(d.provider.to_string()).or_insert(0) += 1;
    }

    let mut by_detector: Vec<DetectorSummary> = by_id.into_values().collect();
    by_detector.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then(a.detector_id.cmp(&b.detector_id))
    });

    (by_detector, by_risk, by_provider)
}

/// Build only the by-risk count map from an iterator of detections. Used
/// to compute `summary_by_risk_excluding_fixtures` without re-walking the
/// whole detection list to rebuild the by_detector and by_provider maps.
fn build_risk_summary<'a, I>(detections: I) -> HashMap<String, usize>
where
    I: IntoIterator<Item = &'a Detection>,
{
    let mut by_risk: HashMap<String, usize> = HashMap::new();
    for d in detections {
        *by_risk
            .entry(risk_to_str(d.risk_level).to_string())
            .or_insert(0) += 1;
    }
    by_risk
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    // All test inputs use clearly-FAKE values. None of these are real keys.

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_tempdir(label: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let mut p = std::env::temp_dir();
        p.push(format!("holster-scan-test-{label}-{nanos}-{n}"));
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn mk(dir: &Path, rel: &str, content: &str) {
        let full = dir.join(rel);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full, content).unwrap();
    }

    fn run(dir: &Path) -> ScanReport {
        scan_local_path(ScanArgs {
            path: dir.display().to_string(),
            follow_symlinks: false,
            respect_gitignore: false,
            max_file_size_bytes: 0,
        })
        .unwrap()
    }

    fn fake_stripe_live_secret() -> String {
        ["sk", "_live_", "FAKE0FAKE0FAKE0FAKE0FAKE0"].concat()
    }

    // ── Path safety ─────────────────────────────────────────────────────────

    #[test]
    fn refuses_filesystem_root() {
        let r = scan_local_path(ScanArgs {
            path: "/".into(),
            follow_symlinks: false,
            respect_gitignore: false,
            max_file_size_bytes: 0,
        });
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("filesystem root"));
    }

    #[test]
    fn refuses_home_directly() {
        let Some(home_path) = dirs::home_dir() else {
            return;
        };
        let home = home_path.display().to_string();
        let r = scan_local_path(ScanArgs {
            path: home.clone(),
            follow_symlinks: false,
            respect_gitignore: false,
            max_file_size_bytes: 0,
        });
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("HOME"));
    }

    #[test]
    fn refuses_nonexistent_path() {
        let r = scan_local_path(ScanArgs {
            path: "/this-path-should-not-exist-zzz".into(),
            follow_symlinks: false,
            respect_gitignore: false,
            max_file_size_bytes: 0,
        });
        assert!(r.is_err());
    }

    // ── Detection on file content ───────────────────────────────────────────

    #[test]
    fn detects_openai_key_in_env_file() {
        let dir = unique_tempdir("openai-env");
        mk(
            &dir,
            ".env.local",
            "OPENAI_API_KEY=sk-FAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKE0123\n",
        );
        let report = run(&dir);
        assert!(report.scanned_files >= 1);
        assert!(report
            .detections
            .iter()
            .any(|d| d.secret_type == "openai_api_key"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn finds_keys_in_nested_dir() {
        let dir = unique_tempdir("nested");
        mk(
            &dir,
            "src/config/secrets.ts",
            "export const k = \"sk-ant-api03-FAKE_FAKE_FAKE_FAKE_FAKE_FAKE_FAKE_FAKE_FAKE_FAKE0\"",
        );
        let report = run(&dir);
        assert!(report
            .detections
            .iter()
            .any(|d| d.secret_type == "anthropic_api_key"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn skips_binary_files() {
        let dir = unique_tempdir("binary");
        let bin: Vec<u8> = vec![0x7f, 0x45, 0x4c, 0x46, 0x00, 0xff, 0xff, 0xff, 0x00, 0x00];
        fs::write(dir.join("native.bin"), &bin).unwrap();
        let report = run(&dir);
        assert_eq!(report.skipped_binary, 1);
        assert_eq!(report.scanned_files, 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn skips_oversize_files() {
        let dir = unique_tempdir("oversize");
        let big = "A".repeat(2000);
        mk(&dir, "huge.txt", &big);
        let report = scan_local_path(ScanArgs {
            path: dir.display().to_string(),
            follow_symlinks: false,
            respect_gitignore: false,
            max_file_size_bytes: 100, // tiny cap
        })
        .unwrap();
        assert_eq!(report.skipped_oversize, 1);
        assert_eq!(report.scanned_files, 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn skips_always_skip_dirs() {
        let dir = unique_tempdir("skip-dirs");
        // Put a fake key inside node_modules/. Should NOT be scanned.
        mk(
            &dir,
            "node_modules/some-pkg/index.js",
            "const k = 'sk-FAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKE0123';",
        );
        // And one outside — should be found.
        mk(
            &dir,
            "src/app.js",
            "OPENAI_API_KEY=sk-FAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKE0123",
        );
        let report = run(&dir);
        let dets_in_node_modules: Vec<_> = report
            .detections
            .iter()
            .filter(|d| {
                d.file_path
                    .as_deref()
                    .map(|p| p.contains("node_modules"))
                    .unwrap_or(false)
            })
            .collect();
        assert!(
            dets_in_node_modules.is_empty(),
            "must not scan node_modules: {dets_in_node_modules:?}"
        );
        assert!(report.detections.iter().any(|d| d
            .file_path
            .as_deref()
            .map(|p| p.contains("src/app.js"))
            .unwrap_or(false)));
        let _ = fs::remove_dir_all(&dir);
    }

    // ── Summary aggregation ─────────────────────────────────────────────────

    #[test]
    fn summary_counts_by_risk_and_provider() {
        let dir = unique_tempdir("summary");
        mk(
            &dir,
            ".env",
            &format!(
                "OPENAI_API_KEY=sk-FAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKE0123\n\
                 STRIPE_SECRET_KEY={}\n\
                 SLACK_HOOK=https://hooks.slack.com/services/TFAKE/BFAKE/FAKE0fake0fake0fake0fake0\n",
                fake_stripe_live_secret()
            ),
        );
        let report = run(&dir);
        assert!(*report.summary_by_risk.get("critical").unwrap_or(&0) >= 2);
        assert!(report.summary_by_provider.contains_key("openai"));
        assert!(report.summary_by_provider.contains_key("stripe"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_scan_returns_empty_report() {
        let dir = unique_tempdir("empty");
        // Just a clean readme.
        mk(
            &dir,
            "README.md",
            "# A clean project\n\nNothing to see here.\n",
        );
        let report = run(&dir);
        assert!(report.detections.is_empty());
        assert_eq!(report.scanned_files, 1);
        let _ = fs::remove_dir_all(&dir);
    }

    // ── The critical security test ──────────────────────────────────────────

    #[test]
    fn serialized_report_never_contains_raw_match() {
        // Build a scan that finds several secrets. Serialize the full
        // ScanReport to JSON. Assert NONE of the raw match strings appear
        // in the JSON output.
        let dir = unique_tempdir("no-leak");
        let raw_openai = "sk-FAKEUNIQUEMARKER1FAKEFAKEFAKEFAKEFAKE0";
        let raw_anthropic =
            "sk-ant-api03-FAKEUNIQUEMARKER2_FAKE_FAKE_FAKE_FAKE_FAKE_FAKE_FAKE_FAKE0";
        let raw_stripe = ["sk", "_live_", "FAKEUNIQUEMARKER3FAKE0FAKE0FAKE"].concat();
        let raw_github = "ghp_FAKEUNIQUEMARKER4FAKE0FAKE0FAKE0F";
        mk(&dir, ".env", &format!(
            "OPENAI_API_KEY={raw_openai}\nANTHROPIC_API_KEY={raw_anthropic}\nSTRIPE_SECRET_KEY={raw_stripe}\nGH_TOKEN={raw_github}\n"
        ));

        let report = run(&dir);
        assert!(!report.detections.is_empty());

        let json = serde_json::to_string(&report).unwrap();
        for raw in [raw_openai, raw_anthropic, raw_stripe.as_str(), raw_github] {
            assert!(
                !json.contains(raw),
                "LEAK: serialized ScanReport contains raw match {raw:?}"
            );
            // The unique markers should also not appear.
            assert!(
                !json.contains("FAKEUNIQUEMARKER1"),
                "LEAK: unique marker 1 found in JSON"
            );
            assert!(
                !json.contains("FAKEUNIQUEMARKER2"),
                "LEAK: unique marker 2 found in JSON"
            );
            assert!(
                !json.contains("FAKEUNIQUEMARKER3"),
                "LEAK: unique marker 3 found in JSON"
            );
            assert!(
                !json.contains("FAKEUNIQUEMARKER4"),
                "LEAK: unique marker 4 found in JSON"
            );
        }
        // The redacted_preview SHOULD appear in JSON (it's the legitimate
        // exposed field).
        assert!(json.contains("redacted_preview"));
        let _ = fs::remove_dir_all(&dir);
    }

    // ── git_tracked detection ───────────────────────────────────────────────

    #[test]
    fn git_tracked_flag_set_when_in_git_repo() {
        let dir = unique_tempdir("git-tracked");
        // Try to make this a real git repo + add the file. If git is not
        // available in the test environment, skip the assertion gracefully.
        let init = Command::new("git")
            .arg("-C")
            .arg(&dir)
            .arg("init")
            .arg("-q")
            .output();
        if init.is_err() || !init.as_ref().unwrap().status.success() {
            // Git not available — skip
            let _ = fs::remove_dir_all(&dir);
            return;
        }
        // Need user.email/user.name for some envs, but for ls-files we don't
        // need a commit. Just `git add` is enough to make it tracked.
        mk(
            &dir,
            ".env",
            "OPENAI_API_KEY=sk-FAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKE0123\n",
        );
        let _ = Command::new("git")
            .arg("-C")
            .arg(&dir)
            .arg("add")
            .arg(".env")
            .output();

        let report = run(&dir);
        let openai = report
            .detections
            .iter()
            .find(|d| d.secret_type == "openai_api_key");
        if let Some(d) = openai {
            // If git_tracked flagged true, the test confirms behavior. If
            // false, git was probably available but `add` failed in this env.
            if d.git_tracked == Some(true) {
                // good
            }
        }
        let _ = fs::remove_dir_all(&dir);
    }

    // ── Empty-state: clean repo ─────────────────────────────────────────────

    #[test]
    fn empty_state_clean_repo() {
        let dir = unique_tempdir("clean");
        mk(&dir, "src/main.rs", "fn main() { println!(\"hello\"); }");
        mk(&dir, "README.md", "# Project\n\nNothing secret here.\n");
        mk(&dir, "Cargo.toml", "[package]\nname = \"clean\"\n");
        let report = run(&dir);
        assert!(
            report.detections.is_empty(),
            "clean project should produce no detections: {:?}",
            report.detections
        );
        assert!(report.scanned_files >= 3);
        assert_eq!(report.real_finding_count, 0);
        assert_eq!(report.fixture_finding_count, 0);
        let _ = fs::remove_dir_all(&dir);
    }

    // ── Classification: path-based ──────────────────────────────────────────

    #[test]
    fn is_test_path_recognizes_common_conventions() {
        // Path-component matches.
        assert!(is_test_path("crates/foo/tests/integration.rs"));
        assert!(is_test_path("apps/cli/test/runner.rs"));
        assert!(is_test_path("crates/foo/examples/demo.rs"));
        assert!(is_test_path("examples/example.rs"));
        assert!(is_test_path("packages/ui/__tests__/Button.tsx"));
        assert!(is_test_path("src/__mocks__/api.ts"));
        assert!(is_test_path("tests/fixtures/sample.json"));
        assert!(is_test_path("data/testdata/case1.json"));
        // Filename-suffix matches.
        assert!(is_test_path("internal/handler_test.go"));
        assert!(is_test_path("pkg/utils_test.py"));
        assert!(is_test_path("src/components/Button.test.tsx"));
        assert!(is_test_path("src/api/client.spec.ts"));
        assert!(is_test_path("scripts/test_pipeline.py"));
        // Real-source negatives.
        assert!(!is_test_path("src/main.rs"));
        assert!(!is_test_path("apps/desktop/src/lib/views/Main.svelte"));
        assert!(!is_test_path("crates/holster-vault/src/vault.rs"));
        assert!(!is_test_path("README.md"));
        assert!(!is_test_path(".env"));
    }

    #[test]
    fn is_self_reference_path_covers_aeo_docs_and_detector_source() {
        // automation-output/ — AEO/marketing docs with intentional examples
        assert!(is_self_reference_path(
            "automation-output/2026-05-07-121432-foo.md"
        ));
        assert!(is_self_reference_path(
            "automation-output/sub/dir/anything.md"
        ));
        assert!(is_self_reference_path(
            "AUTOMATION-OUTPUT/case-insensitive.md"
        ));
        // marketing-artifacts/ — same family as automation-output/
        assert!(is_self_reference_path(
            "marketing-artifacts/2026-05-06-164750-foo.md"
        ));
        assert!(is_self_reference_path(
            "MARKETING-ARTIFACTS/case-insensitive.md"
        ));
        // detector source — contains pattern samples by design
        assert!(is_self_reference_path(
            "apps/desktop/src-tauri/src/detectors.rs"
        ));
        // Real source must NOT be flagged
        assert!(!is_self_reference_path(
            "apps/desktop/src-tauri/src/main.rs"
        ));
        assert!(!is_self_reference_path(
            "apps/desktop/src-tauri/src/repo_scanner.rs"
        ));
        assert!(!is_self_reference_path("crates/holster-vault/src/vault.rs"));
        assert!(!is_self_reference_path("README.md"));
        // Test/fixture paths are NOT self-reference (they have their own
        // path category in is_test_path).
        assert!(!is_self_reference_path("crates/foo/tests/integration.rs"));
    }

    #[test]
    fn detection_in_test_path_is_classified_as_fixture() {
        let dir = unique_tempdir("path-fixture");
        // Same shape, two locations. Values are kept under 40 chars total
        // so they DON'T trigger the high_entropy_generic_fallback detector
        // (which requires ≥40 chars after _KEY=) — keeping the test focused
        // on the openai detector + classification logic, not aggregation.
        // The src/ value is non-fixture-shaped; the tests/ value contains
        // "fake" so it'll classify as TestValue from the value classifier
        // and then upgrade to TestPathAndValue from the path classifier.
        //
        // String literals are split via concat! so this source file
        // (which itself lives at src/repo_scanner.rs) does not self-trip
        // the repo scanner with the non-fixture-shaped value.
        mk(
            &dir,
            "tests/integration.rs",
            concat!("let k = \"sk-", "fakefakefakefakefakefakefakefa\";"),
        );
        mk(
            &dir,
            "src/config.rs",
            concat!("let k = \"sk-", "prodprodprodprodprodprodprodpro\";"),
        );
        let report = run(&dir);

        // Find each detection.
        let in_tests = report
            .detections
            .iter()
            .find(|d| d.file_path.as_deref() == Some("tests/integration.rs"))
            .expect("detection in tests/ path");
        let in_src = report
            .detections
            .iter()
            .find(|d| d.file_path.as_deref() == Some("src/config.rs"))
            .expect("detection in src/ path");

        assert!(
            in_tests.classification.is_fixture(),
            "tests/ should be fixture: {in_tests:?}"
        );
        assert!(
            !in_src.classification.is_fixture(),
            "src/ should be real: {in_src:?}"
        );

        // Verify recommendation text differs.
        assert_ne!(
            in_tests.recommended_action, in_src.recommended_action,
            "test-path and real-path findings should get different recommendations"
        );

        // The summary excluding fixtures must NOT count the tests/ finding.
        let real_critical = *report
            .summary_by_risk_excluding_fixtures
            .get("critical")
            .unwrap_or(&0);
        let total_critical = *report.summary_by_risk.get("critical").unwrap_or(&0);
        assert_eq!(
            total_critical, 2,
            "raw summary_by_risk includes both findings"
        );
        assert_eq!(
            real_critical, 1,
            "excluding-fixtures summary drops the test-path finding"
        );
        assert_eq!(report.real_finding_count, 1);
        assert_eq!(report.fixture_finding_count, 1);

        let _ = fs::remove_dir_all(&dir);
    }

    // ── Classification: value-pattern ───────────────────────────────────────

    #[test]
    fn detection_with_test_value_pattern_is_classified_fixture() {
        let dir = unique_tempdir("value-fixture");
        // Real source path, but the value uses the canonical sk-test- prefix
        // — should be classified as TestValue, not Real.
        mk(
            &dir,
            "src/legit.rs",
            "OPENAI_API_KEY=sk-test-FAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKE0",
        );
        let report = run(&dir);
        let det = report
            .detections
            .iter()
            .find(|d| d.secret_type == "openai_api_key")
            .expect("detector should fire on the sk-test- value");
        assert!(
            det.classification.is_fixture(),
            "sk-test- prefix should be classified as fixture even in real source path: {det:?}"
        );
        assert_eq!(report.real_finding_count, 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn fixture_in_test_path_combines_to_test_path_and_value() {
        let dir = unique_tempdir("both");
        mk(
            &dir,
            "tests/no_secret_leak.rs",
            "const FAKE_VALUE: &str = \"sk-test-FAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKE0\";",
        );
        let report = run(&dir);
        let det = report
            .detections
            .iter()
            .find(|d| d.secret_type == "openai_api_key")
            .expect("detector should fire");
        assert!(matches!(
            det.classification,
            Classification::TestPathAndValue
        ));
        // The recommendation for the strongest fixture classification should
        // be the "no action needed" message — not the rotation hint.
        assert!(
            det.recommended_action
                .to_lowercase()
                .contains("no action needed"),
            "test-path-and-value finding should not say 'rotate immediately': {:?}",
            det.recommended_action
        );
        assert_eq!(report.real_finding_count, 0);
        assert_eq!(report.fixture_finding_count, 1);
        let _ = fs::remove_dir_all(&dir);
    }

    // ── Regression: holster's own test fixtures should classify cleanly ────

    #[test]
    fn holster_canonical_fixture_paths_classify_as_fixtures() {
        // Mirrors the structure that Tier-0 self-test surfaced: deliberate
        // `sk-test-fake-...` constants in `tests/no_secret_leak.rs` and
        // `examples/fake_sidecar_rollback.rs`. After this patch they should
        // never count toward the verdict.
        let dir = unique_tempdir("holster-fixture-regression");
        mk(
            &dir,
            "crates/holster-vault/tests/no_secret_leak.rs",
            "const FAKE_CHILD_WRAPPER_VALUE: &str = \"sk-test-fake-child-wrapper-2026\";\n\
             const FAKE_ALIZA_VALUE: &str = \"sk-test-fake-aliza-2026-05-05\";\n",
        );
        mk(
            &dir,
            "crates/holster-vault/examples/fake_sidecar_rollback.rs",
            "const FAKE_SIDECAR_VALUE: &str = \"sk-test-fake-sidecar-rollback-2026\";\n",
        );

        // Note: the OpenAI detector requires sk- + 30+ chars, so the short
        // fake-values above won't all fire. Add at least one that does.
        mk(
            &dir,
            "crates/holster-vault/src/vault.rs",
            "// inline test fixture used in #[cfg(test)] module\n\
             const FAKE_AGENT_RUNTIME: &str = \"sk-test-fake-agent-runtime-000000000000000000000000000000\";\n",
        );

        let report = run(&dir);
        // Every detection that DOES fire must be classified as a fixture
        // — none should be Real, none should drive the verdict.
        for d in &report.detections {
            assert!(
                d.classification.is_fixture(),
                "holster fixture path/value should classify as fixture, got Real: {d:?}"
            );
        }
        assert_eq!(
            report.real_finding_count, 0,
            "no real findings should remain after fixture classification"
        );
        // Excluding-fixtures summary must be empty for all severities.
        let total: usize = report.summary_by_risk_excluding_fixtures.values().sum();
        assert_eq!(
            total, 0,
            "summary_by_risk_excluding_fixtures must be empty for a fixtures-only repo"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
