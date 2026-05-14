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

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

// Holster Native Detector Pack — see detectors.rs.
// Pure registry + scanner; no Tauri commands wired in this V0 plan pass.
pub mod detectors;
// M3: directory walk + per-file scan, wraps detectors::scan_text.
pub mod repo_scanner;
// M3.1 T3.1.2: safe .gitignore audit + atomic append-only apply.
pub mod gitignore_helper;
// M3.1 T3.1.3: agent runtime profile catalogue (UX presets only).
pub mod agent_profiles;
// M3.1 T3.1.1: .env.example generator (vault and from-file modes).
pub mod env_example;
// M4: local-first TOTP authenticator entries, stored in the encrypted vault.
pub mod auth;
pub mod mcp_preflight;

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
const DEFAULT_EXPORT_FILENAME: &str = ".env.local";

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

#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeExportTarget {
    EnvFile,
}

#[derive(Deserialize, Debug)]
pub struct RuntimeExportArgs {
    pub key_ids: Vec<String>,
    pub target_dir: String,
    pub filename: Option<String>,
    pub profile_name: Option<String>,
    pub target: RuntimeExportTarget,
    pub dry_run: bool,
    pub backup_existing: bool,
    pub update_gitignore: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct RuntimeExportReport {
    pub dry_run: bool,
    pub target_path: String,
    pub profile_name: String,
    pub key_count: usize,
    pub exported_key_names: Vec<String>,
    pub preview_lines: Vec<String>,
    pub file_exists: bool,
    pub backup_path: Option<String>,
    pub git_tracked: bool,
    pub gitignore_updated: bool,
    pub audit_log_path: Option<String>,
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
        VaultError::AccessDenied => "Access denied by agent profile.".into(),
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

fn export_audit_log_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("could not resolve app data dir: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("could not create app data dir: {e}"))?;
    Ok(dir.join("runtime-export-audit.jsonl"))
}

pub(crate) fn sanitize_env_name(raw: &str) -> String {
    let mut out = String::new();
    let mut last_was_underscore = false;
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_uppercase());
            last_was_underscore = false;
        } else if !last_was_underscore {
            out.push('_');
            last_was_underscore = true;
        }
    }
    let trimmed = out.trim_matches('_').to_string();
    if trimmed.is_empty() {
        "KEY".to_string()
    } else {
        trimmed
    }
}

pub(crate) fn default_env_name(provider: Provider, label: &str, used: &[String]) -> String {
    let base = match provider {
        Provider::Anthropic => "ANTHROPIC_API_KEY".to_string(),
        Provider::OpenAI => "OPENAI_API_KEY".to_string(),
        Provider::Google => "GOOGLE_API_KEY".to_string(),
        Provider::Replicate => "REPLICATE_API_TOKEN".to_string(),
        Provider::ElevenLabs => "ELEVENLABS_API_KEY".to_string(),
        Provider::Pinecone => "PINECONE_API_KEY".to_string(),
        Provider::Stripe => "STRIPE_API_KEY".to_string(),
        Provider::Cloudflare => "CLOUDFLARE_API_TOKEN".to_string(),
        Provider::Generic => format!("{}_API_KEY", sanitize_env_name(label)),
    };
    if !used.contains(&base) {
        return base;
    }
    let labeled = format!("{}_{}", base, sanitize_env_name(label));
    if !used.contains(&labeled) {
        return labeled;
    }
    let mut n = 2;
    loop {
        let candidate = format!("{labeled}_{n}");
        if !used.contains(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

fn is_safe_env_filename(filename: &str) -> bool {
    if filename.trim().is_empty() {
        return false;
    }
    let path = Path::new(filename);
    if path.components().count() != 1 {
        return false;
    }
    filename == ".env" || filename == ".env.local" || filename.ends_with(".env")
}

/// Refuse a key value that begins or ends with whitespace.
///
/// V0 hardening (2026-04-30 evening): a real test export surfaced a fake
/// OpenAI key with a leading space (`OPENAI_API_KEY=' sk-FAKE-...'`) — almost
/// always a paste error, but virtually never a legitimate provider value.
/// Major providers (Stripe `sk_`, OpenAI `sk-`, Anthropic `ak-`, Google
/// `AIza`, etc.) do not issue whitespace-bound keys.
///
/// We DO NOT silently trim — a silent trim would (a) mask the user's paste
/// error, (b) potentially mangle a legitimately-whitespace-bounded value
/// from some niche source. Instead, we refuse with a clean UI-safe error
/// telling the user to fix the value at its source.
///
/// Called at TWO points:
///   - `add_key`: blocks future bad entries from entering the vault.
///   - `export_runtime_profile` Phase 2: blocks legacy bad entries from
///     reaching the env file. The user must delete + re-add to fix.
fn check_no_whitespace_bounds(value: &str) -> Result<(), String> {
    if let Some(first) = value.chars().next() {
        if first.is_whitespace() {
            return Err("key value starts with whitespace. This is almost always a \
                 paste error. Trim the value at its source and re-add it \
                 (Holster never silently trims secrets — a real provider \
                 value with intentional whitespace is vanishingly rare)."
                .into());
        }
    }
    if let Some(last) = value.chars().last() {
        if last.is_whitespace() {
            return Err("key value ends with whitespace. This is almost always a \
                 paste error. Trim the value at its source and re-add it \
                 (Holster never silently trims secrets — a real provider \
                 value with intentional whitespace is vanishingly rare)."
                .into());
        }
    }
    Ok(())
}

/// Quote a secret value for safe inclusion in a `.env` / `.env.local` file.
///
/// V0 hardening (2026-04-30):
///   1. REJECTS values containing `\n`, `\r`, or NUL — these would either
///      break the line format or split the secret across lines.
///   2. Uses single-quoted form (`KEY='value'`) so no env-file reader
///      performs `$VAR` expansion or backtick command-substitution on
///      the secret. Embedded `'` is escaped as `'\''` (POSIX-shell
///      standard close-escape-reopen).
///
/// Returns `Err` with a UI-safe message if the value is unsafe to encode.
/// Callers MUST handle the Err — never write a half-encoded secret.
fn shell_quote_env_value(value: &str) -> Result<String, String> {
    if value.bytes().any(|b| b == b'\n' || b == b'\r' || b == 0) {
        return Err("secret value contains newline, carriage return, or NUL — \
             cannot be safely encoded in a .env file. Refusing to export."
            .into());
    }
    let escaped = value.replace('\'', r"'\''");
    Ok(format!("'{escaped}'"))
}

fn relative_to_dir(path: &Path, dir: &Path) -> String {
    path.strip_prefix(dir).unwrap_or(path).display().to_string()
}

fn git_tracked(target_dir: &Path, filename: &str) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(target_dir)
        .arg("ls-files")
        .arg("--error-unmatch")
        .arg(filename)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn ensure_gitignore(target_dir: &Path) -> Result<bool, String> {
    let gitignore = target_dir.join(".gitignore");
    // `*.holster-tmp` covers atomic-write temp files in case a crash leaves
    // one behind — they should never get committed.
    let block = [
        "# Holster runtime secrets",
        ".env",
        ".env.local",
        "*.env",
        "*.holster-tmp",
    ];
    let existing = std::fs::read_to_string(&gitignore).unwrap_or_default();
    let mut changed = false;
    let mut next = existing.clone();
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    for line in block {
        if !existing
            .lines()
            .any(|existing_line| existing_line.trim() == line)
        {
            next.push_str(line);
            next.push('\n');
            changed = true;
        }
    }
    if changed {
        std::fs::write(&gitignore, next)
            .map_err(|e| format!("could not update .gitignore: {e}"))?;
    }
    Ok(changed)
}

/// Set 0600 permissions on a file containing secrets.
///
/// V0 hardening (2026-04-30): chmod failure is now a hard error. Previously
/// we discarded the error with `let _ = ...`, which could silently leave a
/// file with default umask permissions (typically 0644 — world-readable).
/// Returns Err so the caller can refuse to leave the file in place.
///
/// On non-unix targets this is a no-op (Windows has its own ACL story; out
/// of scope for V0).
fn set_secret_file_perms(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("could not chmod 0600 on {}: {e}", path.display()))?;
    }
    #[cfg(not(unix))]
    {
        let _ = path; // not fatal on non-unix
    }
    Ok(())
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
    // Hard-refuse whitespace-bounded values — almost always a paste error.
    // We never silently trim. See `check_no_whitespace_bounds` doc.
    check_no_whitespace_bounds(&args.key_value)?;
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

#[tauri::command]
fn export_runtime_profile(
    args: RuntimeExportArgs,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<RuntimeExportReport, String> {
    match args.target {
        RuntimeExportTarget::EnvFile => {}
    }
    if args.key_ids.is_empty() {
        return Err("select at least one key to export".into());
    }
    let target_dir = expand_home_path(args.target_dir.trim());
    if !target_dir.exists() || !target_dir.is_dir() {
        return Err("target folder does not exist".into());
    }
    let filename = args
        .filename
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_EXPORT_FILENAME)
        .to_string();
    if !is_safe_env_filename(&filename) {
        return Err("filename must be .env, .env.local, or end with .env".into());
    }
    let target_path = target_dir.join(&filename);
    let file_exists = target_path.exists();
    let git_tracked = git_tracked(&target_dir, &filename);
    if git_tracked && !args.dry_run {
        return Err("target env file is tracked by git; refusing to write secrets there".into());
    }

    let profile_name = args
        .profile_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("generic")
        .to_string();
    let ids: Vec<Uuid> = args
        .key_ids
        .iter()
        .map(|id| Uuid::parse_str(id).map_err(|_| "invalid key id".to_string()))
        .collect::<Result<_, _>>()?;

    // Phase 1: build per-key metadata (env name + display name) WITHOUT
    // touching secret values. Both dry-run and real-mode walk this phase.
    struct PreparedKey {
        id: Uuid,
        env_name: String,
        display_name: String,
    }

    let prepared: Vec<PreparedKey> = {
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

        let mut prepared = Vec::with_capacity(ids.len());
        let mut used_env_names: Vec<String> = Vec::with_capacity(ids.len());
        for id in &ids {
            let meta = metas
                .iter()
                .find(|m| m.id == *id)
                .ok_or_else(|| err_to_string(VaultError::KeyNotFound(*id)))?;
            let env_name = default_env_name(meta.provider, &meta.label, &used_env_names);
            used_env_names.push(env_name.clone());
            prepared.push(PreparedKey {
                id: *id,
                env_name,
                display_name: format!("{} / {}", meta.provider.as_str(), meta.label),
            });
        }
        prepared
    };

    let exported_key_names: Vec<String> = prepared.iter().map(|p| p.display_name.clone()).collect();
    let preview_lines: Vec<String> = prepared
        .iter()
        .map(|p| format!("{}=<redacted>", p.env_name))
        .collect();

    let mut backup_path = None;
    let mut gitignore_updated = false;
    let mut audit_log_path = None;

    if args.dry_run {
        // Dry-run: report from metadata only. NEVER calls get_key_value(),
        // NEVER builds secret_lines, NEVER writes anything to disk.
        return Ok(RuntimeExportReport {
            dry_run: true,
            target_path: target_path.display().to_string(),
            profile_name,
            key_count: preview_lines.len(),
            exported_key_names,
            preview_lines,
            file_exists,
            backup_path,
            git_tracked,
            gitignore_updated,
            audit_log_path,
        });
    }

    // Phase 2 (real-mode only): read secrets, encode them, write atomically.

    // Re-acquire vault lock briefly to read each secret. Quote each value
    // with shell_quote_env_value, which REJECTS values containing newline,
    // carriage return, or NUL — those would split the secret across lines
    // or otherwise corrupt the env file.
    let secret_lines: Vec<String> = {
        let session = state.session.lock().map_err(|_| "state lock poisoned")?;
        let vault = state.vault.lock().map_err(|_| "state lock poisoned")?;
        let token = session
            .as_ref()
            .copied()
            .ok_or_else(|| err_to_string(VaultError::InvalidSession))?;
        let v = vault
            .as_ref()
            .ok_or_else(|| err_to_string(VaultError::Locked))?;
        let mut lines = Vec::with_capacity(prepared.len());
        for p in &prepared {
            let secret = v.get_key_value(token, p.id).map_err(err_to_string)?;
            let secret_str = secret.expose_secret();
            // Hard-refuse legacy whitespace-bounded values. Don't silently
            // trim. The error names the offending key (provider/label) but
            // never echoes the value.
            if let Err(e) = check_no_whitespace_bounds(secret_str) {
                return Err(format!(
                    "key {:?} has a value with leading/trailing whitespace and \
                     cannot be exported. Delete the key and re-add it with the \
                     value trimmed. (Detail: {})",
                    p.display_name, e
                ));
            }
            let quoted = shell_quote_env_value(secret_str)?;
            lines.push(format!("{}={}", p.env_name, quoted));
        }
        lines
    };

    if file_exists && args.backup_existing {
        let stamp = Utc::now().format("%Y%m%d%H%M%S");
        let backup = target_dir.join(format!("{filename}.holster-backup-{stamp}"));
        std::fs::copy(&target_path, &backup)
            .map_err(|e| format!("could not create env backup: {e}"))?;
        set_secret_file_perms(&backup)?;
        backup_path = Some(backup.display().to_string());
    }

    // Atomic write: write to a temp file in the same directory (so rename
    // is atomic on POSIX), chmod 0600, then rename into place. On any
    // failure, attempt to remove the temp so we never leave half-encoded
    // secrets at the target path. The `*.holster-tmp` pattern is also in
    // the gitignore block so a crash-leftover temp can't be committed.
    let body = format!(
        "# Generated by Holster for profile: {profile_name}\n\
         # Values are local secrets. Do not commit this file.\n\
         {}\n",
        secret_lines.join("\n")
    );
    let temp_path = PathBuf::from(format!("{}.holster-tmp", target_path.display()));
    if let Err(e) = std::fs::write(&temp_path, &body) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!("could not write env temp file: {e}"));
    }
    if let Err(e) = set_secret_file_perms(&temp_path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(e);
    }
    if let Err(e) = std::fs::rename(&temp_path, &target_path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!("could not rename env temp into place: {e}"));
    }
    // Belt-and-suspenders: re-assert 0600 on the renamed-into-place file.
    // POSIX rename preserves perms, but if rename crossed filesystems
    // (rare; same-dir target avoids that), the perms could differ.
    set_secret_file_perms(&target_path)?;
    // Best-effort scrub of the in-memory secret bytes. The `String` heap
    // backing isn't truly zeroizable from safe Rust without an unsafe
    // overwrite, but dropping here releases the buffer back to the
    // allocator promptly.
    drop(body);
    drop(secret_lines);

    if args.update_gitignore {
        gitignore_updated = ensure_gitignore(&target_dir)?;
    }

    let audit_path = export_audit_log_path(&app)?;
    let audit = serde_json::json!({
        "ts": Utc::now(),
        "profile_name": profile_name,
        "target": "env_file",
        "target_path": target_path.display().to_string(),
        "target_file": relative_to_dir(&target_path, &target_dir),
        "key_count": exported_key_names.len(),
        "key_names": exported_key_names,
    });
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&audit_path)
        .map_err(|e| format!("could not open audit log: {e}"))?;
    writeln!(file, "{audit}").map_err(|e| format!("could not write audit log: {e}"))?;
    set_secret_file_perms(&audit_path)?;
    audit_log_path = Some(audit_path.display().to_string());

    Ok(RuntimeExportReport {
        dry_run: args.dry_run,
        target_path: target_path.display().to_string(),
        profile_name,
        key_count: preview_lines.len(),
        exported_key_names,
        preview_lines,
        file_exists,
        backup_path,
        git_tracked,
        gitignore_updated,
        audit_log_path,
    })
}

// ── M3.1 T3.1.1: .env.example generator commands ────────────────────────────

#[derive(serde::Deserialize, Debug)]
pub struct EnvExampleFromVaultArgs {
    pub key_ids: Vec<String>,
    pub include_holster_comments: bool,
}

/// M3.1 T3.1.1 — build a `.env.example` proposal from selected vault keys.
///
/// Reads each key's METADATA only (provider + label). Never calls
/// `get_key_value`. Derives env var NAMES via `default_env_name` (the
/// same logic used by `export_runtime_profile`). Optional Holster source
/// comments reference provider/label, never values.
///
/// Requires an unlocked vault.
#[tauri::command]
fn env_example_from_vault(
    args: EnvExampleFromVaultArgs,
    state: State<'_, AppState>,
) -> Result<env_example::EnvExampleProposal, String> {
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

    let ids: Vec<Uuid> = args
        .key_ids
        .iter()
        .map(|id| Uuid::parse_str(id).map_err(|_| "invalid key id".to_string()))
        .collect::<Result<_, _>>()?;

    let mut used: Vec<String> = Vec::with_capacity(ids.len());
    let mut lines: Vec<env_example::EnvExampleLine> = Vec::with_capacity(ids.len());
    for id in &ids {
        let meta = metas
            .iter()
            .find(|m| m.id == *id)
            .ok_or_else(|| err_to_string(VaultError::KeyNotFound(*id)))?;
        let env_name = default_env_name(meta.provider, &meta.label, &used);
        used.push(env_name.clone());
        let comment = if args.include_holster_comments {
            Some(format!(
                "stored in Holster as {} / {}",
                meta.provider.as_str(),
                meta.label
            ))
        } else {
            None
        };
        lines.push(env_example::EnvExampleLine {
            name: env_name,
            comment,
        });
    }

    let parsed_count = lines.len();
    Ok(env_example::EnvExampleProposal {
        source_kind: "vault".into(),
        source_label: format!(
            "Vault — {} key{}",
            parsed_count,
            if parsed_count == 1 { "" } else { "s" }
        ),
        lines,
        parsed_count,
        skipped_count: 0,
    })
}

/// M3.1 T3.1.1 — build a `.env.example` proposal from an existing
/// `.env*` file. Reads only var NAMES; the parser stops at the first
/// `=` of each line and discards the value. Refuses non-`.env*`
/// basenames and files larger than 5 MB.
///
/// Does NOT require an unlocked vault.
#[tauri::command]
fn env_example_from_file(
    args: env_example::EnvExampleFromFileArgs,
) -> Result<env_example::EnvExampleProposal, String> {
    env_example::read_env_file_for_proposal(&args)
}

/// M3.1 T3.1.1 — write a `.env.example` to disk atomically. Refuses to
/// overwrite an existing file unless `overwrite=true`. Refuses to
/// write into skip dirs (.git, node_modules, etc.). chmod 0644.
/// Appends an audit log entry to `runtime-export-audit.jsonl` with
/// `kind: "env_example_generated"` (names + path only, never values).
///
/// Does NOT require an unlocked vault.
#[tauri::command]
fn env_example_apply(
    args: env_example::EnvExampleApplyArgs,
    app: AppHandle,
) -> Result<env_example::EnvExampleApplyReport, String> {
    let audit_path = export_audit_log_path(&app)?;
    let mut writer = |payload: &serde_json::Value| -> Result<Option<String>, String> {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&audit_path)
            .map_err(|e| format!("could not open audit log: {e}"))?;
        writeln!(file, "{payload}").map_err(|e| format!("could not write audit log: {e}"))?;
        // Best-effort 0600 on the audit log itself
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&audit_path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(Some(audit_path.display().to_string()))
    };
    env_example::apply_to_disk(&args, &mut writer)
}

/// M3.1 T3.1.3 — return the static agent runtime profile catalogue.
///
/// Pure UX helper. The frontend uses the returned list to populate a
/// dropdown that prefills the runtime export dialog with sensible
/// defaults for OpenClaw / Claude Code / Codex / Hermes. The actual
/// export still goes through `export_runtime_profile` with all its
/// existing hardening intact.
#[tauri::command]
fn list_agent_profiles() -> Vec<agent_profiles::AgentProfile> {
    agent_profiles::agent_profile_catalog().to_vec()
}

/// M4 — list Holster Auth accounts.
///
/// TOTP secrets and backup codes remain encrypted in the vault. The returned
/// DTO contains only metadata plus backup-code count.
#[tauri::command]
fn list_totp_accounts(state: State<'_, AppState>) -> Result<Vec<auth::TotpAccountDto>, String> {
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

    let mut accounts = Vec::new();
    for meta in metas {
        if meta.project_tag.as_deref() != Some(auth::AUTH_PROJECT_TAG) {
            continue;
        }
        let secret = v.get_key_value(token, meta.id).map_err(err_to_string)?;
        let record = auth::secret_string_to_record(&secret)?;
        accounts.push(auth::TotpAccountDto {
            id: meta.id.to_string(),
            label: meta.label,
            issuer: record.issuer,
            account_name: record.account_name,
            backup_code_count: record.backup_codes.len(),
            created_at: meta.created_at,
            last_used_at: meta.last_used_at,
        });
    }
    Ok(accounts)
}

/// M4 — add a TOTP account from a manual secret or otpauth:// URI.
///
/// The plaintext secret is accepted transiently and immediately wrapped into
/// the existing encrypted vault record path. It is never logged or returned.
#[tauri::command]
fn add_totp_account(
    args: auth::AddTotpAccountArgs,
    state: State<'_, AppState>,
) -> Result<auth::TotpAccountDto, String> {
    let label = args.label.trim();
    if label.is_empty() {
        return Err("label cannot be empty".into());
    }

    let mut record = auth::normalize_auth_input(
        &args.secret_or_uri,
        args.issuer.as_deref(),
        args.account_name.as_deref(),
    )?;
    record.backup_codes = auth::parse_backup_codes(args.backup_codes.as_deref());
    let key_value = auth::record_to_secret_string(&record)?;

    let session = state.session.lock().map_err(|_| "state lock poisoned")?;
    let vault = state.vault.lock().map_err(|_| "state lock poisoned")?;
    let token = session
        .as_ref()
        .copied()
        .ok_or_else(|| err_to_string(VaultError::InvalidSession))?;
    let v = vault
        .as_ref()
        .ok_or_else(|| err_to_string(VaultError::Locked))?;

    let meta = v
        .add_key(
            token,
            AddKeyInput {
                provider: Provider::Generic,
                label: label.to_string(),
                key_value,
                project_tag: Some(auth::AUTH_PROJECT_TAG.to_string()),
                expires_at: None,
                notes: Some("Holster Auth TOTP account".to_string()),
            },
        )
        .map_err(err_to_string)?;

    Ok(auth::TotpAccountDto {
        id: meta.id.to_string(),
        label: meta.label,
        issuer: record.issuer,
        account_name: record.account_name,
        backup_code_count: record.backup_codes.len(),
        created_at: meta.created_at,
        last_used_at: meta.last_used_at,
    })
}

/// M4 — generate the current 6-digit TOTP code for one account.
///
/// This is the only Auth command that returns a short-lived secret. It returns
/// only the computed code, never the underlying TOTP secret or backup codes.
#[tauri::command]
fn get_totp_code(id: String, state: State<'_, AppState>) -> Result<auth::TotpCodeReport, String> {
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
    let metas = v.list_keys(token).map_err(err_to_string)?;
    let meta = metas
        .iter()
        .find(|m| m.id == uuid)
        .ok_or_else(|| err_to_string(VaultError::KeyNotFound(uuid)))?;
    if meta.project_tag.as_deref() != Some(auth::AUTH_PROJECT_TAG) {
        return Err("selected vault record is not a Holster Auth account".into());
    }
    let secret = v.get_key_value(token, uuid).map_err(err_to_string)?;
    let record = auth::secret_string_to_record(&secret)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "system clock is before unix epoch".to_string())?
        .as_secs();
    auth::current_totp(&record.secret_base32, now)
}

/// M3.1 T3.1.2 — read-only audit of a project's `.gitignore`.
///
/// Detects project type (node / python / rust / generic) and returns the
/// catalogue of rule sets Holster proposes, with `already_present` flags
/// per rule. Does NOT write or create any file. Vault state is irrelevant.
#[tauri::command]
fn gitignore_audit(
    args: gitignore_helper::GitignoreAuditArgs,
) -> Result<gitignore_helper::GitignoreAuditReport, String> {
    gitignore_helper::audit(args)
}

/// M3.1 T3.1.2 — apply user-confirmed `.gitignore` additions atomically.
///
/// Append-only. Re-validates rule-set membership and dedupes against the
/// current file content at apply time. Refuses to create an empty file
/// when there's nothing to add. Does NOT require an unlocked vault.
#[tauri::command]
fn gitignore_apply(
    args: gitignore_helper::GitignoreApplyArgs,
) -> Result<gitignore_helper::GitignoreApplyReport, String> {
    gitignore_helper::apply(args)
}

/// M3 — scan a local project folder for leaked secrets.
///
/// Pure read-only directory walk + in-memory regex matching. The Tauri
/// command boundary surfaces only redacted previews; the raw matched bytes
/// never leave the Rust process. See `repo_scanner` module docs for the
/// security contract.
///
/// Does NOT require an unlocked vault — secret detection is a defensive
/// audit, not a vault operation. The vault state is irrelevant here.
#[tauri::command]
fn scan_project_for_secrets(
    args: repo_scanner::ScanArgs,
) -> Result<repo_scanner::ScanReport, String> {
    repo_scanner::scan_local_path(args)
}

// ── MCP Preflight commands (V0.5) ────────────────────────────────────────────
//
// IPC wrapper around the `mcp_preflight` analyzer module. Two surfaces:
//   1. `analyze_mcp_config_cmd` — single-entry analyzer. Frontend pastes a
//      JSON entry and gets back a verdict + findings.
//   2. `analyze_claude_desktop_config_cmd` — batch analyzer over the
//      `mcpServers` map in a Claude Desktop config file. Defaults to the
//      standard macOS path; caller can override for testing.
//
// Both commands map the analyzer's typed error into a user-safe `String`
// per the existing M2 pattern. The analyzer module itself does NO file
// I/O; the file read lives here, in the IPC layer, where it can be
// permission-checked by Tauri's allowlist if we tighten things later.

/// Analyze a single MCP server config entry and return a verdict + findings.
///
/// `name` is optional — when supplied, it populates `report.server_name`
/// so the frontend can render multi-server tables.
#[tauri::command]
fn analyze_mcp_config_cmd(
    name: Option<String>,
    json: String,
) -> Result<mcp_preflight::McpPreflightReport, String> {
    let result = match name.as_deref() {
        Some(n) => mcp_preflight::analyze_mcp_config_named(n, &json),
        None => mcp_preflight::analyze_mcp_config(&json),
    };
    result.map_err(map_mcp_preflight_error)
}

#[derive(Serialize, Debug)]
pub struct McpPreflightBatchEntry {
    pub server_name: String,
    pub report: Option<mcp_preflight::McpPreflightReport>,
    pub error: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct McpPreflightBatchReport {
    pub config_path: String,
    pub config_found: bool,
    pub entries: Vec<McpPreflightBatchEntry>,
    pub parse_error: Option<String>,
}

/// Read a Claude Desktop–style config file and analyze every server entry
/// under the `mcpServers` map.
///
/// `path` is optional — when omitted, defaults to the standard macOS path
/// `~/Library/Application Support/Claude/claude_desktop_config.json`.
///
/// Returns a batch report with one entry per server. A per-server analyzer
/// error becomes `entry.error`; a file-level read or parse error becomes
/// `parse_error` at the top level.
#[tauri::command]
fn analyze_claude_desktop_config_cmd(
    path: Option<String>,
) -> Result<McpPreflightBatchReport, String> {
    let config_path = match path {
        Some(p) => PathBuf::from(p),
        None => default_claude_desktop_config_path()?,
    };
    let path_str = config_path.display().to_string();

    if !config_path.exists() {
        return Ok(McpPreflightBatchReport {
            config_path: path_str,
            config_found: false,
            entries: Vec::new(),
            parse_error: None,
        });
    }

    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("could not read {path_str}: {e}"))?;

    let parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            return Ok(McpPreflightBatchReport {
                config_path: path_str,
                config_found: true,
                entries: Vec::new(),
                parse_error: Some(format!("invalid JSON: {e}")),
            });
        }
    };

    let entries = parsed
        .get("mcpServers")
        .and_then(|v| v.as_object())
        .map(|map| {
            map.iter()
                .map(|(name, entry_value)| {
                    let entry_json = entry_value.to_string();
                    match mcp_preflight::analyze_mcp_config_named(name, &entry_json) {
                        Ok(report) => McpPreflightBatchEntry {
                            server_name: name.clone(),
                            report: Some(report),
                            error: None,
                        },
                        Err(e) => McpPreflightBatchEntry {
                            server_name: name.clone(),
                            report: None,
                            error: Some(map_mcp_preflight_error(e)),
                        },
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(McpPreflightBatchReport {
        config_path: path_str,
        config_found: true,
        entries,
        parse_error: None,
    })
}

fn default_claude_desktop_config_path() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME env var not set".to_string())?;
    Ok(PathBuf::from(home).join("Library/Application Support/Claude/claude_desktop_config.json"))
}

/// Expand `~/...` shorthand to `$HOME/...`. Leaves other paths untouched.
/// Used at every IPC boundary that accepts a user-supplied filesystem path.
fn expand_home_path(input: &str) -> PathBuf {
    if input == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home);
        }
        return PathBuf::from(input);
    }
    if let Some(rest) = input.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(input)
}

fn map_mcp_preflight_error(err: mcp_preflight::McpPreflightError) -> String {
    match err {
        mcp_preflight::McpPreflightError::InvalidJson(msg) => format!("Invalid JSON: {msg}"),
        mcp_preflight::McpPreflightError::MissingField(field) => {
            format!("Missing required field: {field}")
        }
        mcp_preflight::McpPreflightError::UnsupportedTransport(t) => {
            format!("Unsupported transport: {t}")
        }
    }
}

#[cfg(test)]
mod mcp_preflight_cmd_tests {
    use super::*;

    fn temp_path(label: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("holster-mcp-cmd-{label}-{nanos}.json"))
    }

    #[test]
    fn analyze_mcp_config_cmd_with_valid_named_input_returns_report() {
        let json = r#"{"command": "/usr/local/bin/server", "args": [], "env": {}}"#;
        let result = analyze_mcp_config_cmd(Some("test-server".into()), json.into()).unwrap();
        assert_eq!(result.server_name.as_deref(), Some("test-server"));
    }

    #[test]
    fn analyze_mcp_config_cmd_with_invalid_json_returns_string_error() {
        let result = analyze_mcp_config_cmd(None, "not json at all".into());
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(msg.starts_with("Invalid JSON"), "got: {msg}");
    }

    #[test]
    fn analyze_claude_desktop_config_cmd_missing_file_returns_not_found() {
        let path = temp_path("missing");
        let report = analyze_claude_desktop_config_cmd(Some(path.display().to_string())).unwrap();
        assert!(!report.config_found);
        assert!(report.entries.is_empty());
        assert!(report.parse_error.is_none());
    }

    #[test]
    fn analyze_claude_desktop_config_cmd_valid_config_returns_per_server_reports() {
        let path = temp_path("valid");
        let body = r#"{
            "mcpServers": {
                "fs": {
                    "command": "npx",
                    "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp/FAKE"]
                },
                "safe": {
                    "command": "/usr/local/bin/server-FAKE",
                    "args": [],
                    "env": {}
                }
            }
        }"#;
        std::fs::write(&path, body).unwrap();

        let report = analyze_claude_desktop_config_cmd(Some(path.display().to_string())).unwrap();
        assert!(report.config_found);
        assert_eq!(report.entries.len(), 2);
        for entry in &report.entries {
            assert!(
                entry.error.is_none(),
                "entry {} had error: {:?}",
                entry.server_name,
                entry.error
            );
            assert!(entry.report.is_some());
        }

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn analyze_claude_desktop_config_cmd_invalid_json_returns_parse_error_not_panic() {
        let path = temp_path("badjson");
        std::fs::write(&path, "not json").unwrap();

        let report = analyze_claude_desktop_config_cmd(Some(path.display().to_string())).unwrap();
        assert!(report.config_found);
        assert!(
            report.parse_error.is_some(),
            "expected parse_error to be set"
        );
        assert!(report.entries.is_empty());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn map_mcp_preflight_error_covers_all_variants() {
        let invalid =
            map_mcp_preflight_error(mcp_preflight::McpPreflightError::InvalidJson("bad".into()));
        assert!(invalid.contains("Invalid JSON"));

        let missing =
            map_mcp_preflight_error(mcp_preflight::McpPreflightError::MissingField("command"));
        assert!(missing.contains("Missing required field"));

        let unsupported = map_mcp_preflight_error(
            mcp_preflight::McpPreflightError::UnsupportedTransport("websocket".into()),
        );
        assert!(unsupported.contains("Unsupported transport"));
    }
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
            export_runtime_profile,
            scan_project_for_secrets,
            gitignore_audit,
            gitignore_apply,
            list_agent_profiles,
            env_example_from_vault,
            env_example_from_file,
            env_example_apply,
            list_totp_accounts,
            add_totp_account,
            get_totp_code,
            analyze_mcp_config_cmd,
            analyze_claude_desktop_config_cmd,
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

// ── Tests ────────────────────────────────────────────────────────────────────
//
// Focused unit tests for the pure helpers introduced by the runtime-export
// feature. All test inputs are dummy strings — no real provider keys, no
// real secrets. Integration tests (vault fixture + Tauri app handle) are out
// of scope for this unit suite; add a fixture-based suite in M3.

#[cfg(test)]
mod runtime_export_tests {
    use super::*;

    // ── sanitize_env_name ────────────────────────────────────────────────────

    #[test]
    fn sanitize_uppercases_and_underscores() {
        assert_eq!(sanitize_env_name("My Project Name"), "MY_PROJECT_NAME");
        assert_eq!(sanitize_env_name("hyphen-cased"), "HYPHEN_CASED");
        assert_eq!(sanitize_env_name("dotted.name"), "DOTTED_NAME");
    }

    #[test]
    fn sanitize_collapses_runs_of_separators() {
        assert_eq!(sanitize_env_name("a    b"), "A_B");
        assert_eq!(sanitize_env_name("a---b---c"), "A_B_C");
    }

    #[test]
    fn sanitize_trims_leading_and_trailing_underscores() {
        assert_eq!(sanitize_env_name("  hi  "), "HI");
        assert_eq!(sanitize_env_name("---x---"), "X");
    }

    #[test]
    fn sanitize_empty_or_only_separators_falls_back_to_key() {
        assert_eq!(sanitize_env_name(""), "KEY");
        assert_eq!(sanitize_env_name("   "), "KEY");
        assert_eq!(sanitize_env_name("---"), "KEY");
    }

    #[test]
    fn sanitize_keeps_digits() {
        assert_eq!(sanitize_env_name("v2 service"), "V2_SERVICE");
    }

    // ── is_safe_env_filename ─────────────────────────────────────────────────

    #[test]
    fn safe_filename_accepts_canonical_names() {
        assert!(is_safe_env_filename(".env"));
        assert!(is_safe_env_filename(".env.local"));
        assert!(is_safe_env_filename("hermes.env"));
        assert!(is_safe_env_filename("project.env"));
    }

    #[test]
    fn safe_filename_rejects_path_traversal() {
        assert!(!is_safe_env_filename("../.env"));
        assert!(!is_safe_env_filename("/etc/.env"));
        assert!(!is_safe_env_filename("subdir/.env"));
        assert!(!is_safe_env_filename("a/b/c.env"));
    }

    #[test]
    fn safe_filename_rejects_empty_or_unrelated() {
        assert!(!is_safe_env_filename(""));
        assert!(!is_safe_env_filename("   "));
        assert!(!is_safe_env_filename("config.json"));
        assert!(!is_safe_env_filename("envfile")); // doesn't end with .env
    }

    // ── check_no_whitespace_bounds ───────────────────────────────────────────
    //
    // V0 hardening (2026-04-30 evening): hard-refuse leading/trailing
    // whitespace at add-key AND export time. Never silently trim.

    #[test]
    fn whitespace_check_passes_clean_value() {
        assert!(check_no_whitespace_bounds("sk-FAKE-fake-fake-1234567890").is_ok());
        assert!(check_no_whitespace_bounds("ANTHROPIC_FAKE_KEY_abc123").is_ok());
        assert!(check_no_whitespace_bounds("a").is_ok());
    }

    #[test]
    fn whitespace_check_rejects_leading_space() {
        let err = check_no_whitespace_bounds(" sk-FAKE-fake-fake-1234567890").unwrap_err();
        assert!(err.contains("starts with whitespace"));
    }

    #[test]
    fn whitespace_check_rejects_trailing_space() {
        let err = check_no_whitespace_bounds("sk-FAKE-fake-fake-1234567890 ").unwrap_err();
        assert!(err.contains("ends with whitespace"));
    }

    #[test]
    fn whitespace_check_rejects_leading_tab() {
        let err = check_no_whitespace_bounds("\tsk-FAKE-fake-fake-1234567890").unwrap_err();
        assert!(err.contains("starts with whitespace"));
    }

    #[test]
    fn whitespace_check_rejects_trailing_tab() {
        let err = check_no_whitespace_bounds("sk-FAKE-fake-fake-1234567890\t").unwrap_err();
        assert!(err.contains("ends with whitespace"));
    }

    #[test]
    fn whitespace_check_rejects_trailing_newline_paste() {
        // Common paste error: terminal copy includes the trailing line break.
        let err = check_no_whitespace_bounds("sk-FAKE-fake-fake-1234567890\n").unwrap_err();
        assert!(err.contains("ends with whitespace"));
    }

    #[test]
    fn whitespace_check_rejects_trailing_carriage_return() {
        let err = check_no_whitespace_bounds("sk-FAKE-fake-fake-1234567890\r").unwrap_err();
        assert!(err.contains("ends with whitespace"));
    }

    #[test]
    fn whitespace_check_passes_internal_whitespace() {
        // Internal whitespace is rare in real keys but not the target of this
        // check. Other safety nets (shell_quote_env_value rejects \n/\r/NUL)
        // catch the bytes that would actually corrupt the env file.
        assert!(check_no_whitespace_bounds("part1 part2").is_ok());
    }

    #[test]
    fn whitespace_check_passes_empty_string() {
        // Empty is handled separately by add_key's "key value cannot be empty"
        // check; this function is a no-op for empty so it doesn't double-error.
        assert!(check_no_whitespace_bounds("").is_ok());
    }

    // ── shell_quote_env_value ────────────────────────────────────────────────
    //
    // V0 hardening (2026-04-30):
    //   - Single-quoted form: 'value', so $VAR / backticks / $(...) are NOT
    //     expanded by dotenv-style or POSIX-shell-`source`-style readers.
    //   - Embedded `'` is escaped as `'\''` (close-escape-reopen).
    //   - Newline / carriage-return / NUL bytes in a secret are REJECTED
    //     with an Err — those would either split the secret across lines
    //     in the env file or otherwise corrupt parsing.

    #[test]
    fn quote_wraps_simple_value_in_single_quotes() {
        assert_eq!(shell_quote_env_value("abc").unwrap(), "'abc'");
    }

    #[test]
    fn quote_escapes_embedded_single_quote() {
        // POSIX-shell close-escape-reopen: ' is closed, escaped \', reopen '
        assert_eq!(shell_quote_env_value("a'b").unwrap(), r"'a'\''b'");
    }

    #[test]
    fn quote_handles_empty() {
        assert_eq!(shell_quote_env_value("").unwrap(), "''");
    }

    #[test]
    fn quote_passes_through_double_quote() {
        // Double quotes are not special inside single-quoted form.
        assert_eq!(shell_quote_env_value("a\"b").unwrap(), "'a\"b'");
    }

    #[test]
    fn quote_passes_through_backslash() {
        // Backslash is not an escape inside single-quoted form.
        assert_eq!(shell_quote_env_value(r"a\b").unwrap(), r"'a\b'");
    }

    #[test]
    fn quote_neutralizes_dollar_for_dotenv_expansion() {
        // `$VAR` must NOT be at-risk of expansion by dotenv/docker-compose
        // readers, which only expand inside DOUBLE quotes. Single-quoting
        // is the protection.
        let q = shell_quote_env_value("abc$VAR").unwrap();
        assert_eq!(q, "'abc$VAR'");
        assert!(
            q.starts_with('\'') && q.ends_with('\''),
            "expected single-quoted form to neutralize $VAR expansion, got {q:?}"
        );
    }

    #[test]
    fn quote_neutralizes_backticks() {
        let q = shell_quote_env_value("a`whoami`b").unwrap();
        assert_eq!(q, "'a`whoami`b'");
    }

    #[test]
    fn quote_rejects_newline() {
        let err = shell_quote_env_value("line1\nline2").unwrap_err();
        assert!(
            err.contains("newline"),
            "expected newline rejection, got {err}"
        );
    }

    #[test]
    fn quote_rejects_carriage_return() {
        let err = shell_quote_env_value("line1\rline2").unwrap_err();
        assert!(
            err.to_lowercase().contains("carriage"),
            "expected CR rejection, got {err}"
        );
    }

    #[test]
    fn quote_rejects_nul() {
        let err = shell_quote_env_value("a\0b").unwrap_err();
        assert!(
            err.to_uppercase().contains("NUL"),
            "expected NUL rejection, got {err}"
        );
    }

    #[test]
    fn quote_handles_unicode_passthrough() {
        // Non-ASCII printable bytes are fine — single-quoted form is byte-transparent.
        assert_eq!(shell_quote_env_value("café").unwrap(), "'café'");
    }

    // ── default_env_name ─────────────────────────────────────────────────────

    #[test]
    fn default_env_name_uses_provider_canonical_first() {
        let used: Vec<String> = vec![];
        assert_eq!(
            default_env_name(Provider::Anthropic, "ignored-label", &used),
            "ANTHROPIC_API_KEY"
        );
        assert_eq!(
            default_env_name(Provider::OpenAI, "ignored-label", &used),
            "OPENAI_API_KEY"
        );
    }

    #[test]
    fn default_env_name_handles_collision_with_label_suffix() {
        let used = vec!["ANTHROPIC_API_KEY".to_string()];
        assert_eq!(
            default_env_name(Provider::Anthropic, "Personal", &used),
            "ANTHROPIC_API_KEY_PERSONAL"
        );
    }

    #[test]
    fn default_env_name_handles_double_collision_with_numeric_suffix() {
        let used = vec![
            "ANTHROPIC_API_KEY".to_string(),
            "ANTHROPIC_API_KEY_PERSONAL".to_string(),
        ];
        assert_eq!(
            default_env_name(Provider::Anthropic, "Personal", &used),
            "ANTHROPIC_API_KEY_PERSONAL_2"
        );
    }

    #[test]
    fn default_env_name_generic_uses_label() {
        let used: Vec<String> = vec![];
        assert_eq!(
            default_env_name(Provider::Generic, "my-tool", &used),
            "MY_TOOL_API_KEY"
        );
    }

    // ── ensure_gitignore ─────────────────────────────────────────────────────

    fn unique_tempdir(label: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        p.push(format!("holster-test-{label}-{nanos}"));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn ensure_gitignore_creates_file_when_missing() {
        let dir = unique_tempdir("gitignore-create");
        let changed = ensure_gitignore(&dir).unwrap();
        assert!(changed);
        let content = std::fs::read_to_string(dir.join(".gitignore")).unwrap();
        assert!(content.contains(".env\n"));
        assert!(content.contains(".env.local\n"));
        assert!(content.contains("*.env\n"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ensure_gitignore_is_idempotent() {
        let dir = unique_tempdir("gitignore-idem");
        let _ = ensure_gitignore(&dir).unwrap();
        let changed_again = ensure_gitignore(&dir).unwrap();
        assert!(!changed_again, "second call should be a no-op");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ensure_gitignore_appends_without_trailing_newline() {
        let dir = unique_tempdir("gitignore-no-nl");
        std::fs::write(dir.join(".gitignore"), "node_modules").unwrap();
        let changed = ensure_gitignore(&dir).unwrap();
        assert!(changed);
        let content = std::fs::read_to_string(dir.join(".gitignore")).unwrap();
        assert!(content.starts_with("node_modules\n"));
        assert!(content.contains(".env.local\n"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ensure_gitignore_includes_holster_tmp_pattern() {
        // Crash-leftover atomic-write temps end with `.holster-tmp` and
        // must be covered by the gitignore patterns we add.
        let dir = unique_tempdir("gitignore-holster-tmp");
        let _ = ensure_gitignore(&dir).unwrap();
        let content = std::fs::read_to_string(dir.join(".gitignore")).unwrap();
        assert!(
            content.contains("*.holster-tmp\n"),
            "expected *.holster-tmp pattern in gitignore, got: {content}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── set_secret_file_perms ────────────────────────────────────────────────
    //
    // V0 hardening: chmod failure is a hard error. Verify the function
    // returns Err for a path that doesn't exist (chmod can't fix what
    // isn't there) — this is the closest portable proxy for "chmod
    // failed" we can write without mocking the filesystem.

    #[cfg(unix)]
    #[test]
    fn set_secret_file_perms_errors_on_missing_path() {
        let mut p = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        p.push(format!("holster-test-missing-{nanos}.tmp"));
        // Don't create the file — chmod should fail.
        let result = set_secret_file_perms(&p);
        assert!(result.is_err(), "expected Err on chmod of missing file");
    }

    #[cfg(unix)]
    #[test]
    fn set_secret_file_perms_succeeds_on_existing_file() {
        let dir = unique_tempdir("perms-ok");
        let f = dir.join("secret.test");
        std::fs::write(&f, "dummy").unwrap();
        let res = set_secret_file_perms(&f);
        assert!(
            res.is_ok(),
            "chmod should succeed on existing file: {res:?}"
        );
        // Verify perms actually got set
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&f).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "expected 0600, got {mode:o}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
