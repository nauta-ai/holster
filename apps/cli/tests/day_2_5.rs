use std::io::Write;
use std::process::{Command, Output, Stdio};

use serde_json::Value;
use tempfile::TempDir;
use uuid::Uuid;

const PASSWORD: &str = "correct-horse-battery-staple";

fn holster_cli() -> &'static str {
    env!("CARGO_BIN_EXE_holster-cli")
}

fn run_cli(args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(holster_cli())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn holster-cli");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(stdin.as_bytes())
        .expect("write stdin");
    child.wait_with_output().expect("wait holster-cli")
}

fn create_vault() -> (TempDir, String) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("vault.db");
    let path_s = path.display().to_string();
    let out = run_cli(&["create", &path_s], &format!("{PASSWORD}\n{PASSWORD}\n"));
    assert_success(&out);
    (dir, path_s)
}

fn add_key(path: &str, provider: &str, label: &str, account: &str) -> Uuid {
    let out = run_cli(
        &[
            "add",
            path,
            "--provider",
            provider,
            "--label",
            label,
            "--project",
            account,
        ],
        &format!("{PASSWORD}\nsk-test-{provider}-{label}\n"),
    );
    assert_success(&out);
    first_uuid(&String::from_utf8_lossy(&out.stdout)).expect("add stdout uuid")
}

fn audit_log(path: &str, extra_args: &[&str]) -> Value {
    let mut args = vec!["audit-log", path, "--since-days", "7", "--json"];
    args.extend_from_slice(extra_args);
    let out = run_cli(&args, &format!("{PASSWORD}\n"));
    assert_success(&out);
    serde_json::from_slice(&out.stdout).expect("valid audit-log json")
}

fn assert_success(out: &Output) {
    assert!(
        out.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn first_uuid(text: &str) -> Option<Uuid> {
    text.split(|ch: char| ch.is_whitespace())
        .filter_map(|token| Uuid::parse_str(token).ok())
        .next()
}

#[test]
fn test_cli_audit_log_json_empty() {
    let (_dir, path) = create_vault();
    let json = audit_log(&path, &[]);
    assert_eq!(json["count"], 0);
    assert_eq!(json["events"].as_array().unwrap().len(), 0);
}

#[test]
fn test_cli_audit_log_json_after_add_delete() {
    let (_dir, path) = create_vault();
    let first = add_key(&path, "anthropic", "first", "acct-a");
    let _second = add_key(&path, "openai", "second", "acct-b");
    let delete = run_cli(
        &["delete", &path, &first.to_string()],
        &format!("{PASSWORD}\n"),
    );
    assert_success(&delete);

    let json = audit_log(&path, &[]);
    let events = json["events"].as_array().unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0]["kind"], "add");
    assert_eq!(events[1]["kind"], "add");
    assert_eq!(events[2]["kind"], "delete");
    assert_eq!(events[2]["entry_id"], first.to_string());
}

#[test]
fn test_cli_audit_log_provider_filter() {
    let (_dir, path) = create_vault();
    let anthropic = add_key(&path, "anthropic", "primary", "acct-a");
    let _openai = add_key(&path, "openai", "secondary", "acct-b");

    let json = audit_log(&path, &["--provider", "anthropic"]);
    let events = json["events"].as_array().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["provider"], "anthropic");
    assert_eq!(events[0]["entry_id"], anthropic.to_string());
}

#[test]
fn test_cli_supersede_happy_path() {
    let (_dir, path) = create_vault();
    let old = add_key(&path, "github", "old", "acct");
    let new = add_key(&path, "github", "new", "acct");

    let out = run_cli(
        &[
            "supersede",
            &path,
            &old.to_string(),
            "--replacement",
            &new.to_string(),
        ],
        &format!("{PASSWORD}\n"),
    );
    assert_success(&out);

    let json = audit_log(&path, &["--provider", "github"]);
    let events = json["events"].as_array().unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events[2]["kind"], "supersede");
    assert_eq!(events[2]["entry_id"], old.to_string());
    assert_eq!(events[2]["superseded_by"], new.to_string());
}

#[test]
fn test_cli_supersede_unknown_id_fails() {
    let (_dir, path) = create_vault();
    let existing = add_key(&path, "stripe", "existing", "acct");
    let missing = Uuid::new_v4();
    let out = run_cli(
        &[
            "supersede",
            &path,
            &missing.to_string(),
            "--replacement",
            &existing.to_string(),
        ],
        &format!("{PASSWORD}\n"),
    );
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("entry_not_found"),
        "stderr did not contain entry_not_found:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
