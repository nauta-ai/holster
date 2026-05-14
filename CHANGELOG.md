# Changelog

All notable changes to Holster + Buildbelt are tracked here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project does not follow strict SemVer yet — it ships in milestones
(see `TASK_QUEUE.md`).

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
