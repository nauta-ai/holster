use holster_vault::{mirror_secret_to, MirrorSecretEntry, MirrorSecretInput, Provider, Vault};
use secrecy::ExposeSecret;
use tempfile::TempDir;
use uuid::Uuid;

const PWD: &str = "correct-horse-battery-staple";

fn mirror_input(id: Uuid, label: &str, value: &str) -> MirrorSecretInput {
    MirrorSecretInput {
        id,
        provider: Provider::Generic,
        label: label.to_string(),
        key_value: value.to_string(),
        project_tag: Some("rosie".to_string()),
        expires_at: None,
        notes: Some("mirror test".to_string()),
    }
}

#[test]
fn mirror_add_creates_destination_vault_and_preserves_primary_id() {
    let dir = TempDir::new().unwrap();
    let mirror_path = dir.path().join("personas.vault");
    let primary_id = Uuid::new_v4();

    let metadata = mirror_secret_to(
        &mirror_path,
        PWD,
        MirrorSecretEntry::Add(mirror_input(primary_id, "brave-api-key", "brave-test-key")),
    )
    .unwrap()
    .unwrap();

    assert_eq!(metadata.id, primary_id);
    assert!(mirror_path.exists());

    let vault = Vault::open(&mirror_path).unwrap();
    let token = vault.unlock(PWD).unwrap();
    let metas = vault.list_keys(token).unwrap();
    assert_eq!(metas.len(), 1);
    assert_eq!(metas[0].id, primary_id);
    assert_eq!(metas[0].project_tag.as_deref(), Some("rosie"));

    let secret = vault.get_key_value(token, primary_id).unwrap();
    assert_eq!(secret.expose_secret(), "brave-test-key");
}

#[test]
fn mirror_add_is_idempotent_for_existing_primary_id() {
    let dir = TempDir::new().unwrap();
    let mirror_path = dir.path().join("personas.vault");
    let primary_id = Uuid::new_v4();

    for _ in 0..2 {
        mirror_secret_to(
            &mirror_path,
            PWD,
            MirrorSecretEntry::Add(mirror_input(primary_id, "brave-api-key", "brave-test-key")),
        )
        .unwrap();
    }

    let vault = Vault::open(&mirror_path).unwrap();
    let token = vault.unlock(PWD).unwrap();
    let metas = vault.list_keys(token).unwrap();
    assert_eq!(metas.len(), 1);
    assert_eq!(metas[0].id, primary_id);
}

#[test]
fn mirror_delete_removes_existing_entry_and_treats_missing_as_ok() {
    let dir = TempDir::new().unwrap();
    let mirror_path = dir.path().join("personas.vault");
    let primary_id = Uuid::new_v4();

    mirror_secret_to(
        &mirror_path,
        PWD,
        MirrorSecretEntry::Add(mirror_input(primary_id, "brave-api-key", "brave-test-key")),
    )
    .unwrap();

    mirror_secret_to(
        &mirror_path,
        PWD,
        MirrorSecretEntry::Delete { id: primary_id },
    )
    .unwrap();
    mirror_secret_to(
        &mirror_path,
        PWD,
        MirrorSecretEntry::Delete { id: primary_id },
    )
    .unwrap();

    let vault = Vault::open(&mirror_path).unwrap();
    let token = vault.unlock(PWD).unwrap();
    assert!(vault.list_keys(token).unwrap().is_empty());
}

#[test]
fn mirror_supersede_marks_old_entry_when_both_entries_exist() {
    let dir = TempDir::new().unwrap();
    let mirror_path = dir.path().join("personas.vault");
    let old = Uuid::new_v4();
    let new = Uuid::new_v4();

    mirror_secret_to(
        &mirror_path,
        PWD,
        MirrorSecretEntry::Add(mirror_input(old, "old-brave", "old-value")),
    )
    .unwrap();
    mirror_secret_to(
        &mirror_path,
        PWD,
        MirrorSecretEntry::Add(mirror_input(new, "new-brave", "new-value")),
    )
    .unwrap();
    mirror_secret_to(
        &mirror_path,
        PWD,
        MirrorSecretEntry::Supersede {
            old_id: old,
            new_id: new,
        },
    )
    .unwrap();

    let vault = Vault::open(&mirror_path).unwrap();
    let token = vault.unlock(PWD).unwrap();
    let metas = vault.list_keys(token).unwrap();
    let old_meta = metas.iter().find(|meta| meta.id == old).unwrap();
    assert_eq!(old_meta.superseded_by, Some(new));
}

#[test]
fn mirror_delete_missing_destination_is_ok() {
    let dir = TempDir::new().unwrap();
    let mirror_path = dir.path().join("missing-personas.vault");

    mirror_secret_to(
        &mirror_path,
        PWD,
        MirrorSecretEntry::Delete { id: Uuid::new_v4() },
    )
    .unwrap();

    assert!(!mirror_path.exists());
}
