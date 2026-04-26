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

## Up next (in order)

- [ ] T1.8 — Test harness CLI (apps/cli)
- [ ] T1.9 — CC review pass

## Operating rules

Read `OPERATING_NOTES.md` and `docs/framework/06_MILESTONE_1_TASKS.md`
before starting any task. Each task has an explicit acceptance check —
do not move on until it passes verifiably.

Henry runbook: `~/obsidian-vault/Nauta-AI/Projects/Holster/14_HENRY_RUNBOOK.md`
