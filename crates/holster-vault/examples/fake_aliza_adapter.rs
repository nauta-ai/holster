use std::path::{Path, PathBuf};
use std::process::Command;

use holster_vault::{
    AddKeyInput, AgentProfile, AgentProfileStore, AllowedKeyPattern, AuditLogger, Provider, Vault,
};
use secrecy::ExposeSecret;

const PASSWORD: &str = "fake-aliza-password-2026";
const FAKE_ENV_NAME: &str = "HOLSTER_FAKE_ALIZA_KEY";
const FAKE_VALUE: &str = "sk-test-fake-aliza-2026-05-05";
const PROJECT_TAG: &str = "fake-aliza";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = test_root()?;
    std::fs::create_dir_all(&root)?;
    set_dir_perms(&root);

    let vault_path = root.join("fake-aliza-vault.db");
    let audit_path = root.join("fake-aliza-audit.jsonl");
    let profile_path = root.join("fake-aliza-agents.json");

    remove_if_exists(&vault_path)?;
    remove_if_exists(&salt_path(&vault_path))?;
    remove_if_exists(&audit_path)?;

    write_profiles(&profile_path)?;

    let vault = Vault::create(&vault_path, PASSWORD)?;
    let token = vault.unlock(PASSWORD)?;
    let key_id = vault
        .add_key(
            token,
            AddKeyInput {
                provider: Provider::Generic,
                label: "fake-etsy-smoke".to_string(),
                key_value: FAKE_VALUE.to_string(),
                project_tag: Some(PROJECT_TAG.to_string()),
                expires_at: None,
                notes: Some("fake Aliza smoke test only".to_string()),
            },
        )?
        .id;

    let profiles = AgentProfileStore::from_json_file(&profile_path)?;
    let audit = AuditLogger::new(&audit_path);
    let secret = vault.fetch_key_for_agent(token, "aliza", key_id, &profiles, &audit)?;

    let output = Command::new("/bin/sh")
        .arg("-c")
        .arg(format!(
            "test -n \"${FAKE_ENV_NAME}\" && printf 'child_env_present\\n'"
        ))
        .env(FAKE_ENV_NAME, secret.expose_secret())
        .output()?;
    drop(secret);

    if !output.status.success() {
        return Err("child process did not receive fake Aliza env var".into());
    }

    let stdout = String::from_utf8(output.stdout)?;
    let stderr = String::from_utf8(output.stderr)?;
    if stdout.trim() != "child_env_present" {
        return Err("child process printed unexpected output".into());
    }
    if stdout.contains(FAKE_VALUE) || stderr.contains(FAKE_VALUE) {
        return Err("child output leaked fake Aliza plaintext".into());
    }

    assert_audit_is_metadata_only(&audit_path)?;
    set_file_perms(&vault_path);
    set_file_perms(&salt_path(&vault_path));
    set_file_perms(&audit_path);
    set_file_perms(&profile_path);

    println!("fake_aliza_adapter=ok");
    println!("vault={}", vault_path.display());
    println!("profiles={}", profile_path.display());
    println!("audit={}", audit_path.display());
    println!("child_stdout={}", stdout.trim());
    Ok(())
}

fn test_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home = std::env::var("HOME")?;
    Ok(Path::new(&home).join(".holster").join("test"))
}

fn write_profiles(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let profiles = vec![AgentProfile::new(
        "aliza",
        vec![AllowedKeyPattern::new(
            Some(Provider::Generic),
            Some(PROJECT_TAG.to_string()),
            Some("fake-*".to_string()),
        )],
    )];
    std::fs::write(path, serde_json::to_string_pretty(&profiles)?)?;
    set_file_perms(path);
    Ok(())
}

fn assert_audit_is_metadata_only(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(path)?;
    if !text.contains(r#""outcome":"allowed""#) {
        return Err("audit log missing allowed event".into());
    }
    if text.contains(FAKE_VALUE) {
        return Err("audit log contains fake Aliza plaintext value".into());
    }
    Ok(())
}

fn remove_if_exists(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Box::new(e)),
    }
}

fn salt_path(vault: &Path) -> PathBuf {
    let mut p = vault.to_path_buf();
    let new_name = format!(
        "{}.salt",
        p.file_name().and_then(|s| s.to_str()).unwrap_or("vault")
    );
    p.set_file_name(new_name);
    p
}

fn set_dir_perms(_path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(_path, std::fs::Permissions::from_mode(0o700));
    }
}

fn set_file_perms(_path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(_path, std::fs::Permissions::from_mode(0o600));
    }
}
