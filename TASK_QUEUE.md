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
- [x] T2.12 — Runtime export V0 (`.env` / `.env.local` selected-key profile export)
      added as a sellable Holster differentiator. Dry-run preview redacts values,
      execution happens in Rust, selected keys only, target env files tracked by
      git are blocked, optional backup and `.gitignore` protection are supported,
      export audit log records names/paths only.
- [x] T2.12.1 — Runtime export V0 hardening pass (2026-04-30 evening).
      Dry-run preview no longer decrypts secrets at all (metadata-only path).
      `shell_quote_env_value` switched to single-quoted form (neutralizes
      `$VAR` / backtick / `$(...)` expansion in dotenv-style readers) and
      hard-rejects `\n`/`\r`/NUL bytes. Atomic write via temp + rename
      (`<filename>.holster-tmp`), with cleanup on failure; `*.holster-tmp`
      added to the gitignore block. `set_secret_file_perms` now propagates
      chmod errors instead of silently swallowing them; called via `?` at
      target / backup / audit-log sites. UI hint added to the export dialog
      reminding users that labels (not values) are recorded in the audit log.
      Real test export with fake keys at `~/Desktop/holster-export-test/`
      verified end-to-end.
- [x] T2.13 — Native Detector Pack V0 (2026-04-30 evening).
      Registry + scanner module at `apps/desktop/src-tauri/src/detectors.rs`.
      22 detectors across three tiers: Tier 1 (OpenAI, Anthropic, Google AI,
      Telegram, GitHub PAT classic + fine-grained + OAuth, Stripe live, Etsy
      OAuth pair, Cloudflare); Tier 2 (Replicate, HuggingFace, OpenRouter,
      ElevenLabs, Pinecone, Supabase JWT, Neon DB, MongoDB, AWS, GCP service
      account, Azure OpenAI); Tier 3 (Slack webhook + token, Discord webhook
      + bot, Notion, Airtable, Apify, JWT, PEM private key, high-entropy
      generic fallback). Pure in-memory `scan_text(&str) -> Vec<Detection>`
      — no file I/O, no external calls. Redaction contract: `redacted_preview`
      shows `<first-4>...<last-4>` for ≥12-char matches, `***` for shorter;
      tests assert the raw match never appears in any Detection field.
      32 unit tests pass with dummy keys only (every test input contains the
      literal `FAKE`). Free/Founder/Pro split decision: ALL detectors are in
      Free — paywalls go on workflow features (custom packs, scheduling, CI,
      team), never on detection. Roadmap doc at
      `obsidian-vault/Nauta-AI/Projects/Holster/2026-04-30-detector-pack-plan.md`
      details M3 repo-scanner wrapping.
- [x] T2.12.2 — Whitespace-bounded value guard (2026-04-30 evening).
      A real test export surfaced a fake OpenAI key that had been added with a
      leading space (`OPENAI_API_KEY=' sk-FAKE-...'`). Added
      `check_no_whitespace_bounds()` helper that hard-refuses values starting
      or ending with any whitespace (space, tab, newline, CR). Wired into
      `add_key` (blocks future bad entries) AND into `export_runtime_profile`
      Phase 2 (blocks legacy bad entries from reaching the env file). Holster
      never silently trims — error surfaces a clean message naming the
      offending key by display name (provider/label, never value) and asks
      the user to fix the value at its source and re-add. 9 unit tests added
      covering passing-clean cases + every common whitespace failure mode.

Verified locally (macOS arm64):

- `cargo test --workspace` → 50 passed, 0 failed (no M1 regressions)
- `cargo clippy --workspace --all-targets -- -D warnings` → clean
- `cargo fmt --all -- --check` → clean
- `pnpm --filter holster-desktop-ui build` → SvelteKit static build to `apps/desktop/build/`
- `pnpm exec tauri build --no-bundle` → arm64 release binary at `target/release/holster-desktop`
- Post T2.12 hardening re-check (2026-04-30): `cargo test --workspace` →
      79 passed (29 runtime-export tests + 50 vault tests);
      `cargo clippy --workspace --all-targets -- -D warnings` → clean;
      `cargo fmt --all -- --check` → clean;
      `pnpm --filter holster-desktop-ui build` → clean except pre-existing
      autofocus warnings in M2 screens;
      `pnpm exec tauri build --no-bundle` → release binary rebuilt at
      `target/release/holster-desktop`.
- Post T2.12.2 whitespace-guard re-check (2026-04-30 evening): see verification
      block below — runtime_export_tests grew from 29 to 38; all five gates green.
- Post T2.13 detector-pack re-check (2026-04-30 evening):
      `cargo test --workspace` → 120 passed total
      (32 detectors::tests + 38 runtime_export_tests + 50 vault crate),
      0 failed, 0 ignored;
      `cargo clippy --workspace --all-targets -- -D warnings` → clean;
      `cargo fmt --all -- --check` → clean.
      Adds `regex = "1"` to `apps/desktop/src-tauri/Cargo.toml`.

Items deferred to M3 (per spec): expiry/status logic, notifications, password
strength meter (zxcvbn), reveal-with-countdown button, settings panel for
clipboard TTL / idle timeout, named OpenClaw/Hermes/Codex templates beyond the
generic env-file runtime export.

## M3 — Detector Pack + Local Repo Scan UI — DONE + ACCEPTED

Signed off **2026-04-30 evening** after Dave's manual acceptance test
(`pnpm exec tauri dev` from `apps/desktop/`).

**Manual acceptance — Dave's 5-point test, all passing:**

1. Clean project scan → "No exposed secrets found" empty state rendered correctly.
2. Controlled fake-positive scan (FAKE-only inputs) → detections appeared
   as expected with correct provider / risk / file:line attribution.
3. No real secrets used at any point during testing.
4. Redaction behavior looked correct — only `<first-4>...<last-4>`
   previews surfaced; raw match never visible in UI.
5. UI flow was usable end-to-end: folder picker → scan → summary chips
   → filterable findings → close.



- [x] T2.13 — Native Detector Pack V0 (registry + scanner; see M2 block above)
- [x] T3.1 — Local Repo Scan UI (2026-04-30 evening).
      Wires the detector pack into a usable feature: pick a project folder
      via the Tauri folder picker, scan it, see redacted findings.
      - Backend: new `apps/desktop/src-tauri/src/repo_scanner.rs` with
        `scan_local_path(args) -> Result<ScanReport, String>`. Wraps
        `detectors::scan_text`. Refuses `/`, `$HOME`, `/etc`, `/var`,
        `/usr`, `/System`, `/Library`, `/private` as scan roots. Always
        skips `.git`, `node_modules`, `target`, `dist`, `build`, `.next`,
        `vendor`, `.venv`, `venv`, `__pycache__`, `.pytest_cache`,
        `.cache`, `.turbo`, `.pnpm-store`. Skips files >5 MB (configurable),
        binary files (NUL byte in first 8 KB), and non-UTF-8 files. Each
        Detection tagged with relative `file_path`, 1-based `line_number`,
        and `git_tracked` (computed once per scan via `git ls-files -z`).
      - Default: `respect_gitignore = false` so we DO find gitignored
        `.env` files — that's the leak surface. The `git_tracked` flag on
        each Detection tells the user whether the file is actually
        committed.
      - Tauri command: `scan_project_for_secrets(args)`. Does NOT require
        an unlocked vault — secret detection is a defensive audit, not a
        vault op.
      - Frontend: `apps/desktop/src/lib/views/ScanProjectDialog.svelte`
        with folder picker, scan button, summary chips by risk + by
        detector, filterable findings list (risk / provider / git-tracked),
        explicit "No exposed secrets found" empty state, redacted
        previews + recommended-action + rotation/docs links.
      - "Scan project" button added beside "Export runtime" on Main view.
      - Security contract: raw matched values never cross IPC. Test
        `serialized_report_never_contains_raw_match` builds a scan with
        4 unique-marker fake secrets, JSON-serializes the full
        `ScanReport`, and asserts none of the markers appear.
      - 11 new repo_scanner tests, all FAKE inputs.

Verified locally (macOS arm64, 2026-04-30 evening):

- `cargo test --workspace` → 133 passed total
  (50 vault + 38 runtime_export + 32 detectors + 11 repo_scanner +
  2 follow-on tests), 0 failed, 0 ignored.
- `cargo clippy --workspace --all-targets -- -D warnings` → clean
- `cargo fmt --all -- --check` → clean
- `pnpm --filter holster-desktop-ui build` → SvelteKit static build,
  no new warnings (existing autofocus warnings from M2 unchanged).
- `pnpm exec tauri build --no-bundle` → release binary rebuilt at
  `target/release/holster-desktop`.
- Adds `ignore = "0.4"` to `apps/desktop/src-tauri/Cargo.toml`.

## M3.1 — Project bootstrap helpers — DONE

Spec: `obsidian-vault/Nauta-AI/Projects/Holster/2026-04-30-m3.1-scope.md`.

Three concrete features that turn Holster from "vault + scanner" into
"project bring-up tool" without crossing any of Dave's hard lines
(no cloud sync, no auto-rotation, no scanning outside user-picked folders).

- [x] T3.1.1 — `.env.example` generator (2026-05-01).
      New backend module `apps/desktop/src-tauri/src/env_example.rs`
      with three Tauri commands: `env_example_from_vault` (vault-based
      mode, requires unlocked session for metadata only), `env_example_from_file`
      (read-only file-based mode, no vault required), `env_example_apply`
      (atomic write + audit log). Two source modes:
      (A) From vault — user picks vault keys; Holster derives env-var
      names via the existing `default_env_name` logic; optional
      Holster source comments reference provider/label, never values.
      (B) From existing `.env*` file — parser stops at the first `=`
      of each line and discards the value; refuses non-`.env*`
      basenames; refuses files >5 MB. Output filename validation
      via new `is_safe_env_example_filename` (accepts only
      `.env.example` and `<stem>.env.example` — rejects `.env`,
      `.env.local`, etc., to prevent accidental real-env overwrite).
      Refuses target paths inside skip dirs (.git, node_modules,
      etc.). Atomic write via `<filename>.holster-tmp` + rename.
      chmod 0644 (committable, not 0600). Audit log entry written
      with `kind: "env_example_generated"` to the existing
      `runtime-export-audit.jsonl` (names + path only, never values).
      New frontend `EnvExampleDialog.svelte` with mode tabs
      (vault / file), folder + file pickers, filename + header
      toggle, live preview pane showing exact body, two-step
      overwrite confirmation when the target exists. "Generate
      .env.example" button added to Main view beside "Review
      .gitignore safety". 41 unit tests including three CRITICAL
      serialization-leak guards proving no value substring leaks
      through proposal JSON, written body, or audit-log payload —
      verified with FAKEUNIQUEMARKER substrings that must never
      appear in any output.
- [x] T3.1.2 — Safe `.gitignore` helper (2026-05-01).
      New backend module `apps/desktop/src-tauri/src/gitignore_helper.rs`
      with `gitignore_audit` (read-only) and `gitignore_apply`
      (atomic, append-only) Tauri commands. Detects project type
      (node / python / rust / generic) via marker files. Curated
      catalogue of 7 rule sets: universal_env (locked on),
      holster, node, python, rust, macos_ide (default off),
      cloud_creds. Per-line dedupe trims whitespace and treats
      `.env` and `  .env  ` as equal. Apply re-validates rule-set
      membership at write time so a hostile frontend cannot sneak
      arbitrary lines in. Atomic write via `<filename>.holster-tmp`
      + rename, chmod 0644 (gitignore is committable). New
      frontend `GitignoreHelperDialog.svelte` with folder picker,
      auto-detection summary, per-set checkbox groups (with
      already-present markers), live diff preview, and explicit
      apply button. Re-audit after apply refreshes UI in place.
      "Review .gitignore safety" button added to Main view beside
      Scan project. 27 unit tests covering detection, append,
      idempotency, !.env.example dedupe, missing-trailing-newline
      handling, path safety, and audit-report-leak guard.
- [x] T3.1.3 — Agent runtime profiles (2026-05-01).
      Pure UX layer over the existing hardened `export_runtime_profile`
      command. New backend module
      `apps/desktop/src-tauri/src/agent_profiles.rs` ships a static
      catalogue of 5 profiles: Generic .env, OpenClaw, Claude Code,
      Codex (OpenAI CLI), and Hermes. New Tauri command
      `list_agent_profiles` exposes the catalogue (no secrets, names
      only). Frontend `ExportRuntimeDialog.svelte` gained a Profile
      dropdown at the top of the form that prefills the filename and
      profile-name fields when a profile is picked; user overrides win
      and stay sticky. Description, suggested env-var names, and
      optional TODO note (Hermes) render under the dropdown.
      14 unit tests including a no-real-key sanity guard that fails
      if any field in the catalogue contains a 25+ char alphanumeric
      run that could be mistaken for a real credential. Hermes
      profile is conservative per Dave's instruction — TODO note
      surfaces in UI rather than blocking the feature on Hermes
      perfection.

Verified locally (macOS arm64, 2026-05-01, M3.1 close):

- `cargo test --workspace` → 215 passed total
  (50 vault + 38 runtime_export + 32 detectors + 11 repo_scanner +
  27 gitignore_helper + 14 agent_profiles + 41 env_example +
  2 follow-on tests), 0 failed, 0 ignored.
- `cargo clippy --workspace --all-targets -- -D warnings` → clean
- `cargo fmt --all -- --check` → clean
- `pnpm --filter holster-desktop-ui build` → SvelteKit static build,
  no new warnings.
- `pnpm exec tauri build --no-bundle` → release binary rebuilt at
  `target/release/holster-desktop`.

**Confirmed:** no real secrets were read, displayed, exported,
rotated, or logged at any point during T3.1.1, T3.1.2, or T3.1.3
implementation. Three serialization-leak guard tests in T3.1.1
prove the from-file path never carries value substrings into the
proposal JSON, written body, or audit-log payload.

**M3.1 status: ALL 3 TASKS DONE.** Machine gates re-run clean on
2026-05-01; awaiting Dave's manual UI acceptance.

Machine verification refresh (2026-05-01):

- `cargo test --workspace` → 215 passed, 0 failed, 0 ignored.
- `cargo clippy --workspace --all-targets -- -D warnings` → clean.
- `cargo fmt --all -- --check` → clean.
- `pnpm --filter holster-desktop-ui build` → clean build; existing
  Svelte autofocus warnings remain.
- `pnpm exec tauri build --no-bundle` → release binary rebuilt at
  `target/release/holster-desktop`.

## M4 — Holster Auth V0 — LOCAL TOTP LANDING

Prompted by Dave's 2026-05-02 note that every new account setup now requires
2FA, and that the authenticator / backup-code sprawl is becoming the same
kind of operator pain Holster is meant to remove.

- [x] T4.1 — Local TOTP authenticator entries (2026-05-02).
      New backend module `apps/desktop/src-tauri/src/auth.rs` implements
      manual TOTP import from either a base32 secret or an `otpauth://totp/...`
      URI. Secrets and backup codes are serialized into a Holster Auth record
      and stored inside the existing encrypted vault as a reserved Generic key
      tagged with `__holster_auth_totp`. The frontend never receives the TOTP
      seed or backup codes after import; it only receives account metadata,
      backup-code count, and the current short-lived six-digit code on explicit
      user request.
- [x] T4.2 — Auth UI surface (2026-05-02).
      New `AuthDialog.svelte` plus Main view entry points: side rail "Auth"
      module and "Holster Auth" action button. UI lists stored authenticator
      accounts, shows issuer/account metadata and backup-code count, supports
      adding an account, and generates the current code for a selected account.
      The dialog explicitly states that manual secret entry is the current V0
      path and QR image scan comes next.

Security stance:

- No real authenticator seeds or production backup codes were used in tests.
- TOTP seed and backup codes do not cross IPC after the add operation.
- Backup-code values are never displayed in the Auth UI.
- Telegram code delivery was deliberately not added in V0. Future Telegram
  integration should default to "open Holster to approve/view" notifications,
  not sending the six-digit code over chat.
- QR-image import / camera scanning is deferred to the next slice.

Verified locally (macOS arm64, 2026-05-02):

- `cargo test --workspace` → 218 passed total
  (50 vault + 38 runtime_export + 32 detectors + 11 repo_scanner +
  27 gitignore_helper + 14 agent_profiles + 41 env_example +
  3 auth + 2 follow-on tests), 0 failed, 0 ignored.
- `cargo clippy --workspace --all-targets -- -D warnings` → clean.
- `cargo fmt --all -- --check` → clean.
- `pnpm --filter holster-desktop-ui build` → clean build; existing
  Svelte autofocus warnings remain in older dialogs.
- `pnpm exec tauri build --no-bundle` from `apps/desktop/` → release
  binary rebuilt at `target/release/holster-desktop`.

## Up next

- M2 manual acceptance by Dave (`pnpm exec tauri dev` from `apps/desktop/`).
  *(M3 already accepted 2026-04-30 — this is the older M2 acceptance still
  outstanding from the M2 sign-off block above.)*
- M3.1 and M4 Auth manual click acceptance by Dave (`pnpm exec tauri dev`
  from `apps/desktop/`), then founder cohort / landing-page work.
- M4.1 — QR-image import for authenticator setup.
- M4.2 — safe Telegram notification design: "approval needed / open Holster",
  not chat-delivered 2FA codes by default.
- M5 — custom detector packs (Founder tier) + repo scheduling + CLI for CI gates.
- Lifecycle features (expiry dates, status colors, notifications) per
  `docs/framework/05_MILESTONES.md` § M3 — re-scope into a future
  milestone since M3 is now closed on detector + scanner work.

## Operating rules

Read `OPERATING_NOTES.md` and `docs/framework/06_MILESTONE_1_TASKS.md`
before starting any task. Each task has an explicit acceptance check —
do not move on until it passes verifiably.

Henry runbook: `~/obsidian-vault/Nauta-AI/Projects/Holster/14_HENRY_RUNBOOK.md`
