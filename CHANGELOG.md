# Changelog

All notable changes to Holster + Buildbelt are tracked here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project does not follow strict SemVer yet — it ships in milestones
(see `TASK_QUEUE.md`).

## [0.7.0] — 2026-05-24 — Master rotation + scriptable password sources

### Added — `rotate-master` CLI subcommand

- `holster-cli rotate-master <vault>` re-encrypts every entry under a new
  master password and regenerates the vault salt — atomic under a single
  exclusive SQLite transaction. If any step fails, the vault remains intact
  under the old master.
- `--keychain-update SERVICE,ACCOUNT` (macOS) updates the cached Keychain
  password entry in the same command, so daemons that read the master via
  `exec-env --password-keychain-*` keep working without manual re-entry.
- Audit log records a `master_rotated` event with the number of entries
  re-encrypted.
- `Vault::rotate_master(&self, old_pw, new_pw) -> Result<usize>` exposed on
  the `holster-vault` crate API for programmatic use (Tauri desktop UI, etc).
- 7 new unit tests in `crates/holster-vault/src/vault.rs` covering happy
  path, wrong-old-password rejection, weak-new-password rejection, audit
  event creation, salt regeneration, empty-vault edge case, and
  consecutive-rotation safety.

### Added — scriptable password sources on `get`, `add`, `rotate-master`

- `--password-env <ENV_NAME>` — read master from an environment variable.
- `--password-stdin` — read first line of stdin (or, for `rotate-master`,
  `--old-password-stdin` + `--new-password-stdin` read OLD then NEW).
- `--password-keychain-service <SVC>` + `--password-keychain-account <ACCT>`
  — read from macOS Keychain (previously available only on `exec-env`).
- Precedence: env → stdin → keychain → interactive prompt fallback.
- Unlocks unattended rotation flows for CI, launchd, systemd, and any
  scripted use that doesn't have an interactive TTY.

### Added — Windows + macOS Intel release binaries

- `.github/workflows/release.yml` matrix expanded with `x86_64-apple-darwin`
  (macOS-13 runner) and `x86_64-pc-windows-msvc` (windows-2022 runner)
  alongside the existing macOS-arm64 + Linux-x86_64 targets.
- Windows release ships as `.zip` containing `holster-cli.exe` (Unix
  targets continue to ship as `.tar.gz`).

### Why

The 2026-05-24 credential-leak rotation across the Nauta fleet exposed a
hard product gap: v0.1.0–v0.6.0 had NO way to rotate the vault master
password. Operators had to manually `get` every entry, create a new vault,
and `add` each entry back under the new master. That's unacceptable for
anything marketed as a local-first API key manager. v0.7.0 closes the gap.

### Migration note for existing v0.1.0–v0.6.0 users

Your existing vault is fully forward-compatible. Upgrade to v0.7.0, then
run `holster-cli rotate-master <vault>` whenever you need to change the
master. No re-creation of the vault required.

## [Unreleased]

### Added — Holster MCP Preflight V0 (M5, 2026-05-14)
- New module `apps/desktop/src-tauri/src/mcp_preflight.rs` —
  `analyze_mcp_config(json)` returns separate run and share verdicts
  (Safe / Caution / Risky) for an MCP server config entry.
- 10 deterministic checks across stdio + http transports: wrapper drift
  (`npx -y` unpinned), HTTP transport on remote URL, implicit env
  inheritance, sensitive env var references (OWASP MCP01), shell exec
  wrappers, and more.
- Pure in-memory JSON analyzer — no network, no AI loop, no file I/O.
- 14 named tests cover every check + verdict rollup + JSON round-trip.

### Added — Buildbelt UX landing (M2.1, 2026-05-14)
- Buildbelt setup rail with guided journey landmarks (level chooser,
  guided signup, starter prompts, post-purchase guide).
- DESIGN.md design contract applied to Main view, Buildbelt setup
  dialog, and Holster Doctor dialog: warm off-white surfaces, amber
  primary actions, green safety signals.
- Holster brand mark assets at all macOS-required resolutions (icns
  bundle + 32/128/128@2x PNGs + SvelteKit `static/holster-mark.png`).
- `apps/desktop/src/lib/scanHistory.ts` persists the last N repo scans
  to localStorage for verdict-delta comparison.

### Added — Detector pack v0.2 (2026-05-14)
- Expanded patterns in `detectors.rs` reflecting M3.1 field-testing.
- Detection contract unchanged — `redacted_preview` never carries the
  raw match.

### Added — Repo scanner v0.2 (2026-05-14)
- `is_test_path()` recognizes common test-directory conventions so
  fixtures don't pollute the real-finding verdict.
- `is_self_reference_path()` recognizes Holster's own AEO docs and
  `detectors.rs` source where pattern strings appear as documentation.
- `build_risk_summary()` aggregates Detection counts by risk level
  for the Doctor dashboard headline.
- Six new fixture-classification tests.

### Added — CLI runtime-secrets path (2026-05-14)
- `holster-cli import` / `import-batch`: scan a launchd plist or
  `.env` file, classify variables by secret-likelihood, and import
  qualifying entries into the vault. Source files untouched.
- `holster-cli exec-env`: read an exec manifest, retrieve named keys
  from the vault, populate the child process environment, and exec the
  agent command. Master password sourced from macOS Keychain.
  Audit log records metadata only.
- Hardens audit-log + manifest path permissions to 0600 / 0700.
- Powers the runtime-secrets pattern Rosie / Amelia / Henry / Atticus
  already depend on.

### Changed
- `.gitignore` extended to cover the full agent-automation overflow set
  (`overnight-research/`, `visual-handoff/`, `automation_outputs/`,
  `marketing-*-staging/`, etc.) plus `*.bak*` patterns. Repo
  `git status` is now clean by default.

## [2026-05-02] — Holster Auth V0 (M4)

### Added
- Local TOTP authenticator entries stored in the encrypted vault.
- AuthDialog UI for adding accounts (manual base32 secret or
  `otpauth://` URI) and generating current six-digit codes.
- Secrets and backup-code values never cross IPC after import.

## [2026-05-01] — Project bootstrap helpers (M3.1)

### Added
- `.env.example` generator (vault + from-file modes).
- Safe `.gitignore` audit + atomic append-only apply.
- Agent runtime profile catalogue (Generic / OpenClaw / Claude Code /
  Codex / Hermes).

## [2026-04-30] — Detector Pack + Local Repo Scan (M3)

### Added
- Native Detector Pack V0 (22 detectors across three tiers covering
  OpenAI, Anthropic, Google AI, Telegram, GitHub PAT variants, Stripe,
  Etsy OAuth, Cloudflare, Replicate, HuggingFace, OpenRouter,
  ElevenLabs, Pinecone, Supabase, Neon, MongoDB, AWS, GCP, Azure
  OpenAI, Slack, Discord, Notion, Airtable, Apify, JWTs, PEM private
  keys).
- Local Repo Scan UI wrapping `detectors::scan_text` with directory
  walk + `.gitignore` respect.
- Runtime export V0 hardening: metadata-only dry-run, atomic write,
  whitespace-bounded value guard, shell-quote env values.

## [2026-04-26] — Vault foundation (M1)

### Added
- `holster-vault` Rust crate with SQLCipher + Argon2id + AES-256-GCM.
- CLI test harness (create / add / list / get / delete subcommands).
- Security review passed; V-1 (re-unlock validates against fresh
  SQLCipher connection) and V-4 (vault DB file chmod 0600) fixes
  landed.
- 50 tests passing.

---

Earlier history: see git log on `milestone/M1-vault-foundation` and
`milestone/M2-desktop-shell` branches.
