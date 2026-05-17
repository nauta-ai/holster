#![cfg(not(target_os = "macos"))]

use std::fs;
use std::io::Write;
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

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

#[cfg(not(target_os = "macos"))]
#[test]
fn test_keychain_flag_rejected_on_linux() {
    let dir = TempDir::new().expect("tempdir");
    let vault_path = dir.path().join("vault.db");
    let audit_path = dir.path().join("exec-audit.jsonl");
    let manifest_path = dir.path().join("manifest.json");
    fs::write(
        &manifest_path,
        format!(
            r#"{{
              "agent_id": "codex-test",
              "audit_path": "{}",
              "env": [
                {{"name": "TEST_TOKEN", "provider": "github", "project": "acct", "label": "primary"}}
              ],
              "command": ["env"]
            }}"#,
            audit_path.display()
        ),
    )
    .expect("write manifest");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&manifest_path, fs::Permissions::from_mode(0o600))
            .expect("chmod manifest");
    }

    let out = run_cli(
        &[
            "exec-env",
            &vault_path.display().to_string(),
            "--manifest",
            &manifest_path.display().to_string(),
            "--password-keychain-service",
            "com.nautaai.holster.test",
        ],
        "",
    );

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--password-keychain-service is only supported on macOS"),
        "unexpected stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("use --password-env <ENV_NAME> or pipe the password via stdin"),
        "unexpected stderr:\n{stderr}"
    );
}
