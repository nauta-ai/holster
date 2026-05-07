use std::path::{Path, PathBuf};

use holster_vault::{
    AddKeyInput, AgentProfile, AgentProfileStore, AllowedKeyPattern, AuditLogger, Provider, Vault,
    VaultError,
};
use secrecy::ExposeSecret;

const PASSWORD: &str = "fake-test-vault-password-2026";
const CODEX_FAKE_VALUE: &str = "sk-test-fake-openai-smoke-2026";
const ALIZA_FAKE_VALUE: &str = "sk-test-fake-etsy-smoke-2026";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = test_root()?;
    std::fs::create_dir_all(&root)?;
    set_dir_perms(&root);

    let vault_path = root.join("fake-agent-vault.db");
    let audit_path = root.join("fetch-events.jsonl");
    let profile_path = root.join("agents.json");

    remove_if_exists(&vault_path)?;
    remove_if_exists(&salt_path(&vault_path))?;
    remove_if_exists(&audit_path)?;

    write_profiles(&profile_path)?;

    let vault = Vault::create(&vault_path, PASSWORD)?;
    let token = vault.unlock(PASSWORD)?;

    let codex_key = vault
        .add_key(
            token,
            AddKeyInput {
                provider: Provider::Generic,
                label: "fake-openai-smoke".to_string(),
                key_value: CODEX_FAKE_VALUE.to_string(),
                project_tag: Some("fake-codex".to_string()),
                expires_at: None,
                notes: Some("fake smoke only".to_string()),
            },
        )?
        .id;

    let aliza_key = vault
        .add_key(
            token,
            AddKeyInput {
                provider: Provider::Generic,
                label: "fake-etsy-smoke".to_string(),
                key_value: ALIZA_FAKE_VALUE.to_string(),
                project_tag: Some("fake-aliza".to_string()),
                expires_at: None,
                notes: Some("fake smoke only".to_string()),
            },
        )?
        .id;

    let profiles = AgentProfileStore::from_json_file(&profile_path)?;
    let audit = AuditLogger::new(&audit_path);

    let secret = vault.fetch_key_for_agent(token, "codex", codex_key, &profiles, &audit)?;
    if secret.expose_secret() != CODEX_FAKE_VALUE {
        return Err("allowed fake fetch returned unexpected value".into());
    }
    drop(secret);

    let denied = vault.fetch_key_for_agent(token, "codex", aliza_key, &profiles, &audit);
    if !matches!(denied, Err(VaultError::AccessDenied)) {
        return Err("wrong-project fake fetch should be denied".into());
    }

    let unknown = vault.fetch_key_for_agent(token, "aliza", codex_key, &profiles, &audit);
    if !matches!(unknown, Err(VaultError::AccessDenied)) {
        return Err("unknown-agent fake fetch should be denied".into());
    }

    assert_audit_is_metadata_only(&audit_path)?;
    set_file_perms(&vault_path);
    set_file_perms(&salt_path(&vault_path));
    set_file_perms(&audit_path);
    set_file_perms(&profile_path);

    println!("fake_agent_runtime_smoke=ok");
    println!("vault={}", vault_path.display());
    println!("profiles={}", profile_path.display());
    println!("audit={}", audit_path.display());
    Ok(())
}

fn test_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home = std::env::var("HOME")?;
    Ok(Path::new(&home).join(".holster").join("test"))
}

fn write_profiles(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let profiles = vec![AgentProfile::new(
        "codex",
        vec![AllowedKeyPattern::new(
            Some(Provider::Generic),
            Some("fake-codex".to_string()),
            Some("fake-*".to_string()),
        )],
    )];
    let text = serde_json::to_string_pretty(&profiles)?;
    std::fs::write(path, text)?;
    set_file_perms(path);
    Ok(())
}

fn assert_audit_is_metadata_only(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(path)?;
    if !text.contains(r#""outcome":"allowed""#) {
        return Err("audit log missing allowed event".into());
    }
    if !text.contains(r#""outcome":"denied""#) {
        return Err("audit log missing denied event".into());
    }
    for forbidden in [CODEX_FAKE_VALUE, ALIZA_FAKE_VALUE] {
        if text.contains(forbidden) {
            return Err("audit log contains fake plaintext value".into());
        }
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
