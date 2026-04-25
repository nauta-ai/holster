# 02 — Architecture

## Stack (locked)

| Layer | Choice | Why |
|---|---|---|
| App shell | Tauri 2.0 | Native menu bar, small binary, Rust security primitives |
| Backend | Rust | Memory-safe crypto, mature ecosystem |
| Frontend | React 18 + TypeScript | Fast iteration on UI |
| Styling | Tailwind CSS | Speed |
| State | Zustand | Lightweight, no Redux overhead |
| Database | SQLite via `rusqlite` with `bundled-sqlcipher` | Encrypted at rest |
| Crypto | `argon2` + `aes-gcm` + `rand` | Audited, standard |
| Bundler | Vite | Tauri default |
| Package manager | pnpm | Faster than npm |
| CLI | Separate Rust binary, shares `vault` crate | Reuses encryption logic |

## Pinned dependencies (Cargo.toml backend)

```toml
[dependencies]
tauri = { version = "2.0", features = ["macos-private-api"] }
tauri-plugin-clipboard-manager = "2.0"
tauri-plugin-notification = "2.0"
tauri-plugin-fs = "2.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1", features = ["full"] }
rusqlite = { version = "0.31", features = ["bundled-sqlcipher"] }
argon2 = "0.5"
aes-gcm = "0.10"
rand = "0.8"
zeroize = { version = "1.7", features = ["derive"] }
secrecy = "0.8"
chrono = { version = "0.4", features = ["serde"] }
keyring = "3.0"
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }
thiserror = "1.0"
anyhow = "1.0"
tracing = "0.1"
tracing-subscriber = "0.3"
```

**Note:** Versions are minimums; Alex pins exact patch versions at scaffold time and commits `Cargo.lock`.

## Vault encryption model

Two layers of defense:

### Layer 1: SQLCipher whole-database encryption

The SQLite file itself is encrypted with SQLCipher using a key derived from the user's master password. Without the master password, the file is unreadable garbage on disk.

### Layer 2: Per-key AES-256-GCM encryption

Each individual key value is encrypted *again* before being written to its row. Each key gets its own 12-byte nonce generated from a CSPRNG.

**Why both?** Defense in depth. If SQLCipher is ever compromised at the implementation level, the per-key encryption still holds. If the per-key implementation has a bug, the file-level encryption still holds.

### Master password flow

```
User enters master password
  → Argon2id(password, salt, m=64MB, t=3, p=4) = 32-byte key
  → Split: first 16 bytes = SQLCipher key, last 16 bytes (HKDF-extended to 32) = per-key encryption key
  → Open SQLCipher DB with key
  → Hold per-key AES key in memory inside `secrecy::Secret<>` wrapper
  → Auto-lock after configurable idle timeout (default 15 min)
  → On lock: zeroize all in-memory key material, close DB
```

**Salt:** Stored in a metadata table. 16 bytes, generated once at vault creation.

**Argon2 parameters:** m=64MB, t=3, p=4. These are 2024 OWASP recommendations for interactive use. Tuneable via a constant; do not let users configure.

### What is stored in the macOS Keychain

Optional: a "remember me on this device" token. If user opts in, we store a wrap key in the system Keychain protected by the user's macOS login. This lets the app unlock automatically after macOS login without re-prompting for the master password every launch. **Off by default.** When enabled, the keychain entry holds the wrap key, not the master password itself.

## IPC contract (Rust ↔ React)

All key-touching commands require a valid session token. Session tokens are issued at unlock and expire on lock or timeout.

### Commands (Rust → exposed to frontend)

```rust
// Unlock / lock
unlock_vault(password: String) -> Result<SessionToken, VaultError>
lock_vault(token: SessionToken) -> Result<(), VaultError>
is_unlocked() -> bool

// Key CRUD
list_keys(token: SessionToken) -> Result<Vec<KeyMetadata>, VaultError>  // metadata only, no values
get_key_value(token: SessionToken, key_id: Uuid) -> Result<SecretString, VaultError>
add_key(token: SessionToken, input: AddKeyInput) -> Result<KeyMetadata, VaultError>
update_key(token: SessionToken, key_id: Uuid, input: UpdateKeyInput) -> Result<KeyMetadata, VaultError>
delete_key(token: SessionToken, key_id: Uuid) -> Result<(), VaultError>

// Clipboard
copy_key_to_clipboard(token: SessionToken, key_id: Uuid, ttl_secs: u32) -> Result<(), VaultError>

// Vault management
create_vault(password: String) -> Result<(), VaultError>
change_master_password(token: SessionToken, old: String, new: String) -> Result<(), VaultError>
```

### KeyMetadata (no value, safe to render)

```rust
struct KeyMetadata {
    id: Uuid,
    provider: Provider,        // enum: Anthropic, OpenAI, Google, ...
    label: String,             // user-defined, e.g. "nauta-books-anthropic"
    project_tag: Option<String>,
    created_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
    last_rotated_at: Option<DateTime<Utc>>,
    last_used_at: Option<DateTime<Utc>>,
    status: KeyStatus,         // Active, ExpiringSoon, Expired, Stale, Revoked
    notes: Option<String>,
    key_format_valid: bool,    // pattern check for provider's expected format
}
```

## Database schema

```sql
CREATE TABLE vault_meta (
    schema_version INTEGER NOT NULL,
    salt BLOB NOT NULL,
    created_at TEXT NOT NULL,
    auto_lock_minutes INTEGER NOT NULL DEFAULT 15
);

CREATE TABLE keys (
    id TEXT PRIMARY KEY,                 -- UUID v4
    provider TEXT NOT NULL,
    label TEXT NOT NULL,
    project_tag TEXT,
    key_ciphertext BLOB NOT NULL,        -- AES-256-GCM ciphertext
    key_nonce BLOB NOT NULL,             -- 12 bytes
    created_at TEXT NOT NULL,
    expires_at TEXT,
    last_rotated_at TEXT,
    last_used_at TEXT,
    notes TEXT,
    revoked INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_keys_provider ON keys(provider);
CREATE INDEX idx_keys_project_tag ON keys(project_tag);
CREATE INDEX idx_keys_expires_at ON keys(expires_at);

CREATE TABLE usage_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key_id TEXT NOT NULL,
    fetched_at TEXT NOT NULL,
    period_start TEXT NOT NULL,
    period_end TEXT NOT NULL,
    input_tokens INTEGER,
    output_tokens INTEGER,
    cost_usd_cents INTEGER,
    raw_response_json TEXT,
    FOREIGN KEY (key_id) REFERENCES keys(id) ON DELETE CASCADE
);

CREATE INDEX idx_usage_key ON usage_snapshots(key_id, fetched_at);

CREATE TABLE leak_scan_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scanned_at TEXT NOT NULL,
    repo_path TEXT NOT NULL,
    matches_found INTEGER NOT NULL,
    matches_json TEXT NOT NULL    -- structured findings (file, line, masked match)
);
```

## File system layout

```
~/Library/Application Support/Holster/
├── vault.db                  # SQLCipher-encrypted SQLite
├── config.json               # non-secret prefs (theme, hotkeys, idle timeout)
├── logs/
│   └── holster.log           # rolling, no key values ever logged
└── backups/
    └── vault-YYYYMMDD.db     # automatic backups, also encrypted
```

## Logging policy

- Use `tracing` crate
- **Never log** decrypted key values, master passwords, session tokens, ciphertext, nonces, or salts
- DO log: command names, key UUIDs, provider, success/failure, timing
- Log rotation: 10MB max per file, keep 5 files
- A `tracing` filter strips any field named `secret`, `password`, `key_value`, `token` defensively

## Auto-lock and clipboard policy

- **Idle auto-lock:** 15 minutes default, user-configurable 1-60 min
- **Lock on macOS sleep:** always
- **Lock on display lock:** always
- **Clipboard auto-clear:** 30s default, user-configurable 5-300s, capped at 300s
- **Clipboard write:** uses `tauri-plugin-clipboard-manager`; we track our own write timestamp and overwrite with empty string at TTL
- **Concealed display:** UI never renders full key values by default; shows masked `sk-ant-…XXXX` (last 4 chars). Reveal requires explicit click and triggers a 5s countdown before re-masking.
