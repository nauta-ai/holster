use std::path::PathBuf;
use std::time::Instant;

use clap::{Parser, Subcommand};
use holster_doctor::{agent_profiles, env_example, gitignore, preflight, scanner};
use serde::Serialize;

#[derive(Parser, Debug)]
#[command(name = "holster-doctor", version, about = "Holster Doctor scanner CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Scan a project tree for secret-shaped values.
    Scan {
        path: PathBuf,
        #[arg(long)]
        depth: Option<usize>,
        #[arg(long)]
        json: bool,
    },
    /// Audit .gitignore safe-default coverage.
    GitignoreAudit {
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Generate a committable .env.example from a profile or local .env file.
    EnvExample {
        path: PathBuf,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Analyze an MCP config JSON file.
    Preflight {
        path: PathBuf,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// List supported agent runtime profiles.
    ListProfiles {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Serialize)]
struct ErrorOut {
    ok: bool,
    error: String,
}

#[derive(Serialize)]
struct ScanFinding {
    file: Option<String>,
    line: usize,
    kind: String,
    severity: String,
    suggestion: String,
}

#[derive(Serialize)]
struct ScanOut {
    ok: bool,
    scanned_files: usize,
    findings: Vec<ScanFinding>,
    elapsed_ms: u64,
}

#[derive(Serialize)]
struct GitignoreOut {
    ok: bool,
    missing_patterns: Vec<String>,
    existing_safe: Vec<String>,
    existing_unsafe: Vec<String>,
    suggested_append: String,
    elapsed_ms: u64,
}

#[derive(Serialize)]
struct EnvExampleOut {
    ok: bool,
    generated_path: String,
    vars_extracted: Vec<String>,
    elapsed_ms: u64,
}

#[derive(Serialize)]
struct PreflightCheck {
    name: String,
    status: String,
    detail: String,
}

#[derive(Serialize)]
struct PreflightOut {
    ok: bool,
    checks: Vec<PreflightCheck>,
    summary: String,
    elapsed_ms: u64,
}

#[derive(Serialize)]
struct ListProfilesOut {
    ok: bool,
    profiles: Vec<String>,
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Scan { path, depth, json } => run_scan(path, depth, json),
        Command::GitignoreAudit { path, json } => run_gitignore(path, json),
        Command::EnvExample {
            path,
            profile,
            json,
        } => run_env_example(path, profile, json),
        Command::Preflight {
            path,
            profile,
            json,
        } => run_preflight(path, profile, json),
        Command::ListProfiles { json } => run_list_profiles(json),
    };

    if let Err(err) = result {
        print_json(&ErrorOut {
            ok: false,
            error: err,
        });
        std::process::exit(1);
    }
}

fn run_scan(path: PathBuf, _depth: Option<usize>, json: bool) -> Result<(), String> {
    let started = Instant::now();
    let report = scanner::scan_local_path(scanner::ScanArgs {
        path: path.display().to_string(),
        follow_symlinks: false,
        respect_gitignore: false,
        max_file_size_bytes: 0,
    })?;
    let findings = report
        .detections
        .into_iter()
        .map(|d| ScanFinding {
            file: d.file_path,
            line: d.line_number,
            kind: d.secret_type.to_string(),
            severity: format!("{:?}", d.risk_level).to_ascii_lowercase(),
            suggestion: d.recommended_action,
        })
        .collect();
    let out = ScanOut {
        ok: true,
        scanned_files: report.scanned_files,
        findings,
        elapsed_ms: started.elapsed().as_millis() as u64,
    };
    emit(json, &out, format!("scanned {} files", out.scanned_files));
    Ok(())
}

fn run_gitignore(path: PathBuf, json: bool) -> Result<(), String> {
    let started = Instant::now();
    let report = gitignore::audit(gitignore::GitignoreAuditArgs {
        path: path.display().to_string(),
    })?;
    let mut missing_patterns = Vec::new();
    let mut existing_safe = Vec::new();
    let existing_unsafe = Vec::new();
    let mut suggested_lines = Vec::new();
    for set in &report.rule_sets {
        for rule in &set.rules {
            if rule.already_present {
                existing_safe.push(rule.line.clone());
            } else {
                missing_patterns.push(rule.line.clone());
                if set.default_on {
                    suggested_lines.push(rule.line.clone());
                }
            }
        }
    }
    let out = GitignoreOut {
        ok: true,
        missing_patterns,
        existing_safe,
        existing_unsafe,
        suggested_append: suggested_lines.join("\n"),
        elapsed_ms: started.elapsed().as_millis() as u64,
    };
    emit(
        json,
        &out,
        format!("{} missing patterns", out.missing_patterns.len()),
    );
    Ok(())
}

fn run_env_example(path: PathBuf, profile: Option<String>, json: bool) -> Result<(), String> {
    let started = Instant::now();
    let root = canonical_project_dir(&path)?;
    let vars = env_vars_for_profile_or_env_file(&root, profile.as_deref())?;
    if vars.is_empty() {
        return Err("no env vars found for profile or local .env file".into());
    }
    let lines = vars
        .iter()
        .map(|name| env_example::EnvExampleLine {
            name: name.clone(),
            comment: profile.as_ref().map(|p| format!("profile: {p}")),
        })
        .collect::<Vec<_>>();
    let args = env_example::EnvExampleApplyArgs {
        target_dir: root.display().to_string(),
        filename: Some(".env.example".into()),
        lines,
        overwrite: true,
        include_header_comments: true,
    };
    let mut audit = |_payload: &serde_json::Value| -> Result<Option<String>, String> { Ok(None) };
    let report = env_example::apply_to_disk(&args, &mut audit)?;
    let out = EnvExampleOut {
        ok: true,
        generated_path: report.target_path,
        vars_extracted: vars,
        elapsed_ms: started.elapsed().as_millis() as u64,
    };
    emit(json, &out, format!("generated {}", out.generated_path));
    Ok(())
}

fn run_preflight(path: PathBuf, profile: Option<String>, json: bool) -> Result<(), String> {
    let started = Instant::now();
    let body = std::fs::read_to_string(&path)
        .map_err(|e| format!("could not read {}: {e}", path.display()))?;
    let report = match profile.as_deref() {
        Some(name) => preflight::analyze_mcp_config_named(name, &body),
        None => preflight::analyze_mcp_config(&body),
    }
    .map_err(|e| e.to_string())?;
    let mut checks = Vec::new();
    for finding in &report.findings {
        checks.push(PreflightCheck {
            name: finding.check.clone(),
            status: format!("{:?}", finding.severity).to_ascii_lowercase(),
            detail: finding.message.clone(),
        });
    }
    let summary = format!(
        "run={:?}; share={:?}; {} finding(s)",
        report.run_verdict,
        report.share_verdict,
        report.findings.len()
    )
    .to_ascii_lowercase();
    let out = PreflightOut {
        ok: true,
        checks,
        summary,
        elapsed_ms: started.elapsed().as_millis() as u64,
    };
    emit(json, &out, out.summary.clone());
    Ok(())
}

fn run_list_profiles(json: bool) -> Result<(), String> {
    let profiles = agent_profiles::agent_profile_catalog()
        .iter()
        .map(|p| match p.id {
            "generic" => "Generic".to_string(),
            "openclaw" => "OpenClaw".to_string(),
            "claude_code" => "ClaudeCode".to_string(),
            "codex" => "Codex".to_string(),
            other => {
                let mut chars = other.chars();
                match chars.next() {
                    Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>();
    let out = ListProfilesOut { ok: true, profiles };
    emit(json, &out, out.profiles.join(", "));
    Ok(())
}

fn env_vars_for_profile_or_env_file(
    root: &std::path::Path,
    profile: Option<&str>,
) -> Result<Vec<String>, String> {
    if let Some(profile) = profile {
        let wanted = normalize_profile_id(profile);
        let Some(found) = agent_profiles::agent_profile_catalog()
            .iter()
            .find(|p| p.id == wanted || p.name.eq_ignore_ascii_case(profile))
        else {
            return Err(format!("unknown profile: {profile}"));
        };
        return Ok(found
            .suggested_env_vars
            .iter()
            .map(|s| s.to_string())
            .collect());
    }

    for name in [".env", ".env.local", ".env.development"] {
        let candidate = root.join(name);
        if candidate.is_file() {
            let proposal =
                env_example::read_env_file_for_proposal(&env_example::EnvExampleFromFileArgs {
                    source_path: candidate.display().to_string(),
                })?;
            return Ok(proposal.lines.into_iter().map(|line| line.name).collect());
        }
    }
    Ok(Vec::new())
}

fn normalize_profile_id(raw: &str) -> String {
    raw.trim().to_ascii_lowercase().replace([' ', '-'], "_")
}

fn canonical_project_dir(path: &std::path::Path) -> Result<PathBuf, String> {
    let candidate = if path == std::path::Path::new("~") {
        dirs::home_dir().ok_or_else(|| "could not resolve home directory".to_string())?
    } else if let Ok(rest) = path.strip_prefix("~/") {
        dirs::home_dir()
            .ok_or_else(|| "could not resolve home directory".to_string())?
            .join(rest)
    } else {
        path.to_path_buf()
    };
    let canonical = candidate
        .canonicalize()
        .map_err(|e| format!("could not canonicalize {}: {e}", candidate.display()))?;
    if !canonical.is_dir() {
        return Err(format!("path is not a directory: {}", canonical.display()));
    }
    Ok(canonical)
}

fn emit<T: Serialize>(json: bool, value: &T, text: String) {
    if json {
        print_json(value);
    } else {
        println!("{text}");
    }
}

fn print_json<T: Serialize>(value: &T) {
    match serde_json::to_string_pretty(value) {
        Ok(s) => println!("{s}"),
        Err(e) => {
            println!(r#"{{"ok":false,"error":"json serialization failed: {e}"}}"#);
            std::process::exit(1);
        }
    }
}
