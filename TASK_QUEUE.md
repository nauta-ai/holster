# Task queue — what's next

## Done

- [x] T1.0 — Repo scaffolding
- [x] T1.1 — `holster-vault` crate skeleton (Cargo.toml + lib.rs + module stubs)
- [x] T1.2 — Error types (thiserror enum, no plaintext leakage)
- [x] T1.3 — Crypto (Argon2id + AES-256-GCM, 6 tests passing, security-critical)
- [x] T1.4 — Models (Provider, KeyStatus, KeyMetadata, AddKeyInput w/ redacted Debug, 7 tests)
- [x] T1.5 — DB module (SQLCipher schema + parameterized CRUD, 11 tests passing)
- [x] T1.6 — Session module (UUID newtype tokens, idle timeout, 12 tests)
- [x] T1.7 — Vault facade (create/open/unlock/lock + add/list/get/delete, 12 tests including full lifecycle)
- [x] T1.8 — CLI test harness (apps/cli — create/add/list/get/delete subcommands)
- [x] T1.9 — CC review pass (PASS WITH FIXES; see `docs/reviews/m1_security_review.md`; V-1 + V-4 landed 2026-04-26)

## M1 — DONE

Signed off **2026-04-26** after security review (`docs/reviews/m1_security_review.md`)
returned **PASS WITH FIXES** and the two recommended fixes landed:

- [x] V-1 (MEDIUM) — Re-unlock now validates the master password against a
      fresh SQLCipher connection before issuing a session token. Wrong-password
      re-unlock returns `BadPassword` instead of silently issuing a token whose
      AES key is wrong. Regression test
      `vault::tests::unlock_wrong_password_fails_after_prior_lock`.
- [x] V-4 (LOW) — Vault DB file is explicitly chmod'd to 0600 in
      `Vault::create`, mirroring the existing salt-sidecar treatment.
      Regression test `vault::tests::create_sets_vault_file_mode_0600`.

Verified at sign-off (macOS ARM64, Dave's laptop):

- `cargo test --workspace` → 50 passed, 0 failed, 0 ignored
  (was 48 pre-fix; V-1 adds `unlock_wrong_password_fails_after_prior_lock`,
  V-4 adds `create_sets_vault_file_mode_0600` under `cfg(unix)`).
- `cargo clippy --workspace --all-targets -- -D warnings` → clean
- `cargo fmt --all -- --check` → clean

## M2 — Desktop app shell + unlock UX (shipped, awaiting Dave's manual acceptance)

Spec: `docs/framework/05_MILESTONES.md` § M2.

- [x] T2.0 — Tauri 2 backend scaffold (`apps/desktop/src-tauri/{Cargo.toml,build.rs,tauri.conf.json,capabilities/default.json,icons/}`)
- [x] T2.1 — IPC commands wrapping `Vault` (`vault_status`, `create_vault`,
      `unlock_vault`, `lock_vault`, `list_keys`, `add_key`, `delete_key`,
      `copy_to_clipboard`). Session token never crosses the IPC boundary.
- [x] T2.2 — Sanitized error mapping (`VaultError` → user-facing strings)
- [x] T2.3 — SvelteKit (Svelte 5) frontend with static adapter (`pnpm --filter holster-desktop-ui build`)
- [x] T2.4 — First-run wizard (creates vault at `~/Library/Application Support/com.nautaai.holster/vault.db`)
- [x] T2.5 — Unlock screen (clean error on wrong password — no stack trace)
- [x] T2.6 — Key list view (provider | label | project | created | last_used; no plaintext)
- [x] T2.7 — Add-key dialog (provider dropdown, masked key input)
- [x] T2.8 — Delete-key confirmation modal
- [x] T2.9 — Copy-to-clipboard with 30-second auto-clear (clipboard write happens in Rust, plaintext never leaves the IPC boundary by JS)
- [x] T2.10 — Auto-lock observation (UI polls `list_keys` every 60s; on `SessionExpired` from the crate, bounces back to unlock screen)
- [x] T2.11 — README at `apps/desktop/README.md`

Verified locally (macOS arm64):

- `cargo test --workspace` → 50 passed, 0 failed (no M1 regressions)
- `cargo clippy --workspace --all-targets -- -D warnings` → clean
- `cargo fmt --all -- --check` → clean
- `pnpm --filter holster-desktop-ui build` → SvelteKit static build to `apps/desktop/build/`
- `pnpm exec tauri build --no-bundle` → arm64 release binary at `target/release/holster-desktop`

Items deferred to M3 (per spec): expiry/status logic, notifications, password
strength meter (zxcvbn), reveal-with-countdown button, settings panel for
clipboard TTL / idle timeout.

## Up next

- M2 manual acceptance by Dave (`pnpm exec tauri dev` from `apps/desktop/`).
- M3 — lifecycle (expiry dates, status colors, notifications) per `docs/framework/05_MILESTONES.md` § M3.

## Operating rules

Read `OPERATING_NOTES.md` and `docs/framework/06_MILESTONE_1_TASKS.md`
before starting any task. Each task has an explicit acceptance check —
do not move on until it passes verifiably.

Henry runbook: `~/obsidian-vault/Nauta-AI/Projects/Holster/14_HENRY_RUNBOOK.md`
