# 05 — Milestones

8 milestones, each with explicit acceptance criteria. CC reviews against these criteria before Dave approves merge.

---

## M1 — Vault Foundation (security-critical)

**Goal:** Encrypted SQLite vault with master password, working CRUD via test harness. No UI yet.

**Deliverables:**
- `holster-vault` crate compiles and passes all unit tests
- Argon2id KDF with locked parameters (m=64MB, t=3, p=4)
- AES-256-GCM per-key encryption with unique nonces
- SQLCipher whole-DB encryption
- Schema migrations runnable via test harness
- `Vault` API: `create`, `unlock`, `lock`, `add_key`, `get_key`, `list_keys`, `update_key`, `delete_key`, `change_password`
- Session token system with idle timeout
- All zeroization on lock verified by tests

**Acceptance criteria:**
1. ✅ Unit tests cover: encryption roundtrip, wrong password fails, idle timeout locks, session token validation, password change re-encrypts all keys
2. ✅ `cargo audit` clean (no known CVEs)
3. ✅ `cargo clippy -- -D warnings` clean
4. ✅ No `unwrap()`, `expect()`, or `panic!()` in any function that touches plaintext key material (CC verifies via grep)
5. ✅ `Debug` impl on key-bearing structs redacts values (test verifies)
6. ✅ Memory dump test: after `lock()`, plaintext key material not findable in process heap (best-effort with `zeroize`)
7. ✅ Two test vault files: one created by CLI test harness, one by desktop test harness — both readable by both (cross-compat invariant)

**CC focus areas:**
- Crypto parameter correctness
- Nonce uniqueness (each key gets fresh nonce, never reused)
- Error handling (no panics on malformed input)
- Memory hygiene (zeroize, secrecy crate usage)

---

## M2 — Menu Bar App Shell + Unlock UX

**Goal:** Tauri menu bar app launches, prompts for master password (or vault creation on first run), shows a list of keys with masked values.

**Deliverables:**
- `pnpm tauri dev` launches a working menu bar app
- Click menu bar icon → popup window
- Vault creation flow with password strength meter (zxcvbn ≥ 3 required)
- Unlock screen with master password input
- Key list view: provider logo, label, masked value (`sk-ant-…3f9a`), status dot
- Reveal button per key (5s countdown, re-masks)
- "Copy" button per key (uses clipboard with auto-clear)
- Lock button + auto-lock on idle
- Settings panel: idle timeout, clipboard TTL

**Acceptance criteria:**
1. ✅ App launches in <2s on M1/M2 Mac
2. ✅ Menu bar icon visible and themed (light + dark mode)
3. ✅ First-launch creates vault; subsequent launches go straight to unlock
4. ✅ Password strength meter blocks weak passwords
5. ✅ Idle timer locks vault after configured timeout (test with 1-min override)
6. ✅ Clipboard auto-clears at TTL
7. ✅ No raw key values appear in React DevTools or console
8. ✅ `pnpm tauri build` produces a signable `.app` bundle

**CC focus areas:**
- IPC contract compliance (no key values flowing without session token)
- Clipboard handling (we own the TTL; verify timer fires)
- Frontend never persists key values to local storage / sessionStorage

---

## M3 — Lifecycle (dates, status, notifications)

**Goal:** Keys have expiry dates, status indicators, and the user gets notified before expiry.

**Deliverables:**
- Add/edit modal includes `expires_at` field with date picker
- Status logic: `Active` (>14d to expiry), `ExpiringSoon` (≤14d), `Expired` (past expiry), `Stale` (no `last_used_at` in 90d), `Revoked`
- Status dots color-coded (green / yellow / red / gray / black)
- Background task: daily check for expiring keys
- macOS notification 14, 7, 1 day before expiry
- "Last used" timestamp updates on copy/inject
- Sortable list: by status, expiry, last used, provider

**Acceptance criteria:**
1. ✅ Status transitions verified via unit tests with mocked `now()`
2. ✅ Notifications fire via `tauri-plugin-notification`
3. ✅ User can dismiss / snooze notifications
4. ✅ "Stale" state correctly identifies keys not used in 90 days
5. ✅ Edit history not retained (privacy — old expiry dates not logged)

---

## M4 — Tags, Search, Leak Scanner

**Goal:** Keys can be tagged by project, searched fast, and the leak scanner can find exposed keys in local repos.

**Deliverables:**
- `project_tag` field on keys (free-text, autocomplete from existing tags)
- Search: filter by provider, tag, status, label substring
- Leak scanner UI: pick a directory, scan, show findings
- Scanner uses provider-specific regex patterns
- Findings show file path, line number, masked match, recommendation (rotate or remove)
- Scanner respects `.gitignore` and skips `node_modules`, `target`, `.venv`
- Scan history persisted

**Acceptance criteria:**
1. ✅ Scanner correctly identifies keys for: Anthropic (`sk-ant-`), OpenAI (`sk-` 51 chars), Google AI (`AIza`), Replicate (`r8_`), ElevenLabs (`xi-api-key:` patterns)
2. ✅ Test fixture repo with planted fake keys → scanner finds all of them, zero false positives on a benign repo
3. ✅ Scanner does not read files >5MB
4. ✅ Scanner does not follow symlinks outside the chosen root
5. ✅ Findings file paths are sanitized (no path traversal in display)

**CC focus areas:**
- Regex correctness (provider patterns must match real key formats — verify against current 2026 formats via web check)
- Path sanitization
- Resource limits (scanner can't OOM on a huge repo)

---

## M5 — Usage Tracking (Anthropic + OpenAI)

**Goal:** For Anthropic and OpenAI keys, Holster polls the provider's usage endpoint hourly and shows monthly spend per key and per project tag.

**Deliverables:**
- Provider trait with `fetch_usage(api_key) -> UsageSnapshot`
- Anthropic implementation: `/v1/organizations/usage_report/messages` (admin key required — handle gracefully if user provided non-admin key)
- OpenAI implementation: `/v1/usage` endpoint
- Background poller: hourly, respects rate limits, exponential backoff on errors
- Spend dashboard: this month total, per provider, per project tag, per key
- Recharts-based visualizations
- "Stale data" indicator if last fetch >2h old
- User can disable tracking per key

**Acceptance criteria:**
1. ✅ Polling does not block UI
2. ✅ Errors are user-visible but don't crash the app
3. ✅ Rate limit responses respected (no spam)
4. ✅ Spend numbers match provider dashboard within 5% (manual verification by Dave with his real keys)
5. ✅ Disabling a key stops polling immediately
6. ✅ No usage data is sent to any third party — all polling is provider → Holster direct

**CC focus areas:**
- Verify endpoint URLs and request formats are current (2026)
- TLS configuration (rustls, modern cipher suites only)
- Cost calculation accuracy

---

## M6 — CLI Companion

**Goal:** `holster use anthropic` injects the Anthropic key as an env var into the current shell session without writing it to disk.

**Deliverables:**
- `holster` binary installed via `cargo install` or homebrew formula
- `holster unlock` — prompts for master password, holds session in keychain or short-lived token file (XDG runtime dir, mode 0600, deleted on lock)
- `holster list` — shows keys (no values)
- `holster use <provider>` — outputs `export ANTHROPIC_API_KEY=...` for `eval $(holster use anthropic)` pattern
- `holster scan <path>` — runs leak scanner from CLI
- Shell completions for bash, zsh, fish

**Acceptance criteria:**
1. ✅ CLI and desktop app share `holster-vault` crate (vault written by one is readable by other)
2. ✅ `holster use` output is shell-safe (proper quoting, no injection)
3. ✅ Session token file is mode 0600 and in user-only directory
4. ✅ `holster lock` clears session immediately
5. ✅ Errors go to stderr, success output to stdout (so `eval` works cleanly)

---

## M7 — Pro Tier Scaffolding (license + sync stub)

**Goal:** License key validation gates Pro features. Encrypted iCloud sync stub (sync logic deferred; just the file-watcher and conflict resolution skeleton).

**Deliverables:**
- License key entry in settings
- Online validation against a Cloudflare Worker license endpoint (deferred design — for now, accept any key matching pattern `HSTR-XXXX-XXXX-XXXX-XXXX`)
- Pro features gated: usage tracking, CLI, advanced search, sync
- iCloud Documents folder watch: writes encrypted vault to `~/Library/Mobile Documents/.../Holster/`
- Conflict detection (last-write-wins for v1, with a backup of the loser)

**Acceptance criteria:**
1. ✅ Free tier fully usable without license
2. ✅ Pro features clearly marked in UI when not licensed
3. ✅ Sync writes only encrypted file (no plaintext ever leaves the machine)
4. ✅ Conflict creates `vault-conflict-YYYYMMDD-HHMMSS.db` backup
5. ✅ License revocation handled gracefully (downgrades to free tier, keeps vault intact)

---

## M8 — Polish, Sign, Notarize, Land

**Goal:** Shippable v1.0.

**Deliverables:**
- Apple Developer ID code signing
- Notarization workflow scripted
- DMG installer with drag-to-Applications UX
- Auto-update via `tauri-plugin-updater` (signed updates)
- Landing page at holster.dev (separate sub-task)
- Privacy policy + security disclosure page
- Onboarding tutorial (3 screens on first launch)
- Telemetry: opt-in, anonymous, aggregated (just "vault unlocked" / "key added" counts, no content)

**Acceptance criteria:**
1. ✅ Signed `.dmg` passes Gatekeeper without warnings
2. ✅ Notarization ticket stapled
3. ✅ Auto-update tested with two consecutive versions
4. ✅ Landing page live with download link
5. ✅ Telemetry verifiable as zero-content (Dave audits payload)
6. ✅ Dave has been using v0.9 daily for 2 weeks before v1.0 ships

---

## Dependency graph

```
M1 (vault foundation)
  ↓
  ├──→ M2 (menu bar UI) ──→ M3 (lifecycle) ──→ M4 (tags + scanner)
  └──→ M6 (CLI)                                      ↓
                                                 M5 (usage)
                                                     ↓
                                                 M7 (pro tier)
                                                     ↓
                                                 M8 (ship)
```

M2 and M6 can be parallel after M1 is done. M3, M4 sequential on the UI track.
