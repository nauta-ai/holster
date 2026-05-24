//! Holster CLI — test harness for the holster-vault crate.
//!
//! T1.8: minimal subcommands proving the full Vault API works end-to-end
//! from a real shell. Each invocation prompts for the master password
//! (no persistent session between calls — by design).
//!
//! Examples:
//!   holster create /tmp/test.db
//!   holster add /tmp/test.db --provider anthropic --label primary
//!   holster list /tmp/test.db
//!   holster get  /tmp/test.db <uuid>
//!   holster delete /tmp/test.db <uuid>

use std::collections::BTreeMap;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, ExitCode};

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Duration, Utc};
use clap::{Parser, Subcommand, ValueEnum};
use plist::Value;
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use holster_vault::{AddKeyInput, AuditEvent, KeyMetadata, KeyStatus, Provider, Vault};

#[derive(Parser)]
#[command(
    name = "holster",
    about = "Holster vault CLI — local-first API key manager",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Initialize a new vault at PATH. Prompts for password (entered twice).
    Create { path: PathBuf },
    /// Add a key to an existing vault.
    Add {
        path: PathBuf,
        #[arg(long, value_enum)]
        provider: ProviderArg,
        #[arg(long)]
        label: String,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        notes: Option<String>,
        /// v0.2.0: read master password from environment variable.
        #[arg(long)]
        password_env: Option<String>,
        /// v0.2.0: read master password from stdin (first line).
        #[arg(long)]
        password_stdin: bool,
        /// v0.2.0: read master password from macOS Keychain (service name).
        #[arg(long)]
        password_keychain_service: Option<String>,
        /// v0.2.0: macOS Keychain account paired with --password-keychain-service.
        #[arg(long)]
        password_keychain_account: Option<String>,
    },
    /// List metadata for all keys (no plaintext shown).
    List { path: PathBuf },
    /// Decrypt and print a key value.
    Get {
        path: PathBuf,
        id: Uuid,
        /// v0.2.0: read master password from environment variable.
        #[arg(long)]
        password_env: Option<String>,
        /// v0.2.0: read master password from stdin (first line).
        #[arg(long)]
        password_stdin: bool,
        /// v0.2.0: read master password from macOS Keychain (service name).
        #[arg(long)]
        password_keychain_service: Option<String>,
        /// v0.2.0: macOS Keychain account paired with --password-keychain-service.
        #[arg(long)]
        password_keychain_account: Option<String>,
    },
    /// v0.2.0: Rotate the vault master password.
    /// Re-encrypts every entry under a new master + regenerates the salt
    /// atomically. Optionally updates a macOS Keychain entry to cache the
    /// new password for daemon use.
    RotateMaster {
        path: PathBuf,
        /// Read OLD master password from environment variable.
        #[arg(long)]
        old_password_env: Option<String>,
        /// Read OLD master password from macOS Keychain (service name).
        #[arg(long)]
        old_password_keychain_service: Option<String>,
        /// macOS Keychain account paired with --old-password-keychain-service.
        #[arg(long)]
        old_password_keychain_account: Option<String>,
        /// Read OLD master password from stdin (first line).
        #[arg(long)]
        old_password_stdin: bool,
        /// Read NEW master password from environment variable
        /// (skips interactive confirm — caller is responsible for strength).
        #[arg(long)]
        new_password_env: Option<String>,
        /// Read NEW master password from stdin (second line if
        /// --old-password-stdin also set; otherwise first line).
        #[arg(long)]
        new_password_stdin: bool,
        /// After successful rotation, also update the named macOS Keychain
        /// entry to the NEW password so daemons that read it (e.g. via
        /// `exec-env --password-keychain-service X --password-keychain-account Y`)
        /// keep working without manual intervention. Format: SERVICE,ACCOUNT.
        #[arg(long)]
        keychain_update: Option<String>,
    },
    /// Delete a key by id.
    Delete { path: PathBuf, id: Uuid },
    /// Mark one vault entry as superseded by another.
    Supersede {
        path: PathBuf,
        old_id: Uuid,
        #[arg(long)]
        replacement: Uuid,
    },
    /// Print mutation audit events from the encrypted vault.
    AuditLog {
        path: PathBuf,
        #[arg(long, default_value_t = 30)]
        since_days: i64,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long = "account")]
        account: Option<String>,
    },
    /// Import secret-bearing environment variables from a launchd plist.
    ImportPlistEnv {
        path: PathBuf,
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        label_prefix: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        allow_duplicates: bool,
    },
    /// Import secret-bearing variables from a .env-style file.
    ImportEnv {
        path: PathBuf,
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        label_prefix: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        allow_duplicates: bool,
    },
    /// Import multiple launchd plist and .env sources with one unlock.
    ImportBatch {
        path: PathBuf,
        #[arg(long = "plist")]
        plists: Vec<PathBuf>,
        #[arg(long = "env")]
        envs: Vec<PathBuf>,
        #[arg(long)]
        project: Option<String>,
        #[arg(long)]
        label_prefix: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        allow_duplicates: bool,
    },
    /// Run a child process with env vars fetched from Holster.
    ExecEnv {
        path: PathBuf,
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        password_env: Option<String>,
        #[arg(long)]
        password_keychain_service: Option<String>,
        #[arg(long)]
        password_keychain_account: Option<String>,
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(ValueEnum, Clone, Copy)]
enum ProviderArg {
    Anthropic,
    Github,
    Openai,
    Google,
    Replicate,
    Elevenlabs,
    Pinecone,
    Stripe,
    Cloudflare,
    Generic,
}

impl From<ProviderArg> for Provider {
    fn from(p: ProviderArg) -> Self {
        match p {
            ProviderArg::Anthropic => Provider::Anthropic,
            ProviderArg::Github => Provider::GitHub,
            ProviderArg::Openai => Provider::OpenAI,
            ProviderArg::Google => Provider::Google,
            ProviderArg::Replicate => Provider::Replicate,
            ProviderArg::Elevenlabs => Provider::ElevenLabs,
            ProviderArg::Pinecone => Provider::Pinecone,
            ProviderArg::Stripe => Provider::Stripe,
            ProviderArg::Cloudflare => Provider::Cloudflare,
            ProviderArg::Generic => Provider::Generic,
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            // Print full chain for debugging
            for cause in e.chain().skip(1) {
                eprintln!("  caused by: {cause}");
            }
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Create { path } => cmd_create(&path),
        Command::Add {
            path,
            provider,
            label,
            project,
            notes,
            password_env,
            password_stdin,
            password_keychain_service,
            password_keychain_account,
        } => cmd_add(
            &path,
            provider.into(),
            label,
            project,
            notes,
            password_env.as_deref(),
            password_stdin,
            password_keychain_service.as_deref(),
            password_keychain_account.as_deref(),
        ),
        Command::List { path } => cmd_list(&path),
        Command::Get {
            path,
            id,
            password_env,
            password_stdin,
            password_keychain_service,
            password_keychain_account,
        } => cmd_get(
            &path,
            id,
            password_env.as_deref(),
            password_stdin,
            password_keychain_service.as_deref(),
            password_keychain_account.as_deref(),
        ),
        Command::RotateMaster {
            path,
            old_password_env,
            old_password_keychain_service,
            old_password_keychain_account,
            old_password_stdin,
            new_password_env,
            new_password_stdin,
            keychain_update,
        } => cmd_rotate_master(
            &path,
            old_password_env.as_deref(),
            old_password_keychain_service.as_deref(),
            old_password_keychain_account.as_deref(),
            old_password_stdin,
            new_password_env.as_deref(),
            new_password_stdin,
            keychain_update.as_deref(),
        ),
        Command::Delete { path, id } => cmd_delete(&path, id),
        Command::Supersede {
            path,
            old_id,
            replacement,
        } => cmd_supersede(&path, old_id, replacement),
        Command::AuditLog {
            path,
            since_days,
            json,
            provider,
            account,
        } => cmd_audit_log(
            &path,
            since_days,
            json,
            provider.as_deref(),
            account.as_deref(),
        ),
        Command::ImportPlistEnv {
            path,
            source,
            project,
            label_prefix,
            dry_run,
            allow_duplicates,
        } => cmd_import(
            &path,
            ImportSource::LaunchdPlist(source),
            project,
            label_prefix,
            dry_run,
            allow_duplicates,
        ),
        Command::ImportEnv {
            path,
            source,
            project,
            label_prefix,
            dry_run,
            allow_duplicates,
        } => cmd_import(
            &path,
            ImportSource::EnvFile(source),
            project,
            label_prefix,
            dry_run,
            allow_duplicates,
        ),
        Command::ImportBatch {
            path,
            plists,
            envs,
            project,
            label_prefix,
            dry_run,
            allow_duplicates,
        } => cmd_import_batch(
            &path,
            plists,
            envs,
            project,
            label_prefix,
            dry_run,
            allow_duplicates,
        ),
        Command::ExecEnv {
            path,
            manifest,
            password_env,
            password_keychain_service,
            password_keychain_account,
            dry_run,
        } => cmd_exec_env(
            &path,
            &manifest,
            password_env.as_deref(),
            password_keychain_service.as_deref(),
            password_keychain_account.as_deref(),
            dry_run,
        ),
    }
}

fn cmd_create(path: &std::path::Path) -> Result<()> {
    let pw = prompt_secret("New master password: ").context("reading password")?;
    let confirm = prompt_secret("Confirm: ").context("reading confirmation")?;
    if pw != confirm {
        return Err(anyhow!("passwords do not match"));
    }
    Vault::create(path, &pw).context("creating vault")?;
    println!("✓ vault created at {}", path.display());
    println!("  salt sidecar: {}", salt_path(path).display());
    Ok(())
}

fn cmd_add(
    path: &std::path::Path,
    provider: Provider,
    label: String,
    project: Option<String>,
    notes: Option<String>,
    password_env: Option<&str>,
    password_stdin: bool,
    password_keychain_service: Option<&str>,
    password_keychain_account: Option<&str>,
) -> Result<()> {
    let vault = Vault::open(path).context("opening vault")?;
    let pw = read_password_with_sources(
        password_env,
        password_keychain_service,
        password_keychain_account,
        password_stdin,
        "Master password: ",
    )?;
    let token = vault
        .unlock(&pw)
        .context("unlock failed (wrong password?)")?;

    let key_value = prompt_secret("Key value: ").context("reading key value")?;
    if key_value.is_empty() {
        return Err(anyhow!("empty key value"));
    }

    let input = AddKeyInput {
        provider,
        label,
        key_value,
        project_tag: project,
        expires_at: None,
        notes,
    };

    let meta = vault.add_key(token, input).context("adding key")?;
    println!("✓ added key");
    print_metadata(&meta);
    vault.lock(token).ok();
    Ok(())
}

fn cmd_list(path: &std::path::Path) -> Result<()> {
    let vault = Vault::open(path)?;
    let pw = prompt_secret("Master password: ")?;
    let token = vault
        .unlock(&pw)
        .context("unlock failed (wrong password?)")?;
    let metas = vault.list_keys(token).context("listing keys")?;
    if metas.is_empty() {
        println!("(no keys)");
    } else {
        println!("{} key(s):", metas.len());
        for m in &metas {
            print_metadata(m);
            println!();
        }
    }
    vault.lock(token).ok();
    Ok(())
}

fn cmd_get(
    path: &std::path::Path,
    id: Uuid,
    password_env: Option<&str>,
    password_stdin: bool,
    password_keychain_service: Option<&str>,
    password_keychain_account: Option<&str>,
) -> Result<()> {
    let vault = Vault::open(path)?;
    let pw = read_password_with_sources(
        password_env,
        password_keychain_service,
        password_keychain_account,
        password_stdin,
        "Master password: ",
    )?;
    let token = vault
        .unlock(&pw)
        .context("unlock failed (wrong password?)")?;
    let secret = vault
        .get_key_value(token, id)
        .context("getting key value")?;
    println!("{}", secret.expose_secret());
    vault.lock(token).ok();
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_rotate_master(
    path: &std::path::Path,
    old_password_env: Option<&str>,
    old_password_keychain_service: Option<&str>,
    old_password_keychain_account: Option<&str>,
    old_password_stdin: bool,
    new_password_env: Option<&str>,
    new_password_stdin: bool,
    keychain_update: Option<&str>,
) -> Result<()> {
    let vault = Vault::open(path).context("opening vault")?;

    // OLD password — flexible source (env / stdin / keychain / interactive).
    let old_pw = read_password_with_sources(
        old_password_env,
        old_password_keychain_service,
        old_password_keychain_account,
        old_password_stdin,
        "OLD master password: ",
    )?;

    // NEW password — if a non-interactive source is provided, accept it as-is.
    // Otherwise prompt twice with confirm.
    let new_pw = if new_password_env.is_some() || new_password_stdin {
        read_password_with_sources(
            new_password_env,
            None,
            None,
            new_password_stdin,
            "NEW master password: ",
        )?
    } else {
        let p1 = prompt_secret("NEW master password (min 8 chars): ")
            .context("reading new password")?;
        let p2 = prompt_secret("Confirm NEW master password: ")
            .context("reading new password confirm")?;
        if p1 != p2 {
            return Err(anyhow!("NEW master passwords do not match"));
        }
        p1
    };

    if new_pw.len() < 8 {
        return Err(anyhow!("NEW master password must be at least 8 characters"));
    }
    if new_pw == old_pw {
        return Err(anyhow!(
            "NEW master password is identical to OLD — rotation aborted"
        ));
    }

    eprintln!("rotating master password (re-encrypts every entry + rekeys SQLCipher) ...");
    let count = vault
        .rotate_master(&old_pw, &new_pw)
        .context("rotating master password")?;

    println!("✓ rotated master: {count} entries re-encrypted under new master");
    println!("  salt sidecar regenerated: {}", salt_path(path).display());
    println!("  audit event appended (kind: master_rotated)");

    // Optional: update macOS Keychain with the new password so daemons that
    // cache via --password-keychain-* keep working without manual re-entry.
    if let Some(spec) = keychain_update {
        let (service, account) = parse_keychain_update_spec(spec)?;
        update_keychain_password(service, account, &new_pw)?;
        println!("  ✓ Keychain entry updated: service={service} account={account}");
    } else {
        println!("  note: if you cache this master in Keychain for daemons,");
        println!("        run: security add-generic-password -U -s <SVC> -a <ACCT> -w '<NEW PW>'");
        println!("        OR re-run with --keychain-update SERVICE,ACCOUNT");
    }

    Ok(())
}

fn parse_keychain_update_spec(spec: &str) -> Result<(&str, &str)> {
    let (service, account) = spec
        .split_once(',')
        .ok_or_else(|| anyhow!("--keychain-update expects SERVICE,ACCOUNT (got: {spec:?})"))?;
    let service = service.trim();
    let account = account.trim();
    if service.is_empty() || account.is_empty() {
        return Err(anyhow!(
            "--keychain-update SERVICE and ACCOUNT must both be non-empty"
        ));
    }
    Ok((service, account))
}

#[cfg(target_os = "macos")]
fn update_keychain_password(service: &str, account: &str, password: &str) -> Result<()> {
    let status = ProcessCommand::new("/usr/bin/security")
        .arg("add-generic-password")
        .arg("-U")
        .arg("-s")
        .arg(service)
        .arg("-a")
        .arg(account)
        .arg("-w")
        .arg(password)
        .status()
        .context("running security add-generic-password -U")?;
    if !status.success() {
        return Err(anyhow!(
            "security add-generic-password -U failed (exit {:?})",
            status.code()
        ));
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn update_keychain_password(_service: &str, _account: &str, _password: &str) -> Result<()> {
    Err(anyhow!(
        "--keychain-update is only supported on macOS. Use OS-native credential storage on this platform."
    ))
}

fn cmd_delete(path: &std::path::Path, id: Uuid) -> Result<()> {
    let vault = Vault::open(path)?;
    let pw = prompt_secret("Master password: ")?;
    let token = vault
        .unlock(&pw)
        .context("unlock failed (wrong password?)")?;
    vault.delete_key(token, id).context("deleting key")?;
    println!("✓ deleted {id}");
    vault.lock(token).ok();
    Ok(())
}

fn cmd_supersede(path: &std::path::Path, old_id: Uuid, replacement: Uuid) -> Result<()> {
    let vault = Vault::open(path)?;
    let pw = prompt_secret("Master password: ")?;
    let token = vault
        .unlock(&pw)
        .context("unlock failed (wrong password?)")?;
    vault
        .mark_superseded(old_id, replacement)
        .with_context(|| format!("entry_not_found: {old_id} or {replacement}"))?;
    println!("superseded {old_id} -> {replacement}");
    vault.lock(token).ok();
    Ok(())
}

#[derive(Serialize)]
struct AuditLogOut {
    events: Vec<AuditEvent>,
    count: usize,
    window_days: i64,
    vault_path: String,
}

fn cmd_audit_log(
    path: &std::path::Path,
    since_days: i64,
    json: bool,
    provider: Option<&str>,
    account: Option<&str>,
) -> Result<()> {
    if since_days < 1 {
        return Err(anyhow!("since-days must be >= 1"));
    }
    let vault = Vault::open(path)?;
    let pw = prompt_secret("Master password: ")?;
    let token = vault
        .unlock(&pw)
        .context("unlock failed (wrong password?)")?;
    let cutoff = Utc::now() - Duration::days(since_days);
    let mut events = Vec::new();
    for event in vault.audit_events().context("reading audit events")? {
        let ts = DateTime::parse_from_rfc3339(&event.ts_utc)
            .map(|dt| dt.with_timezone(&Utc))
            .with_context(|| format!("bad audit timestamp: {}", event.ts_utc))?;
        if ts < cutoff {
            continue;
        }
        if let Some(provider) = provider {
            if !event
                .provider
                .as_deref()
                .map(|value| value.eq_ignore_ascii_case(provider))
                .unwrap_or(false)
            {
                continue;
            }
        }
        if let Some(account) = account {
            if !event
                .project
                .as_deref()
                .map(|value| value.eq_ignore_ascii_case(account))
                .unwrap_or(false)
            {
                continue;
            }
        }
        events.push(event);
    }
    vault.lock(token).ok();

    let vault_path = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string();
    if json {
        let out = AuditLogOut {
            count: events.len(),
            events,
            window_days: since_days,
            vault_path,
        };
        serde_json::to_writer_pretty(std::io::stdout(), &out)?;
        println!();
    } else {
        if events.is_empty() {
            println!("(no audit events)");
        } else {
            println!("{} event(s) in last {since_days} day(s):", events.len());
            for event in &events {
                print_audit_event(event);
            }
        }
    }
    Ok(())
}

enum ImportSource {
    LaunchdPlist(PathBuf),
    EnvFile(PathBuf),
}

struct ImportCandidate {
    name: String,
    value: String,
    provider: Provider,
    source_label: String,
}

fn cmd_import(
    path: &Path,
    source: ImportSource,
    project: Option<String>,
    label_prefix: Option<String>,
    dry_run: bool,
    allow_duplicates: bool,
) -> Result<()> {
    let candidates = read_import_candidates(&source)?;
    let source_path = match &source {
        ImportSource::LaunchdPlist(p) | ImportSource::EnvFile(p) => p,
    };

    println!(
        "source: {}",
        source_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("<unknown>")
    );
    println!("candidate secret vars: {}", candidates.len());
    for candidate in &candidates {
        println!(
            "  {} ({})",
            display_candidate_name(candidate),
            candidate.provider.as_str()
        );
    }

    if dry_run {
        println!("dry run only; no vault writes");
        return Ok(());
    }

    if candidates.is_empty() {
        println!("nothing to import");
        return Ok(());
    }

    let vault = Vault::open(path).context("opening vault")?;
    let pw = prompt_secret("Master password: ")?;
    let token = vault
        .unlock(&pw)
        .context("unlock failed (wrong password?)")?;
    let existing = vault.list_keys(token).context("listing existing keys")?;

    let mut added = 0usize;
    let mut skipped = 0usize;
    for candidate in candidates {
        let display_name = display_candidate_name(&candidate);
        let label = import_label(
            label_prefix.as_deref(),
            &candidate.source_label,
            &candidate.name,
        );
        let already_present = existing.iter().any(|meta| {
            meta.provider == candidate.provider
                && meta.label == label
                && meta.project_tag.as_deref() == project.as_deref()
        });

        if already_present && !allow_duplicates {
            println!("  skipped existing {display_name}");
            skipped += 1;
            continue;
        }

        let input = AddKeyInput {
            provider: candidate.provider,
            label,
            key_value: candidate.value,
            project_tag: project.clone(),
            expires_at: None,
            notes: Some(format!(
                "Imported from {}. Source path intentionally not stored in public reports.",
                source_kind(&source)
            )),
        };
        let meta = vault.add_key(token, input).context("adding imported key")?;
        println!("  added {display_name} as {}", meta.id);
        added += 1;
    }

    vault.lock(token).ok();
    println!("import complete: added={added} skipped={skipped}");
    Ok(())
}

fn cmd_import_batch(
    path: &Path,
    plists: Vec<PathBuf>,
    envs: Vec<PathBuf>,
    project: Option<String>,
    label_prefix: Option<String>,
    dry_run: bool,
    allow_duplicates: bool,
) -> Result<()> {
    if plists.is_empty() && envs.is_empty() {
        return Err(anyhow!("provide at least one --plist or --env source"));
    }

    let mut candidates = Vec::new();
    for source in plists {
        candidates.extend(read_import_candidates(&ImportSource::LaunchdPlist(source))?);
    }
    for source in envs {
        candidates.extend(read_import_candidates(&ImportSource::EnvFile(source))?);
    }

    println!("batch candidate secret vars: {}", candidates.len());
    for candidate in &candidates {
        println!(
            "  {} ({})",
            display_candidate_name(candidate),
            candidate.provider.as_str()
        );
    }

    if dry_run {
        println!("dry run only; no vault writes");
        return Ok(());
    }

    if candidates.is_empty() {
        println!("nothing to import");
        return Ok(());
    }

    let vault = Vault::open(path).context("opening vault")?;
    let pw = prompt_secret("Master password: ")?;
    let token = vault
        .unlock(&pw)
        .context("unlock failed (wrong password?)")?;
    let existing = vault.list_keys(token).context("listing existing keys")?;

    let mut added = 0usize;
    let mut skipped = 0usize;
    for candidate in candidates {
        let display_name = display_candidate_name(&candidate);
        let label = import_label(
            label_prefix.as_deref(),
            &candidate.source_label,
            &candidate.name,
        );
        let already_present = existing.iter().any(|meta| {
            meta.provider == candidate.provider
                && meta.label == label
                && meta.project_tag.as_deref() == project.as_deref()
        });

        if already_present && !allow_duplicates {
            println!("  skipped existing {display_name}");
            skipped += 1;
            continue;
        }

        let input = AddKeyInput {
            provider: candidate.provider,
            label,
            key_value: candidate.value,
            project_tag: project.clone(),
            expires_at: None,
            notes: Some("Imported from MoonShot batch migration. Source path intentionally not stored in public reports.".into()),
        };
        let meta = vault.add_key(token, input).context("adding imported key")?;
        println!("  added {display_name} as {}", meta.id);
        added += 1;
    }

    vault.lock(token).ok();
    println!("batch import complete: added={added} skipped={skipped}");
    Ok(())
}

fn source_kind(source: &ImportSource) -> &'static str {
    match source {
        ImportSource::LaunchdPlist(_) => "launchd plist",
        ImportSource::EnvFile(_) => "env file",
    }
}

fn read_import_candidates(source: &ImportSource) -> Result<Vec<ImportCandidate>> {
    let source_label = match source {
        ImportSource::LaunchdPlist(path) | ImportSource::EnvFile(path) => source_label(path),
    };
    let vars = match source {
        ImportSource::LaunchdPlist(path) => read_launchd_env(path)?,
        ImportSource::EnvFile(path) => read_env_file(path)?,
    };

    let mut candidates = Vec::new();
    for (name, value) in vars {
        if !is_secret_var(&name) || value.trim().is_empty() {
            continue;
        }
        candidates.push(ImportCandidate {
            provider: provider_for_var(&name),
            name,
            value,
            source_label: source_label.clone(),
        });
    }
    Ok(candidates)
}

fn read_launchd_env(path: &Path) -> Result<BTreeMap<String, String>> {
    let value = Value::from_file(path).with_context(|| format!("reading {}", path.display()))?;
    let root = value
        .as_dictionary()
        .ok_or_else(|| anyhow!("plist root must be a dictionary"))?;
    let env = root
        .get("EnvironmentVariables")
        .and_then(Value::as_dictionary)
        .ok_or_else(|| anyhow!("plist has no EnvironmentVariables dictionary"))?;

    let mut vars = BTreeMap::new();
    for (key, value) in env {
        if let Some(s) = value.as_string() {
            vars.insert(key.to_string(), s.to_string());
        }
    }
    Ok(vars)
}

fn read_env_file(path: &Path) -> Result<BTreeMap<String, String>> {
    let body =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut vars = BTreeMap::new();
    for line in body.lines() {
        let mut trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("export ") {
            trimmed = rest.trim_start();
        }
        let Some((key, raw_value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty()
            || !key
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        {
            continue;
        }
        vars.insert(key.to_string(), unquote_env_value(raw_value.trim()));
    }
    Ok(vars)
}

fn unquote_env_value(raw: &str) -> String {
    if raw.len() >= 2 {
        let bytes = raw.as_bytes();
        let first = bytes[0];
        let last = bytes[raw.len() - 1];
        if (first == b'\'' && last == b'\'') || (first == b'"' && last == b'"') {
            return raw[1..raw.len() - 1].to_string();
        }
    }
    raw.to_string()
}

fn is_secret_var(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    upper.contains("KEY")
        || upper.contains("TOKEN")
        || upper.contains("SECRET")
        || upper.contains("PASSWORD")
        || upper.ends_with("_PASS")
}

fn provider_for_var(name: &str) -> Provider {
    let upper = name.to_ascii_uppercase();
    if upper.contains("ANTHROPIC") || upper.contains("CLAUDE") {
        Provider::Anthropic
    } else if upper.contains("OPENAI") {
        Provider::OpenAI
    } else if upper.contains("GEMINI") || upper.contains("GOOGLE") {
        Provider::Google
    } else if upper.contains("REPLICATE") {
        Provider::Replicate
    } else if upper.contains("ELEVEN") {
        Provider::ElevenLabs
    } else if upper.contains("PINECONE") {
        Provider::Pinecone
    } else if upper.contains("STRIPE") {
        Provider::Stripe
    } else if upper.contains("CLOUDFLARE") {
        Provider::Cloudflare
    } else {
        Provider::Generic
    }
}

fn import_label(prefix: Option<&str>, source_label: &str, name: &str) -> String {
    match prefix {
        Some(prefix) if !prefix.trim().is_empty() => {
            format!("{}:{}:{}", prefix.trim(), source_label, name)
        }
        _ => format!("{source_label}:{name}"),
    }
}

fn source_label(path: &Path) -> String {
    let mut parts = Vec::new();
    if let Some(grandparent) = path
        .parent()
        .and_then(Path::parent)
        .and_then(Path::file_name)
        .and_then(|s| s.to_str())
    {
        parts.push(grandparent);
    }
    if let Some(parent) = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|s| s.to_str())
    {
        parts.push(parent);
    }
    if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
        parts.push(name.trim_start_matches('.'));
    }
    sanitize_label(&parts.join("-"))
}

fn sanitize_label(raw: &str) -> String {
    let mut out = String::new();
    let mut last_sep = false;
    for ch in raw.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else {
            '-'
        };
        if mapped == '-' {
            if !last_sep {
                out.push(mapped);
                last_sep = true;
            }
        } else {
            out.push(mapped);
            last_sep = false;
        }
    }
    out.trim_matches('-').to_string()
}

fn display_candidate_name(candidate: &ImportCandidate) -> String {
    format!("{}:{}", candidate.source_label, candidate.name)
}

#[derive(Debug, Deserialize)]
struct ExecManifest {
    agent_id: String,
    audit_path: PathBuf,
    env: Vec<ExecEnvVar>,
    command: Vec<String>,
    working_directory: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct ExecEnvVar {
    name: String,
    provider: Option<Provider>,
    project: Option<String>,
    label: String,
}

fn cmd_exec_env(
    vault_path: &Path,
    manifest_path: &Path,
    password_env: Option<&str>,
    password_keychain_service: Option<&str>,
    password_keychain_account: Option<&str>,
    dry_run: bool,
) -> Result<()> {
    let manifest = read_exec_manifest(manifest_path)?;
    validate_exec_manifest(&manifest)?;

    println!("agent: {}", manifest.agent_id);
    println!("env vars requested: {}", manifest.env.len());
    for spec in &manifest.env {
        println!("  {}", spec.name);
    }
    println!("command: {}", manifest.command[0]);

    if dry_run {
        println!("dry run only; no vault unlock and no child process");
        return Ok(());
    }

    let password = read_runtime_password(
        password_env,
        password_keychain_service,
        password_keychain_account,
    )?;
    let vault = Vault::open(vault_path).context("opening vault")?;
    let token = vault
        .unlock(&password)
        .context("unlock failed (wrong password?)")?;
    let metadata = vault.list_keys(token).context("listing keys")?;

    let mut child_env = BTreeMap::new();
    for spec in &manifest.env {
        let id = resolve_manifest_key(spec, &metadata)?;
        let secret = vault
            .get_key_value(token, id)
            .with_context(|| format!("fetching key for {}", spec.name))?;
        child_env.insert(spec.name.clone(), secret.expose_secret().to_string());
        drop(secret);
    }

    let audit_path = &manifest.audit_path;
    if let Some(parent) = audit_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating audit dir {}", parent.display()))?;
        set_owner_only_dir(parent);
    }
    append_exec_audit(audit_path, &manifest.agent_id, &manifest.env)?;
    set_owner_only_file(audit_path);

    let status = run_manifest_child(&manifest, &child_env)?;
    vault.lock(token).ok();
    std::process::exit(status);
}

fn read_exec_manifest(path: &Path) -> Result<ExecManifest> {
    ensure_owner_only_path(path)?;
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

fn validate_exec_manifest(manifest: &ExecManifest) -> Result<()> {
    if manifest.agent_id.trim().is_empty() {
        return Err(anyhow!("manifest agent_id is required"));
    }
    if manifest.env.is_empty() {
        return Err(anyhow!("manifest env list is empty"));
    }
    if manifest.command.is_empty() || manifest.command[0].trim().is_empty() {
        return Err(anyhow!("manifest command is required"));
    }
    for spec in &manifest.env {
        if spec.name.trim().is_empty() {
            return Err(anyhow!("manifest env var name is required"));
        }
        if !spec
            .name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        {
            return Err(anyhow!(
                "env var {} must be uppercase env-name shaped",
                spec.name
            ));
        }
        if spec.label.trim().is_empty() {
            return Err(anyhow!("manifest label for {} is required", spec.name));
        }
    }
    Ok(())
}

fn read_runtime_password(
    password_env: Option<&str>,
    password_keychain_service: Option<&str>,
    password_keychain_account: Option<&str>,
) -> Result<String> {
    read_password_with_sources(
        password_env,
        password_keychain_service,
        password_keychain_account,
        false,
        "Master password: ",
    )
}

/// v0.2.0: unified password-source resolver used by `get`, `add`, `rotate-master`,
/// and `exec-env`. Order of precedence (first non-None wins):
///   1. `--password-env <NAME>` — read from environment variable
///   2. `--password-stdin` — read the first line of stdin
///   3. `--password-keychain-service <SVC> [--password-keychain-account <ACCT>]`
///      — macOS Keychain (errors on non-macOS if requested)
///   4. Interactive TTY prompt (fallback)
fn read_password_with_sources(
    password_env: Option<&str>,
    password_keychain_service: Option<&str>,
    password_keychain_account: Option<&str>,
    password_stdin: bool,
    prompt: &str,
) -> Result<String> {
    if let Some(name) = password_env {
        let value =
            std::env::var(name).with_context(|| format!("reading password env var {name}"))?;
        if value.is_empty() {
            return Err(anyhow!("password env var {name} is empty"));
        }
        return Ok(value);
    }

    if password_stdin {
        let mut line = String::new();
        std::io::stdin()
            .lock()
            .read_line(&mut line)
            .context("reading password from stdin")?;
        let pw = line.trim_end_matches(['\r', '\n']).to_string();
        if pw.is_empty() {
            return Err(anyhow!("stdin password was empty"));
        }
        return Ok(pw);
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = password_keychain_account;
        if password_keychain_service.is_some() {
            anyhow::bail!(
                "--password-keychain-service is only supported on macOS. \
                 On Linux/Windows, use --password-env <ENV_NAME> or --password-stdin."
            );
        }
    }

    #[cfg(target_os = "macos")]
    if let Some(service) = password_keychain_service {
        let mut cmd = ProcessCommand::new("/usr/bin/security");
        cmd.arg("find-generic-password")
            .arg("-w")
            .arg("-s")
            .arg(service);
        if let Some(account) = password_keychain_account {
            cmd.arg("-a").arg(account);
        }
        let output = cmd
            .output()
            .context("running security find-generic-password")?;
        if !output.status.success() {
            return Err(anyhow!(
                "could not read Holster password from Keychain (service={service})"
            ));
        }
        let password = String::from_utf8(output.stdout)
            .context("keychain password was not utf-8")?
            .trim_end_matches(['\r', '\n'])
            .to_string();
        if password.is_empty() {
            return Err(anyhow!("keychain password was empty"));
        }
        return Ok(password);
    }

    prompt_secret(prompt).context("reading password")
}

fn prompt_secret(prompt: &str) -> std::io::Result<String> {
    match rpassword::prompt_password(prompt) {
        Ok(value) => Ok(value),
        Err(err) if err.raw_os_error() == Some(6) => {
            let mut line = String::new();
            std::io::stdin().lock().read_line(&mut line)?;
            Ok(line.trim_end_matches(['\r', '\n']).to_string())
        }
        Err(err) => Err(err),
    }
}

fn resolve_manifest_key(spec: &ExecEnvVar, metadata: &[KeyMetadata]) -> Result<Uuid> {
    let matches: Vec<&KeyMetadata> = metadata
        .iter()
        .filter(|meta| {
            meta.label == spec.label
                && spec
                    .project
                    .as_deref()
                    .map_or(true, |project| meta.project_tag.as_deref() == Some(project))
                && spec
                    .provider
                    .map_or(true, |provider| meta.provider == provider)
        })
        .collect();

    match matches.as_slice() {
        [meta] => Ok(meta.id),
        [] => Err(anyhow!(
            "no Holster key metadata matched env var {}",
            spec.name
        )),
        _ => Err(anyhow!(
            "multiple Holster key metadata rows matched env var {}",
            spec.name
        )),
    }
}

fn run_manifest_child(
    manifest: &ExecManifest,
    child_env: &BTreeMap<String, String>,
) -> Result<i32> {
    let mut child = ProcessCommand::new(&manifest.command[0]);
    if manifest.command.len() > 1 {
        child.args(&manifest.command[1..]);
    }
    if let Some(cwd) = &manifest.working_directory {
        child.current_dir(cwd);
    }
    child.envs(child_env);

    let status = child.status().context("running child process")?;
    Ok(status.code().unwrap_or(1))
}

fn append_exec_audit(path: &Path, agent_id: &str, specs: &[ExecEnvVar]) -> Result<()> {
    let event = serde_json::json!({
        "kind": "exec_env",
        "agent_id": agent_id,
        "env_names": specs.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        "labels": specs.iter().map(|s| s.label.as_str()).collect::<Vec<_>>(),
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("opening audit {}", path.display()))?;
    writeln!(file, "{event}").context("writing exec audit")?;
    Ok(())
}

fn ensure_owner_only_path(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(path)
            .with_context(|| format!("stat {}", path.display()))?
            .permissions()
            .mode()
            & 0o777;
        if mode & 0o077 != 0 {
            return Err(anyhow!(
                "{} has insecure permissions {mode:o}; want owner-only",
                path.display()
            ));
        }
    }
    Ok(())
}

fn set_owner_only_file(_path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(_path, std::fs::Permissions::from_mode(0o600));
    }
}

fn set_owner_only_dir(_path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(_path, std::fs::Permissions::from_mode(0o700));
    }
}

fn print_metadata(m: &KeyMetadata) {
    let status = match m.status {
        KeyStatus::Active => "active",
        KeyStatus::ExpiringSoon => "expiring-soon",
        KeyStatus::Expired => "expired",
        KeyStatus::Stale => "stale",
        KeyStatus::Revoked => "revoked",
    };
    println!("  id:        {}", m.id);
    println!("  provider:  {}", m.provider.as_str());
    println!("  label:     {}", m.label);
    if let Some(t) = &m.project_tag {
        println!("  project:   {t}");
    }
    println!("  status:    {status}");
    println!("  created:   {}", m.created_at.to_rfc3339());
    if let Some(t) = m.last_used_at {
        println!("  last used: {}", t.to_rfc3339());
    }
    if let Some(id) = m.superseded_by {
        println!("  superseded by: {id}");
    }
}

fn print_audit_event(event: &AuditEvent) {
    let superseded_by = event
        .superseded_by
        .map(|id| id.to_string())
        .unwrap_or_else(|| "-".to_string());
    println!(
        "{}  {:<10}  {:<12}  {:<24}  {}  superseded_by={}",
        event.ts_utc,
        event.kind.as_str(),
        event.provider.as_deref().unwrap_or("-"),
        event.label.as_deref().unwrap_or("-"),
        event.entry_id,
        superseded_by
    );
}

fn salt_path(vault: &std::path::Path) -> PathBuf {
    let mut p = vault.to_path_buf();
    let new_name = format!(
        "{}.salt",
        p.file_name().and_then(|s| s.to_str()).unwrap_or("vault")
    );
    p.set_file_name(new_name);
    p
}
