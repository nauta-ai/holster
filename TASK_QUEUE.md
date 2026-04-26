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

## Up next (in order)

(M2 backlog lives in `docs/framework/06_MILESTONE_1_TASKS.md` follow-on docs.)

## Operating rules

Read `OPERATING_NOTES.md` and `docs/framework/06_MILESTONE_1_TASKS.md`
before starting any task. Each task has an explicit acceptance check —
do not move on until it passes verifiably.

Henry runbook: `~/obsidian-vault/Nauta-AI/Projects/Holster/14_HENRY_RUNBOOK.md`
