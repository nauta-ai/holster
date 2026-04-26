//! Holster desktop — Tauri 2 backend.
//!
//! M2: thin wrapper around `holster-vault` exposing the seven user-facing
//! features as Tauri commands. The frontend is treated as untrusted UI:
//! - The session token never crosses the IPC boundary. It lives in Rust state.
//! - Plaintext key material never crosses the IPC boundary either; the only
//!   way to extract a key is `copy_to_clipboard`, which writes it to the OS
//!   clipboard inside Rust and schedules an auto-clear.
//! - Master passwords are taken in by command argument (a transient `String`)
//!   and dropped at the end of the command. They are never persisted, never
//!   echoed back, never logged.
//!
//! State machine:
//!   no_vault          → first-run wizard calls `create_vault`
//!   locked            → user enters password, `unlock_vault` called
//!   unlocked          → list/add/get/delete; auto-locks on idle (15 min)
//!   wrong password    → returns `BadPassword` (clean error, no stack)
//!   session expired   → vault crate returns `SessionExpired`; UI re-prompts.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use chrono::{DateTime, Utc};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use uuid::Uuid;

use holster_vault::{
    AddKeyInput, KeyMetadata, KeyStatus, Provider, SessionToken, Vault, VaultError,
};

/// Auto-clear clipboard 30 seconds after a copy. Security feature.
const CLIPBOARD_AUTO_CLEAR_SECS: u64 = 30;

/// Default vault filename. Lives under the OS-standard app-data dir.
const DEFAULT_VAULT_FILENAME: &str = "vault.db";

// ── App state ────────────────────────────────────────────────────────────────

/// All shared mutable state. Each Mutex is held only briefly per command.
pub struct AppState {
    /// Path to the vault file. Set on `create_vault` or auto-resolved on
    /// startup based on the OS app-data dir. Public commands check this.
    vault_path: Mutex<Option<PathBuf>>,
    /// The Vault handle. `None` until first unlock or open.
    vault: Mutex<Option<Vault>>,
    /// Active session token. `None` while locked.
    session: Mutex<Option<SessionToken>>,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            vault_path: Mutex::new(None),
            vault: Mutex::new(None),
            session: Mutex::new(None),
        }
    }
}

// ── Frontend-visible types ───────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum VaultStatus {
    /// No vault file exists at the configured path. Show first-run wizard.
    NoVault,
    /// Vault exists on disk but no active session. Show unlock screen.
    Locked,
    /// Vault open with a valid session. Show key list.
    Unlocked,
}

#[derive(Serialize, Clone, Debug)]
pub struct VaultStatusReport {
    pub status: VaultStatus,
    pub path: Option<String>,
}

/// Plain serializable form of `KeyMetadata`. We re-export rather than relying
/// on the crate's serde impls so we control field naming and never accidentally
/// surface a future plaintext field.
#[derive(Serialize, Clone, Debug)]
pub struct KeyMetadataDto {
    pub id: String,
    pub provider: String,
    pub label: String,
    pub project_tag: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub status: String,
    pub notes: Option<String>,
}

impl From<KeyMetadata> for KeyMetadataDto {
    fn from(m: KeyMetadata) -> Self {
        let status = match m.status {
            KeyStatus::Active => "active",
            KeyStatus::ExpiringSoon => "expiring_soon",
            KeyStatus::Expired => "expired",
            KeyStatus::Stale => "stale",
            KeyStatus::Revoked => "revoked",
        }
        .to_string();
        KeyMetadataDto {
            id: m.id.to_string(),
            provider: m.provider.as_str().to_string(),
            label: m.label,
            project_tag: m.project_tag,
            created_at: m.created_at,
            expires_at: m.expires_at,
            last_used_at: m.last_used_at,
            status,
            notes: m.notes,
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct AddKeyArgs {
    pub provider: String,
    pub label: String,
    pub project_tag: Option<String>,
    pub notes: Option<String>,
    pub key_value: String, // plaintext, transient — never stored beyond this command
}

// ── Error mapping ────────────────────────────────────────────────────────────

/// Map a `VaultError` to a clean string for the UI. We deliberately omit any
/// internal context (paths, cause chains) that could leak structure to a
/// hostile observer. The error variants are already low-cardinality.
fn err_to_string(e: VaultError) -> String {
    match e {
        VaultError::Locked => "Vault is locked.".into(),
        VaultError::InvalidSession => "Session is invalid. Please unlock again.".into(),
        VaultError::SessionExpired => "Session expired due to inactivity.".into(),
        VaultError::BadPassword => "Incorrect password.".into(),
        VaultError::WeakPassword => "Password is too short (minimum 8 characters).".into(),
        VaultError::VaultNotFound => "No vault found at the configured path.".into(),
        VaultError::VaultAlreadyExists => "A vault already exists at that path.".into(),
        VaultError::KeyNotFound(_) => "Key not found.".into(),
        // The DB / Crypto / Migration variants can carry implementation strings.
        // For SQLCipher, any wrong-key open surfaces here as a Db error. Map it
        // to the same user-facing message as BadPassword to avoid signalling
        // crypto internals.
        VaultError::Db(_) => "Incorrect password or vault corrupted.".into(),
        VaultError::Crypto(_) => "Cryptographic error.".into(),
        VaultError::Migration(_) => "Vault file is corrupted or incompatible.".into(),
        VaultError::Io(_) => "Filesystem error.".into(),
    }
}

// ── Path helpers ─────────────────────────────────────────────────────────────

/// Resolve the default vault path under the OS app-data dir. On macOS that's
/// `~/Library/Application Support/com.nautaai.holster/vault.db`.
fn default_vault_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("could not resolve app data dir: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("could not create app data dir: {e}"))?;
    Ok(dir.join(DEFAULT_VAULT_FILENAME))
}

/// Look up the configured vault path, falling back to the default and
/// caching it on first use.
fn resolved_vault_path(app: &AppHandle, state: &AppState) -> Result<PathBuf, String> {
    {
        let g = state.vault_path.lock().map_err(|_| "state lock poisoned")?;
        if let Some(p) = g.as_ref() {
            return Ok(p.clone());
        }
    }
    let p = default_vault_path(app)?;
    let mut g = state.vault_path.lock().map_err(|_| "state lock poisoned")?;
    *g = Some(p.clone());
    Ok(p)
}

// ── Tauri commands ───────────────────────────────────────────────────────────

#[tauri::command]
fn vault_status(app: AppHandle, state: State<'_, AppState>) -> Result<VaultStatusReport, String> {
    let path = resolved_vault_path(&app, &state)?;
    let session = state.session.lock().map_err(|_| "state lock poisoned")?;
    let vault = state.vault.lock().map_err(|_| "state lock poisoned")?;

    let status = if !path.exists() {
        VaultStatus::NoVault
    } else if session.is_some() && vault.is_some() {
        // Opportunistic re-validation: if the session has expired in the crate,
        // surface as Locked rather than Unlocked.
        if let (Some(v), Some(t)) = (vault.as_ref(), session.as_ref()) {
            match v.list_keys(*t) {
                Ok(_) => VaultStatus::Unlocked,
                Err(_) => VaultStatus::Locked,
            }
        } else {
            VaultStatus::Locked
        }
    } else {
        VaultStatus::Locked
    };

    Ok(VaultStatusReport {
        status,
        path: Some(path.display().to_string()),
    })
}

#[tauri::command]
fn create_vault(
    password: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<VaultStatusReport, String> {
    let path = resolved_vault_path(&app, &state)?;
    if path.exists() {
        return Err(err_to_string(VaultError::VaultAlreadyExists));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create dir: {e}"))?;
    }
    Vault::create(&path, &password).map_err(err_to_string)?;
    // Drop the freshly-created (locked) Vault handle; the subsequent unlock
    // call will open a new one. Mirrors how the CLI flows.
    drop(password);

    Ok(VaultStatusReport {
        status: VaultStatus::Locked,
        path: Some(path.display().to_string()),
    })
}

#[tauri::command]
fn unlock_vault(
    password: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let path = resolved_vault_path(&app, &state)?;
    if !path.exists() {
        return Err(err_to_string(VaultError::VaultNotFound));
    }
    // Re-use existing Vault handle if present (so SQLCipher connection is reused),
    // otherwise open one.
    let mut vault_slot = state.vault.lock().map_err(|_| "state lock poisoned")?;
    if vault_slot.is_none() {
        let v = Vault::open(&path).map_err(err_to_string)?;
        *vault_slot = Some(v);
    }
    let vault = vault_slot
        .as_ref()
        .ok_or_else(|| "vault handle missing after open".to_string())?;

    let token = vault.unlock(&password).map_err(err_to_string)?;
    drop(password);

    let mut session_slot = state.session.lock().map_err(|_| "state lock poisoned")?;
    *session_slot = Some(token);
    Ok(())
}

#[tauri::command]
fn lock_vault(state: State<'_, AppState>) -> Result<(), String> {
    let mut session_slot = state.session.lock().map_err(|_| "state lock poisoned")?;
    let token = session_slot.take();
    drop(session_slot);
    if let Some(t) = token {
        let vault_slot = state.vault.lock().map_err(|_| "state lock poisoned")?;
        if let Some(v) = vault_slot.as_ref() {
            // Best-effort: revoke. Ignore errors (idempotent in the crate).
            let _ = v.lock(t);
        }
    }
    Ok(())
}

#[tauri::command]
fn list_keys(state: State<'_, AppState>) -> Result<Vec<KeyMetadataDto>, String> {
    let session = state.session.lock().map_err(|_| "state lock poisoned")?;
    let vault = state.vault.lock().map_err(|_| "state lock poisoned")?;
    let token = session
        .as_ref()
        .copied()
        .ok_or_else(|| err_to_string(VaultError::InvalidSession))?;
    let v = vault
        .as_ref()
        .ok_or_else(|| err_to_string(VaultError::Locked))?;
    let metas = v.list_keys(token).map_err(err_to_string)?;
    Ok(metas.into_iter().map(KeyMetadataDto::from).collect())
}

#[tauri::command]
fn add_key(args: AddKeyArgs, state: State<'_, AppState>) -> Result<KeyMetadataDto, String> {
    let provider = Provider::from_str(&args.provider)
        .ok_or_else(|| format!("unknown provider: {}", args.provider))?;
    if args.label.trim().is_empty() {
        return Err("label cannot be empty".into());
    }
    if args.key_value.is_empty() {
        return Err("key value cannot be empty".into());
    }
    let session = state.session.lock().map_err(|_| "state lock poisoned")?;
    let vault = state.vault.lock().map_err(|_| "state lock poisoned")?;
    let token = session
        .as_ref()
        .copied()
        .ok_or_else(|| err_to_string(VaultError::InvalidSession))?;
    let v = vault
        .as_ref()
        .ok_or_else(|| err_to_string(VaultError::Locked))?;
    let input = AddKeyInput {
        provider,
        label: args.label,
        key_value: args.key_value, // moved in; AddKeyInput drops it after encrypt
        project_tag: args.project_tag.filter(|s| !s.trim().is_empty()),
        expires_at: None,
        notes: args.notes.filter(|s| !s.trim().is_empty()),
    };
    let meta = v.add_key(token, input).map_err(err_to_string)?;
    Ok(meta.into())
}

#[tauri::command]
fn delete_key(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let uuid = Uuid::parse_str(&id).map_err(|_| "invalid id".to_string())?;
    let session = state.session.lock().map_err(|_| "state lock poisoned")?;
    let vault = state.vault.lock().map_err(|_| "state lock poisoned")?;
    let token = session
        .as_ref()
        .copied()
        .ok_or_else(|| err_to_string(VaultError::InvalidSession))?;
    let v = vault
        .as_ref()
        .ok_or_else(|| err_to_string(VaultError::Locked))?;
    v.delete_key(token, uuid).map_err(err_to_string)
}

/// Decrypt a key by id and write it to the OS clipboard. Schedules a clipboard
/// auto-clear after `CLIPBOARD_AUTO_CLEAR_SECS` seconds. Plaintext is dropped
/// from Rust as soon as the clipboard write returns.
#[tauri::command]
fn copy_to_clipboard(
    id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<u64, String> {
    let uuid = Uuid::parse_str(&id).map_err(|_| "invalid id".to_string())?;

    // Block to keep the locks held only as long as needed.
    let plaintext: String = {
        let session = state.session.lock().map_err(|_| "state lock poisoned")?;
        let vault = state.vault.lock().map_err(|_| "state lock poisoned")?;
        let token = session
            .as_ref()
            .copied()
            .ok_or_else(|| err_to_string(VaultError::InvalidSession))?;
        let v = vault
            .as_ref()
            .ok_or_else(|| err_to_string(VaultError::Locked))?;
        let secret = v.get_key_value(token, uuid).map_err(err_to_string)?;
        secret.expose_secret().clone()
    };

    app.clipboard()
        .write_text(plaintext.clone())
        .map_err(|e| format!("clipboard write failed: {e}"))?;
    // Best-effort zeroize of the local copy. clipboard owns its own copy now.
    drop(plaintext);

    // Schedule auto-clear. Use a detached thread (no tokio runtime needed for a
    // single sleep). The clear is unconditional — we don't try to detect whether
    // the user has copied something else in the meantime, which would require
    // reading the clipboard back (a privacy regression). Worst case: we clobber
    // the user's other copy 30s after they last copied a Holster key. Acceptable.
    let app_for_thread = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(CLIPBOARD_AUTO_CLEAR_SECS));
        let _ = app_for_thread.clipboard().clear();
        let _ = app_for_thread.emit("clipboard-cleared", ());
    });

    Ok(CLIPBOARD_AUTO_CLEAR_SECS)
}

// ── Application bootstrap ────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            vault_status,
            create_vault,
            unlock_vault,
            lock_vault,
            list_keys,
            add_key,
            delete_key,
            copy_to_clipboard,
        ])
        .setup(|app| {
            // Eagerly resolve the default vault path so subsequent commands
            // don't all race to create the same directory.
            let handle = app.handle().clone();
            let state: State<'_, AppState> = handle.state();
            if let Ok(p) = default_vault_path(&handle) {
                let mut g = state.vault_path.lock().expect("state lock poisoned");
                if g.is_none() {
                    *g = Some(p);
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
