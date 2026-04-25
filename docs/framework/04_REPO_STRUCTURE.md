# 04 — Repo Structure

## Top-level layout

```
~/holster/
├── README.md
├── LICENSE                          # Source-available, commercial license
├── .gitignore
├── .editorconfig
├── pnpm-workspace.yaml              # Yes, monorepo for app + cli
├── package.json
├── apps/
│   ├── desktop/                     # Tauri menu bar app
│   │   ├── src-tauri/               # Rust backend
│   │   │   ├── Cargo.toml
│   │   │   ├── tauri.conf.json
│   │   │   ├── build.rs
│   │   │   ├── icons/
│   │   │   └── src/
│   │   │       ├── main.rs                    # Tauri entry, command registry
│   │   │       ├── error.rs                   # VaultError, IpcError types
│   │   │       ├── session.rs                 # SessionToken, idle timer
│   │   │       ├── ipc/
│   │   │       │   ├── mod.rs
│   │   │       │   ├── vault_commands.rs      # unlock, lock, list, etc.
│   │   │       │   ├── key_commands.rs        # add, update, delete, copy
│   │   │       │   └── usage_commands.rs      # poll_usage, get_spend
│   │   │       ├── vault/
│   │   │       │   ├── mod.rs                 # Vault struct, public API
│   │   │       │   ├── crypto.rs              # Argon2 + AES-GCM
│   │   │       │   ├── db.rs                  # SQLCipher connection mgmt
│   │   │       │   ├── schema.rs              # migrations
│   │   │       │   └── models.rs              # KeyMetadata, KeyRecord
│   │   │       ├── providers/
│   │   │       │   ├── mod.rs                 # Provider enum + trait
│   │   │       │   ├── anthropic.rs           # key format, usage endpoint
│   │   │       │   ├── openai.rs
│   │   │       │   ├── google.rs
│   │   │       │   ├── replicate.rs
│   │   │       │   ├── elevenlabs.rs
│   │   │       │   └── generic.rs             # fallback for unknown providers
│   │   │       ├── leak_scan/
│   │   │       │   ├── mod.rs
│   │   │       │   ├── patterns.rs            # regex per provider
│   │   │       │   └── scanner.rs             # walk + grep
│   │   │       ├── menu_bar.rs                # macOS menu bar setup
│   │   │       ├── notifications.rs           # expiry, leak alerts
│   │   │       ├── clipboard.rs               # write + auto-clear
│   │   │       └── logging.rs                 # tracing setup with redaction
│   │   ├── src/                     # React frontend
│   │   │   ├── main.tsx
│   │   │   ├── App.tsx
│   │   │   ├── components/
│   │   │   │   ├── KeyList.tsx
│   │   │   │   ├── KeyRow.tsx
│   │   │   │   ├── AddKeyModal.tsx
│   │   │   │   ├── UnlockScreen.tsx
│   │   │   │   ├── CreateVaultScreen.tsx
│   │   │   │   ├── SettingsPanel.tsx
│   │   │   │   ├── SpendDashboard.tsx
│   │   │   │   ├── LeakScanModal.tsx
│   │   │   │   └── StatusDot.tsx
│   │   │   ├── lib/
│   │   │   │   ├── ipc.ts           # typed wrappers around invoke()
│   │   │   │   ├── session.ts       # token mgmt
│   │   │   │   └── format.ts        # mask keys, format dates
│   │   │   ├── hooks/
│   │   │   │   ├── useVault.ts
│   │   │   │   ├── useKeys.ts
│   │   │   │   └── useIdleTimer.ts
│   │   │   ├── state/
│   │   │   │   └── store.ts         # Zustand
│   │   │   └── styles/
│   │   │       └── globals.css
│   │   ├── index.html
│   │   ├── vite.config.ts
│   │   ├── tsconfig.json
│   │   ├── tailwind.config.js
│   │   └── package.json
│   └── cli/                         # holster CLI binary
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           └── commands/
│               ├── use.rs           # holster use <provider>
│               ├── list.rs
│               ├── unlock.rs
│               └── scan.rs
├── crates/
│   └── holster-vault/               # Shared vault crate (used by app + cli)
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── crypto.rs
│           ├── db.rs
│           └── models.rs
├── docs/
│   ├── ARCHITECTURE.md              # Copy of framework doc
│   ├── THREAT_MODEL.md
│   ├── CONTRIBUTING.md
│   └── SECURITY.md                  # Disclosure policy
├── tests/
│   ├── integration/
│   │   ├── vault_lifecycle.rs
│   │   ├── encryption_roundtrip.rs
│   │   └── leak_scanner.rs
│   └── fixtures/
│       └── sample_repo/             # for leak scanner tests
└── scripts/
    ├── dev.sh                       # pnpm tauri dev
    ├── build.sh                     # release build + sign
    ├── notarize.sh                  # macOS notarization
    └── ci-check.sh                  # cargo fmt, clippy, test, audit
```

## Why a monorepo with shared `holster-vault` crate

The CLI (`holster use anthropic`) and the desktop app must produce *bit-for-bit identical* encryption results. If a key written by the desktop app can't be read by the CLI, the product is broken. Sharing the crypto/vault crate enforces this at the type level.

## Branch strategy

- `main` — protected, always shippable
- `dev` — integration branch
- `milestone/M1-vault-foundation` — milestone branches
- `feat/<short-name>` — task-level branches off milestone branch

CC reviews PRs from milestone branch → `dev`. Dave approves merge to `main`.

## Commit policy

- Conventional commits: `feat:`, `fix:`, `chore:`, `sec:` (security), `docs:`, `test:`
- `sec:` commits get extra CC scrutiny
- No commits with key values in test fixtures (use clearly fake values like `sk-ant-test-1111111111111111111111111111111111111111`)
- Pre-commit hook runs the leak scanner on staged files (dogfooding)

## .gitignore essentials

```
# Build artifacts
target/
dist/
node_modules/
*.app
*.dmg

# Local vault (NEVER commit a real vault)
*.db
*.db-journal
*.db-shm
*.db-wal

# Env / secrets
.env
.env.local
*.key
*.pem

# OS
.DS_Store
Thumbs.db

# IDE
.vscode/
.idea/
*.swp

# Logs
*.log
logs/
```
