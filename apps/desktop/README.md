# Holster Desktop (M2)

Tauri 2 desktop app wrapping the `holster-vault` crate. macOS-only for now.

## Architecture

```
apps/desktop/
├── src/                    SvelteKit frontend (TypeScript + Svelte 5)
│   ├── lib/api.ts          Typed wrappers over Tauri commands
│   ├── lib/views/          FirstRun / Unlock / Main / AddKeyDialog / ConfirmDelete
│   ├── lib/styles.css      Shared styles
│   └── routes/+page.svelte Dispatch by VaultStatus -> view
├── src-tauri/              Rust backend (Tauri 2 + holster-vault)
│   ├── src/lib.rs          AppState + 8 IPC commands
│   ├── tauri.conf.json     Window + bundle config
│   └── capabilities/       Permission allowlist
└── build/                  Vite output, served by Tauri
```

## Security boundary

The frontend is treated as untrusted UI:

- **Session tokens never cross the IPC boundary.** They live in
  `Mutex<Option<SessionToken>>` inside the Rust `AppState`. The frontend just
  knows whether the vault is unlocked or not.
- **Plaintext key material never crosses the IPC boundary.** The only way to
  extract a key value is `copy_to_clipboard`, which decrypts inside Rust,
  writes to the OS clipboard, and schedules a 30-second auto-clear.
- **Master passwords are transient.** They arrive as a Tauri command argument,
  are passed straight to `Vault::create` / `Vault::unlock`, and the binding is
  dropped when the command returns. They are never persisted or logged.
- **Errors are sanitized.** `VaultError` is mapped to short user-facing strings
  in `err_to_string` — no cause chains or paths in production messages.

## Build / run

Prerequisites: Node 20+, pnpm 10+, Rust stable, Xcode CLT.

```sh
# from repo root
pnpm install

# dev (hot reload, Svelte HMR + Cargo watch)
cd apps/desktop
pnpm exec tauri dev

# release build (no bundle)
pnpm exec tauri build --no-bundle
# binary: target/release/holster-desktop

# release build with .app/.dmg
pnpm exec tauri build
# bundle: target/release/bundle/macos/Holster.app
```

## Vault location

By default the vault is created at:

```
~/Library/Application Support/com.nautaai.holster/vault.db
```

Both the database and its salt sidecar are chmod 0600.

## Features (M2 spec)

| # | Feature                  | Status |
|---|--------------------------|--------|
| 1 | Unlock screen            | ✅     |
| 2 | Key list view            | ✅     |
| 3 | Copy to clipboard (30s)  | ✅     |
| 4 | Add key dialog           | ✅     |
| 5 | Delete key (confirm)     | ✅     |
| 6 | Auto-lock on idle (15m)  | ✅ (UI polls and re-prompts on `SessionExpired`) |
| 7 | First-run wizard         | ✅     |

## Tauri commands

All commands return `Result<T, String>`; errors are pre-sanitized strings.

| Command              | Arguments                      | Notes                         |
|----------------------|--------------------------------|-------------------------------|
| `vault_status`       | —                              | `no_vault` / `locked` / `unlocked` |
| `create_vault`       | `password`                     | First-run; minimum 8 chars    |
| `unlock_vault`       | `password`                     | Wrong pwd → `BadPassword`     |
| `lock_vault`         | —                              | Idempotent                    |
| `list_keys`          | —                              | Metadata only, no plaintext   |
| `add_key`            | `args: AddKeyArgs`             |                                |
| `delete_key`         | `id`                           |                                |
| `copy_to_clipboard`  | `id`                           | Returns auto-clear delay (s)  |

## Cross-compat with the CLI

Both `apps/cli` and `apps/desktop` link against the same `holster-vault` crate
with the same SQLCipher schema. A vault created by either app is readable by
the other (modulo path).
