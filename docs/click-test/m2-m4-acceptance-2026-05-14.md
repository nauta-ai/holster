# M2 / M3.1 / M4 Click-Test Acceptance Script

**Date:** 2026-05-14
**Branch:** `milestone/M2-desktop-shell`
**Time:** ~15 minutes
**Audience:** Dave (sign-off)

## How to run

```bash
cd ~/holster/apps/desktop
pnpm exec tauri dev
```

Wait for the Tauri window to open. The first run may compile for ~30s.

## 1. M2 Acceptance — vault unlock + key management (5 min)

### 1.1 First-run wizard
- [ ] On launch (if no vault exists), wizard prompts for master password
- [ ] Pick a memorable test password (we'll lock + re-unlock to verify)
- [ ] Confirm-password screen rejects mismatched entries
- [ ] On accept, vault created at `~/Library/Application Support/com.nautaai.holster/vault.db`
- [ ] Main view opens (empty key list)

### 1.2 Add a key
- [ ] Click "Add Key" → dialog opens with provider dropdown
- [ ] Pick "OpenAI", enter label `test-import-1`, enter fake key `sk-FAKE-TEST-KEY-DO-NOT-USE-1234567890`
- [ ] Save → returns to key list, the row appears with provider/label/created/last_used
- [ ] **Key value NOT shown anywhere** in the UI (only metadata)

### 1.3 Copy + auto-clear
- [ ] Click "Copy" on the row
- [ ] Paste somewhere (Notes, Terminal) within 5 sec — the fake key value pastes
- [ ] Wait 30 sec, paste again — clipboard is empty (auto-clear fired)

### 1.4 Wrong password
- [ ] Use the menu / sidebar to "Lock Vault"
- [ ] Re-unlock with a deliberately wrong password
- [ ] Error message is clean (no Rust stack trace; no crash)
- [ ] Re-unlock with correct password → key list returns

### 1.5 Delete key
- [ ] Click delete on the test row → confirmation modal appears
- [ ] Confirm → key removed, list updates

**M2 PASS criteria:** all five sub-tests pass without crashing.

---

## 2. M3.1 Acceptance — repo scan + bootstrap helpers (4 min)

### 2.1 Scan a real repo
- [ ] Open Holster Doctor (sidebar entry or main view)
- [ ] Pick `~/holster` (this repo) as the scan target
- [ ] Run scan → completes within ~5 sec
- [ ] Verdict appears with risk-level breakdown (Critical/High/Medium/Low counts)
- [ ] Findings list shows redacted previews (first-4...last-4 format) — no full keys visible

### 2.2 Fixture classification (this is the v0.2 work)
- [ ] Some findings should be classified as "Test fixture" (separate panel from real findings)
- [ ] Test-path findings: paths matching `tests/`, `__tests__/`, `*_test.rs`
- [ ] Self-reference findings: AEO `.md` docs in repo root (`2026-05-*-holster-*.md`),
      `detectors.rs` source where pattern strings appear

### 2.3 .env.example generator
- [ ] Tools → "Generate .env.example"
- [ ] Pick a key from vault → preview shows redacted entry
- [ ] Apply → `.env.example` file written; values are placeholders only

### 2.4 .gitignore helper
- [ ] Tools → ".gitignore audit"
- [ ] Tool inspects current `.gitignore`, suggests safe additions for known credential file patterns
- [ ] Apply → atomic append (no destructive rewrites of existing content)

### 2.5 Agent runtime profile
- [ ] Tools → "Agent runtime profile"
- [ ] Pick one of: Generic / OpenClaw / Claude Code / Codex / Hermes
- [ ] Profile preview matches expected behavior (per `docs/framework/06_MILESTONE_1_TASKS.md`)

**M3.1 PASS criteria:** all five sub-tests pass; redaction never breaks.

---

## 3. M4 Acceptance — Local TOTP authenticator (3 min)

### 3.1 Add TOTP account
- [ ] Open Auth dialog
- [ ] Pick "Add account" → enter dummy account `test@example.com`, issuer `TestIssuer`
- [ ] Enter base32 secret `JBSWY3DPEHPK3PXP` (this is the canonical RFC 6238 fake test secret)
- [ ] Save → account appears in list with issuer/account/backup-code-count metadata

### 3.2 Generate current code
- [ ] Click "Generate code" on the row
- [ ] Six-digit code appears (changes every 30 sec)
- [ ] Code matches what an external authenticator would generate for `JBSWY3DPEHPK3PXP`

### 3.3 Backup-code privacy
- [ ] Backup-code count is visible, but **values are not** (per spec)
- [ ] Confirm via Inspect (Tauri devtools) that no backup-code value ever crosses IPC

### 3.4 Telegram code not delivered
- [ ] Confirm the V0 dialog does NOT offer a "send to Telegram" option (deferred to M4.2)

**M4 PASS criteria:** add + generate + privacy checks all pass.

---

## 4. M2.1 — Buildbelt UX (2 min sanity)

This is new in 2026-05-14's commit batch; not part of the legacy sign-off but
worth eyeing the design.

- [ ] Buildbelt setup rail visible on first launch with the journey landmarks
- [ ] Design feels Apple-utility-adjacent (warm off-white, amber primary,
      green safety signals) — not a loud SaaS dashboard
- [ ] No motion that feels "marketing-y" (spinners + slide-ins should be restrained)

If anything feels off, flag in the sign-off doc and we'll route to Jordan.

---

## Sign-off

After all four sections PASS:

```
M2:    PASS / FAIL  ____
M3.1:  PASS / FAIL  ____
M4:    PASS / FAIL  ____
M2.1:  PASS / FAIL  ____  (sanity only — feedback welcome)

Dave signature: ___________________________
Date:           ___________________________
```

Then I'll mark the milestones DONE in TASK_QUEUE.md and we move to T5.1.

## If something fails

- Don't restart from scratch — flag the exact step + screenshot
- I'll diagnose and patch (each module has its own test surface, so we can
  fix in isolation without rolling back the M2.1 commit chain)
- Worst case: `git revert` the offending commit + leave a `TODO_M2_FOLLOWUP`
  note in TASK_QUEUE
