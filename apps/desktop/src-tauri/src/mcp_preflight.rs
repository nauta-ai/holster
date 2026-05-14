//! Holster MCP Preflight V0.
//!
//! Deterministic, local-first inspection for a single MCP server config entry.
//! This module does not execute the server, contact the network, or expose a
//! Tauri command; it only classifies config shape before a user trusts it.

use serde::{Deserialize, Serialize};
use serde_json::Value;

const KNOWN_WRAPPERS: &[&str] = &["npx", "uvx", "bunx", "pipx", "pnpm", "yarn"];

const SENSITIVE_ENV_VARS: &[&str] = &[
    "OPENAI_API_KEY",
    "ANTHROPIC_API_KEY",
    "GEMINI_API_KEY",
    "GOOGLE_API_KEY",
    "GITHUB_TOKEN",
    "GH_TOKEN",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_ACCESS_KEY_ID",
    "AWS_SESSION_TOKEN",
    "STRIPE_SECRET_KEY",
    "STRIPE_LIVE_SECRET_KEY",
    "OPENROUTER_API_KEY",
    "HUGGINGFACE_TOKEN",
    "HF_TOKEN",
    "CLOUDFLARE_API_TOKEN",
    "TELEGRAM_BOT_TOKEN",
    "REPLICATE_API_TOKEN",
    "ELEVENLABS_API_KEY",
    "PINECONE_API_KEY",
    "DATABASE_URL",
    "MONGODB_URI",
    "SUPABASE_SERVICE_KEY",
];

/// Run/share verdict for an MCP server config.
///
/// `Safe` means no caution or risk finding applies to that verdict category.
/// `Caution` and `Risky` roll up from the highest applicable finding severity.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Safe,
    Caution,
    Risky,
}

/// Severity for one preflight finding.
///
/// Informational findings document positive or neutral signals. Caution and
/// risk findings affect the run/share verdicts.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Caution,
    Risk,
}

/// Verdict category affected by a finding.
///
/// Run findings describe what happens on this machine when the MCP server is
/// launched. Share findings describe what happens if the config is shared.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Run,
    Share,
    Both,
}

/// One deterministic MCP preflight finding.
///
/// `check` is the stable check id. Caution and risk findings include a fix
/// hint; informational findings intentionally leave it empty.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Finding {
    pub check: String,
    pub severity: Severity,
    pub category: Category,
    pub message: String,
    pub fix_hint: Option<String>,
}

/// Full MCP preflight report for one server config entry.
///
/// The report separates the run verdict from the share verdict because a config
/// can be acceptable to run locally while still being unsafe to send to others.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct McpPreflightReport {
    pub server_name: Option<String>,
    pub run_verdict: Verdict,
    pub share_verdict: Verdict,
    pub findings: Vec<Finding>,
    pub raw_command_summary: String,
}

/// Errors returned when a config cannot be analyzed.
///
/// These are structural errors only. Risky but valid configurations are
/// reported as findings rather than returned as errors.
#[derive(thiserror::Error, Debug)]
pub enum McpPreflightError {
    #[error("invalid JSON: {0}")]
    InvalidJson(String),
    #[error("missing required field: {0}")]
    MissingField(&'static str),
    #[error("unsupported transport: {0}")]
    UnsupportedTransport(String),
}

#[derive(Debug)]
struct ParsedConfig {
    transport: Transport,
    command: Option<String>,
    args: Vec<String>,
    env_present: bool,
    env_is_null: bool,
    env_keys: Vec<String>,
    cwd_present: bool,
    cwd_is_null: bool,
    url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Transport {
    Stdio,
    Http,
}

/// Analyze an unnamed MCP server config entry.
///
/// This is the pure analyzer API for callers that do not have a server key
/// available. Use `analyze_mcp_config_named` when the config came from a named
/// Claude Desktop MCP server entry.
pub fn analyze_mcp_config(json: &str) -> Result<McpPreflightReport, McpPreflightError> {
    analyze_mcp_config_with_name(None, json)
}

/// Analyze a named MCP server config entry.
///
/// The returned report stores `name` in `server_name`, allowing a UI or audit
/// log to associate findings with the original config key.
pub fn analyze_mcp_config_named(
    name: &str,
    json: &str,
) -> Result<McpPreflightReport, McpPreflightError> {
    analyze_mcp_config_with_name(Some(name.to_string()), json)
}

fn analyze_mcp_config_with_name(
    server_name: Option<String>,
    json: &str,
) -> Result<McpPreflightReport, McpPreflightError> {
    let value: Value = serde_json::from_str(json)
        .map_err(|err| McpPreflightError::InvalidJson(err.to_string()))?;
    let config = parse_config(&value)?;
    let findings = analyze_findings(&config)?;
    let raw_command_summary = raw_command_summary(&config)?;
    let run_verdict = rollup_verdict(&findings, Category::Run);
    let share_verdict = rollup_verdict(&findings, Category::Share);

    Ok(McpPreflightReport {
        server_name,
        run_verdict,
        share_verdict,
        findings,
        raw_command_summary,
    })
}

fn parse_config(value: &Value) -> Result<ParsedConfig, McpPreflightError> {
    let transport = match value.get("transport").and_then(Value::as_str) {
        None | Some("stdio") => Transport::Stdio,
        Some("http") => Transport::Http,
        Some(other) => return Err(McpPreflightError::UnsupportedTransport(other.to_string())),
    };

    let args = value
        .get("args")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default();

    let env_value = value.get("env");
    let env_keys = env_value
        .and_then(Value::as_object)
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default();

    let config = ParsedConfig {
        transport,
        command: value
            .get("command")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        args,
        env_present: env_value.is_some(),
        env_is_null: env_value.is_some_and(Value::is_null),
        env_keys,
        cwd_present: value.get("cwd").is_some(),
        cwd_is_null: value.get("cwd").is_some_and(Value::is_null),
        url: value
            .get("url")
            .and_then(Value::as_str)
            .map(ToString::to_string),
    };

    match config.transport {
        Transport::Stdio if config.command.is_none() => {
            Err(McpPreflightError::MissingField("command"))
        }
        Transport::Http if config.url.is_none() => Err(McpPreflightError::MissingField("url")),
        _ => Ok(config),
    }
}

fn analyze_findings(config: &ParsedConfig) -> Result<Vec<Finding>, McpPreflightError> {
    match config.transport {
        Transport::Stdio => analyze_stdio_findings(config),
        Transport::Http => analyze_http_findings(config),
    }
}

fn analyze_stdio_findings(config: &ParsedConfig) -> Result<Vec<Finding>, McpPreflightError> {
    let command = config
        .command
        .as_deref()
        .ok_or(McpPreflightError::MissingField("command"))?;
    let mut findings = Vec::new();
    let is_wrapper = is_known_wrapper(command);

    if !command.contains('/') && !is_wrapper {
        findings.push(finding(
            "command_path_relative",
            Severity::Caution,
            Category::Run,
            format!("Command `{command}` is resolved through PATH rather than an absolute path."),
            Some(format!(
                "Resolve the command to an absolute path before trusting this config — `which {command}` reveals what PATH would pick up."
            )),
        ));
    }

    if is_wrapper {
        let first_arg = first_non_flag_arg(&config.args);
        let has_yes_flag = config.args.iter().any(|arg| arg == "-y");
        let is_pinned = first_arg.is_some_and(is_pinned_package_arg);

        if has_yes_flag || !is_pinned {
            findings.push(finding(
                "wrapper_unpinned",
                Severity::Risk,
                Category::Run,
                format!("Wrapper `{command}` can fetch or execute package code that is not pinned."),
                Some("Pin the package version (`@<version>` or `@sha256:...`) so a future package update can't change what gets executed.".into()),
            ));
        }

        if is_pinned {
            findings.push(finding(
                "wrapper_pinned",
                Severity::Info,
                Category::Run,
                format!("Wrapper `{command}` references a pinned package argument."),
                None,
            ));
        }
    }

    if !config.env_present || config.env_is_null {
        findings.push(finding(
            "env_implicit",
            Severity::Risk,
            Category::Share,
            "Config inherits the parent process environment.".into(),
            Some("Add an explicit `env` block so sharing this config doesn't leak your local environment.".into()),
        ));
    } else {
        let sensitive_keys = sensitive_env_keys(&config.env_keys);
        if sensitive_keys.is_empty() {
            findings.push(finding(
                "env_explicit_clean",
                Severity::Info,
                Category::Share,
                "Config has an explicit env block with no known sensitive variable names.".into(),
                None,
            ));
        } else {
            findings.push(finding(
                "env_sensitive_referenced",
                Severity::Risk,
                Category::Share,
                format!(
                    "Config references sensitive env var names: {}.",
                    sensitive_keys.join(", ")
                ),
                Some("This env block references a credential variable. Strip the env block or replace with placeholders before sharing.".into()),
            ));
        }
    }

    if !config.cwd_present || config.cwd_is_null {
        findings.push(finding(
            "cwd_inherited",
            Severity::Info,
            Category::Run,
            "Config inherits the current working directory.".into(),
            None,
        ));
    }

    if is_shell(command) && config.args.iter().any(|arg| arg == "-c") {
        findings.push(finding(
            "shell_exec",
            Severity::Risk,
            Category::Run,
            format!("Shell `{command}` executes an opaque command string via `-c`."),
            Some("A shell wrapper makes the actual command opaque to readers — invoke the binary directly with its args instead.".into()),
        ));
    }

    Ok(findings)
}

fn analyze_http_findings(config: &ParsedConfig) -> Result<Vec<Finding>, McpPreflightError> {
    let url = config
        .url
        .as_deref()
        .ok_or(McpPreflightError::MissingField("url"))?;
    let host = url_host(url);
    let mut findings = Vec::new();

    if host.as_deref().is_some_and(is_loopback_host) {
        findings.push(finding(
            "transport_http_local",
            Severity::Caution,
            Category::Both,
            "HTTP MCP server points at a loopback host.".into(),
            Some("Loopback HTTP is local, but still check which process owns the port before trusting it.".into()),
        ));
    } else {
        findings.push(finding(
            "transport_http_remote",
            Severity::Risk,
            Category::Run,
            "HTTP MCP server points outside loopback.".into(),
            Some("An HTTP MCP server outside loopback can be replaced by anyone controlling the URL — only use with TLS + auth you trust.".into()),
        ));
    }

    Ok(findings)
}

fn finding(
    check: &str,
    severity: Severity,
    category: Category,
    message: String,
    fix_hint: Option<String>,
) -> Finding {
    Finding {
        check: check.into(),
        severity,
        category,
        message,
        fix_hint,
    }
}

fn rollup_verdict(findings: &[Finding], category: Category) -> Verdict {
    let max_severity = findings
        .iter()
        .filter(|finding| finding.category == category || finding.category == Category::Both)
        .map(|finding| finding.severity)
        .max();

    match max_severity {
        Some(Severity::Risk) => Verdict::Risky,
        Some(Severity::Caution) => Verdict::Caution,
        Some(Severity::Info) | None => Verdict::Safe,
    }
}

fn raw_command_summary(config: &ParsedConfig) -> Result<String, McpPreflightError> {
    let raw = match config.transport {
        Transport::Stdio => {
            let command = config
                .command
                .as_deref()
                .ok_or(McpPreflightError::MissingField("command"))?;
            if config.args.is_empty() {
                command.to_string()
            } else {
                format!("{} {}", command, config.args.join(" "))
            }
        }
        Transport::Http => {
            let url = config
                .url
                .as_deref()
                .ok_or(McpPreflightError::MissingField("url"))?;
            format!("http {url}")
        }
    };

    Ok(truncate_summary(&raw))
}

fn truncate_summary(raw: &str) -> String {
    const MAX_LEN: usize = 200;
    if raw.chars().count() <= MAX_LEN {
        return raw.to_string();
    }

    raw.chars().take(MAX_LEN - 3).collect::<String>() + "..."
}

fn is_known_wrapper(command: &str) -> bool {
    KNOWN_WRAPPERS.contains(&command)
}

fn is_shell(command: &str) -> bool {
    matches!(command, "sh" | "bash" | "zsh" | "fish")
}

fn first_non_flag_arg(args: &[String]) -> Option<&str> {
    args.iter()
        .find(|arg| !arg.starts_with('-'))
        .map(String::as_str)
}

fn is_pinned_package_arg(arg: &str) -> bool {
    if arg.contains("@sha256:") {
        return true;
    }

    arg.rfind('@').is_some_and(|index| index > 0)
}

fn sensitive_env_keys(keys: &[String]) -> Vec<String> {
    keys.iter()
        .filter(|key| SENSITIVE_ENV_VARS.contains(&key.as_str()))
        .cloned()
        .collect()
}

fn url_host(url: &str) -> Option<String> {
    let after_scheme = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = after_scheme
        .split(['/', '?', '#'])
        .next()
        .filter(|part| !part.is_empty())?;
    let host_port = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);

    if let Some(rest) = host_port.strip_prefix('[') {
        return rest.split_once(']').map(|(host, _)| host.to_string());
    }

    Some(
        host_port
            .split_once(':')
            .map_or(host_port, |(host, _)| host)
            .to_string(),
    )
}

fn is_loopback_host(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checks(report: &McpPreflightReport) -> Vec<&str> {
        report
            .findings
            .iter()
            .map(|finding| finding.check.as_str())
            .collect()
    }

    #[test]
    fn safe_stdio_with_absolute_command_and_explicit_env() {
        let json = r#"{
            "command": "/usr/local/bin/example-mcp",
            "args": ["--profile", "FAKE"],
            "env": {"NODE_NO_WARNINGS": "1"},
            "cwd": "/Users/example"
        }"#;

        let report = analyze_mcp_config(json).expect("valid report");

        assert_eq!(report.run_verdict, Verdict::Safe);
        assert_eq!(report.share_verdict, Verdict::Safe);
        assert!(checks(&report).contains(&"env_explicit_clean"));
    }

    #[test]
    fn wrapper_npx_unpinned_flags_run_risk() {
        let json = r#"{
            "command": "npx",
            "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp/FAKE"],
            "env": {}
        }"#;

        let report = analyze_mcp_config(json).expect("valid report");

        assert_eq!(report.run_verdict, Verdict::Risky);
        assert!(checks(&report).contains(&"wrapper_unpinned"));
    }

    #[test]
    fn wrapper_npx_pinned_at_version_is_info_only() {
        let json = r#"{
            "command": "npx",
            "args": ["@modelcontextprotocol/server-filesystem@1.2.3", "/tmp/FAKE"],
            "env": {}
        }"#;

        let report = analyze_mcp_config(json).expect("valid report");

        assert_eq!(report.run_verdict, Verdict::Safe);
        assert!(checks(&report).contains(&"wrapper_pinned"));
        assert!(!checks(&report).contains(&"wrapper_unpinned"));
    }

    #[test]
    fn transport_http_localhost_is_caution_both() {
        let json = r#"{
            "transport": "http",
            "url": "http://localhost:3001/mcp"
        }"#;

        let report = analyze_mcp_config(json).expect("valid report");

        assert_eq!(report.run_verdict, Verdict::Caution);
        assert_eq!(report.share_verdict, Verdict::Caution);
        assert!(checks(&report).contains(&"transport_http_local"));
    }

    #[test]
    fn transport_http_remote_url_is_run_risk() {
        let json = r#"{
            "transport": "http",
            "url": "https://mcp.example.invalid/FAKE"
        }"#;

        let report = analyze_mcp_config(json).expect("valid report");

        assert_eq!(report.run_verdict, Verdict::Risky);
        assert_eq!(report.share_verdict, Verdict::Safe);
        assert!(checks(&report).contains(&"transport_http_remote"));
    }

    #[test]
    fn missing_env_block_is_share_risk() {
        let json = r#"{
            "command": "/usr/local/bin/example-mcp",
            "args": ["FAKE"]
        }"#;

        let report = analyze_mcp_config(json).expect("valid report");

        assert_eq!(report.share_verdict, Verdict::Risky);
        assert!(checks(&report).contains(&"env_implicit"));
    }

    #[test]
    fn env_with_sensitive_var_name_flags_share_risk() {
        let json = r#"{
            "command": "/usr/local/bin/example-mcp",
            "args": ["FAKE"],
            "env": {"OPENAI_API_KEY": "FAKE_OPENAI_API_KEY"}
        }"#;

        let report = analyze_mcp_config(json).expect("valid report");

        assert_eq!(report.share_verdict, Verdict::Risky);
        assert!(checks(&report).contains(&"env_sensitive_referenced"));
    }

    #[test]
    fn shell_exec_command_is_run_risk() {
        let json = r#"{
            "command": "bash",
            "args": ["-c", "echo FAKE"],
            "env": {}
        }"#;

        let report = analyze_mcp_config(json).expect("valid report");

        assert_eq!(report.run_verdict, Verdict::Risky);
        assert!(checks(&report).contains(&"shell_exec"));
    }

    #[test]
    fn relative_command_path_without_wrapper_is_run_caution() {
        let json = r#"{
            "command": "example-mcp",
            "args": ["FAKE"],
            "env": {}
        }"#;

        let report = analyze_mcp_config(json).expect("valid report");

        assert_eq!(report.run_verdict, Verdict::Caution);
        assert!(checks(&report).contains(&"command_path_relative"));
    }

    #[test]
    fn cwd_absent_is_info_only() {
        let json = r#"{
            "command": "/usr/local/bin/example-mcp",
            "args": ["FAKE"],
            "env": {}
        }"#;

        let report = analyze_mcp_config(json).expect("valid report");

        assert_eq!(report.run_verdict, Verdict::Safe);
        assert!(checks(&report).contains(&"cwd_inherited"));
    }

    #[test]
    fn invalid_json_returns_invalid_json_error() {
        let err = analyze_mcp_config("{FAKE").expect_err("invalid JSON should fail");

        assert!(matches!(err, McpPreflightError::InvalidJson(_)));
    }

    #[test]
    fn unsupported_transport_returns_error() {
        let json = r#"{
            "transport": "websocket",
            "url": "wss://example.invalid/FAKE"
        }"#;
        let err = analyze_mcp_config(json).expect_err("unsupported transport should fail");

        assert!(matches!(err, McpPreflightError::UnsupportedTransport(_)));
    }

    #[test]
    fn verdict_rolls_up_to_highest_severity_per_category() {
        let findings = vec![
            finding(
                "info",
                Severity::Info,
                Category::Run,
                "FAKE info".into(),
                None,
            ),
            finding(
                "caution",
                Severity::Caution,
                Category::Both,
                "FAKE caution".into(),
                Some("FAKE hint".into()),
            ),
            finding(
                "risk",
                Severity::Risk,
                Category::Share,
                "FAKE risk".into(),
                Some("FAKE hint".into()),
            ),
        ];

        assert_eq!(rollup_verdict(&findings, Category::Run), Verdict::Caution);
        assert_eq!(rollup_verdict(&findings, Category::Share), Verdict::Risky);
    }

    #[test]
    fn report_serializes_to_json_round_trip() {
        let json = r#"{
            "command": "/usr/local/bin/example-mcp",
            "args": ["FAKE"],
            "env": {}
        }"#;
        let report = analyze_mcp_config_named("fake-server", json).expect("valid report");
        let serialized = serde_json::to_string(&report).expect("serializes");
        let decoded: McpPreflightReport = serde_json::from_str(&serialized).expect("deserializes");

        assert_eq!(decoded.server_name.as_deref(), Some("fake-server"));
        assert_eq!(decoded.run_verdict, report.run_verdict);
        assert_eq!(decoded.share_verdict, report.share_verdict);
        assert_eq!(decoded.findings.len(), report.findings.len());
    }
}
