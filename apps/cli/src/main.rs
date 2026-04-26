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

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use secrecy::ExposeSecret;
use uuid::Uuid;

use holster_vault::{
    AddKeyInput, KeyMetadata, KeyStatus, Provider, Vault,
};

#[derive(Parser)]
#[command(name = "holster", about = "Holster vault CLI — local-first API key manager", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Initialize a new vault at PATH. Prompts for password (entered twice).
    Create {
        path: PathBuf,
    },
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
    },
    /// List metadata for all keys (no plaintext shown).
    List {
        path: PathBuf,
    },
    /// Decrypt and print a key value.
    Get {
        path: PathBuf,
        id: Uuid,
    },
    /// Delete a key by id.
    Delete {
        path: PathBuf,
        id: Uuid,
    },
}

#[derive(ValueEnum, Clone, Copy)]
enum ProviderArg {
    Anthropic,
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
        Command::Add { path, provider, label, project, notes } =>
            cmd_add(&path, provider.into(), label, project, notes),
        Command::List { path } => cmd_list(&path),
        Command::Get { path, id } => cmd_get(&path, id),
        Command::Delete { path, id } => cmd_delete(&path, id),
    }
}

fn cmd_create(path: &std::path::Path) -> Result<()> {
    let pw = rpassword::prompt_password("New master password: ")
        .context("reading password")?;
    let confirm = rpassword::prompt_password("Confirm: ")
        .context("reading confirmation")?;
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
) -> Result<()> {
    let vault = Vault::open(path).context("opening vault")?;
    let pw = rpassword::prompt_password("Master password: ")?;
    let token = vault.unlock(&pw).context("unlock failed (wrong password?)")?;

    let key_value = rpassword::prompt_password("Key value: ")
        .context("reading key value")?;
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
    let pw = rpassword::prompt_password("Master password: ")?;
    let token = vault.unlock(&pw).context("unlock failed (wrong password?)")?;
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

fn cmd_get(path: &std::path::Path, id: Uuid) -> Result<()> {
    let vault = Vault::open(path)?;
    let pw = rpassword::prompt_password("Master password: ")?;
    let token = vault.unlock(&pw).context("unlock failed (wrong password?)")?;
    let secret = vault.get_key_value(token, id).context("getting key value")?;
    println!("{}", secret.expose_secret());
    vault.lock(token).ok();
    Ok(())
}

fn cmd_delete(path: &std::path::Path, id: Uuid) -> Result<()> {
    let vault = Vault::open(path)?;
    let pw = rpassword::prompt_password("Master password: ")?;
    let token = vault.unlock(&pw).context("unlock failed (wrong password?)")?;
    vault.delete_key(token, id).context("deleting key")?;
    println!("✓ deleted {id}");
    vault.lock(token).ok();
    Ok(())
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
}

fn salt_path(vault: &std::path::Path) -> PathBuf {
    let mut p = vault.to_path_buf();
    let new_name = format!("{}.salt", p.file_name().and_then(|s| s.to_str()).unwrap_or("vault"));
    p.set_file_name(new_name);
    p
}
