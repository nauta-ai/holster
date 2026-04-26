# Holster M1 Security Review

**Reviewer:** Claude Code
**Date:** 2026-04-26
**Scope:** M1 deliverables — `holster-vault` crate (crypto, db, session, vault, models, error) + `holster-cli` test harness
**Rubric:** `docs/framework/07_SECURITY_REVIEW_CHECKLIST.md`

---

## Summary

**Verdict: PASS WITH FIXES** — Approve for personal-use M1 with one MEDIUM bug recommended (not strictly required) and a couple of LOW polish items. The crypto core is solid: locked Argon2id parameters meeting OWASP, AES-256-GCM with fresh `OsRng` nonces per call, SQLCipher with hex-encoded `PRAGMA key`, 100% parameterized SQL, hand-rolled `Debug` redacting all key material, correct `Secret`/`Zeroize` usage on plaintext and AES keys, no `unsafe` blocks, no `panic!`/`dbg!`/library-side `println!`, no `unwrap`/`expect` outside `#[cfg(test)]`. The full test suite (48 tests) passes; clippy with `-D warnings` is clean; `cargo fmt` is clean; `cargo audit` reports zero advisories across 168 dependencies. The most material finding is a logic flaw in `Vault::unlock`'s re-unlock branch (MEDIUM) where wrong-password is not detected on a second unlock once the connection is already open — the consequence is a misleading session token rather than a confidentiality breach (decryption still fails on use), but it should be fixed before this is the only signal users rely on. None of the universal-section checks in the rubric is failing in a blocking way.

---

## Per-file findings

### `crates/holster-vault/src/crypto.rs`

| # | Severity | Finding |
|---|---|---|
| C-1 | INFO | Argon2id parameters match the locked checklist exactly: 64 MB / 3 iters / 4 lanes / Argon2id / V0x13 / 64-byte output split 32+32. |
| C-2 | INFO | `generate_salt` and `generate_nonce` both use `OsRng.fill_bytes` — correct CSPRNG. Nonce is fresh per `encrypt_key_value` call (line 106) — no nonce reuse. |
| C-3 | INFO | Argon2 64-byte output buffer is zeroized after splitting (line 89). The split halves are wrapped in `Secret<[u8;32]>` immediately. |
| C-4 | INFO | `encrypt_key_value` returns `(Vec<u8>, [u8; NONCE_LEN])` — nonce stored alongside ciphertext for recovery; matches checklist. |
| C-5 | INFO | Negative-path tests present: `wrong_password_fails_decrypt`, `nonces_are_unique`, `different_salt_different_key`. |
| C-6 | LOW | `encrypt_key_value` / `decrypt_key_value` map AES errors into `VaultError::Crypto(format!("aes decrypt: {e}"))`. The underlying `aes_gcm` error is intentionally opaque (it does not include plaintext or key bytes), so this is safe — but worth a `#[allow]` comment so future maintainers don't add detail that *could* leak via timing or substring. Not a real leak today. |
| C-7 | INFO | Constant-time concerns: AES-GCM authentication tag verification is constant-time inside `aes_gcm`. No HMAC or token comparison happens in this module. |
| C-8 | INFO | No `unsafe`. No `unwrap`/`expect` outside the `#[cfg(test)]` module (which is gated `#[allow(clippy::unwrap_used, clippy::expect_used)]`). |

### `crates/holster-vault/src/db.rs`

| # | Severity | Finding |
|---|---|---|
| D-1 | INFO | All CRUD uses `params![]` / `?N` placeholders — 100% parameterized. The lone exception is `PRAGMA key = "x'<hex>'"` which is structurally non-parameterizable in SQLite; the input is a fixed-length 64-char hex string derived from a 32-byte fixed-length array, so there is no injection vector. Comment on lines 9-12 makes this explicit. |
| D-2 | INFO | `parameterized_query_resists_injection_in_label` test confirms behavior with `'); DROP TABLE keys; --` payload. Good. |
| D-3 | INFO | `KeyRecord` has hand-rolled `Debug` that redacts ciphertext + nonce (lines 93-104). No `#[derive(Debug)]` on the type. Test `select_all_metadata_lists_all_inserted` asserts no ciphertext bytes in metadata Debug. |
| D-4 | INFO | `Connection` is wrapped in `Mutex<Connection>`; lock acquired briefly per operation. No reentrancy; no held-lock-across-await (no async). |
| D-5 | LOW | Mutex poisoning maps to `VaultError::Crypto("db mutex poisoned ...")` — semantically misleading (it isn't a crypto error). A dedicated `VaultError::Internal(String)` variant would be cleaner and is non-blocking. |
| D-6 | INFO | `PRAGMA foreign_keys = ON` is set after PRAGMA key — correct order, foreign-key cascades work for `usage_snapshots`. |
| D-7 | INFO | Salt bootstrap inserts all-zero salt at migration time and is overwritten by `set_salt` during `Vault::create`; the gap is internal-only since no external code path can call CRUD without the salt being set first. |
| D-8 | INFO | No `cipher_compatibility = 4` PRAGMA is set explicitly. The bundled SQLCipher (rusqlite `bundled-sqlcipher` 0.31) defaults to SQLCipher 4 format, which matches the checklist intent. Worth verifying via a one-line manual `sqlite3 vault.db .schema` smoke test (should fail without the key) — flagged as LOW polish below. |

### `crates/holster-vault/src/session.rs`

| # | Severity | Finding |
|---|---|---|
| S-1 | INFO | `SessionToken` is a UUIDv4 newtype generated via `Uuid::new_v4()` (uuid 1.10 with `v4` feature uses `getrandom` → OS CSPRNG). Tokens are session identifiers, not secrets, so storing them by value in `HashMap` keys is fine; `Debug` intentionally exposes the UUID for logging. |
| S-2 | INFO | `SessionState` has hand-rolled `Debug` that redacts `aes_key` (lines 87-96). Verified by `session_state_debug_redacts_aes_key` test. |
| S-3 | INFO | AES key is `Secret<[u8; 32]>`. `aes_key()` clones the underlying bytes into a fresh `Secret` so both the in-store copy and the returned copy zeroize on drop. |
| S-4 | INFO | `validate` and `touch` both check `is_idle_expired` and remove the entry on expiry, which drops `SessionState` and zeroizes its `Secret`. Good. |
| S-5 | INFO | `revoke` is idempotent — removes if present. `Drop` of `SessionState` triggers `Secret::drop` → `Zeroize`. |
| S-6 | INFO | Token comparison happens via `HashMap` lookup (UUID equality). Tokens are not crypto material, and the timing of "is this token in my map" is not a meaningful side channel here (UUID space is 122 bits — guessing is infeasible regardless of timing). No constant-time compare needed. |
| S-7 | INFO | Single `Mutex<HashMap<...>>` — all session operations take the lock; no TOCTOU between "validate" and "use" because `aes_key()` itself does its own validation atomically inside the same lock acquisition. Race concerns: minimal. |
| S-8 | INFO | Negative-path tests: `validate_rejects_unknown_token`, `validate_rejects_expired_session`, `touch_rejects_expired_and_removes`, `aes_key_rejects_expired_session`, `revoke_drops_secret_zeroize_path_smoke`. Coverage is thorough. |
| S-9 | LOW | Clock-skew handling in `is_idle_expired`: a backwards jump returns `false` (treats as not-expired). For a personal-use M1 this is fine; in a paranoid setting one could prefer to expire on negative elapsed. Non-blocking. |

### `crates/holster-vault/src/vault.rs`

| # | Severity | Finding |
|---|---|---|
| V-1 | **MEDIUM** | **Re-unlock branch can accept a wrong password.** In `unlock()` (lines 124-136), when `db_slot` is already `Some(db)` (e.g., user calls `unlock` a second time after a previous unlock+lock), the code does NOT verify the new password against SQLCipher. It only re-reads the salt from the already-open connection and compares it to the sidecar — but the sidecar salt never changes, so this comparison is always trivially true. Result: a wrong-password second unlock returns `Ok(token)` and creates a session whose AES key is derived from the wrong password. Subsequent `get_key_value` calls will fail with an AES-GCM auth-tag error (so confidentiality is preserved), but `BadPassword` is never surfaced and the user gets misleading errors at usage time. **Fix sketch:** in the re-unlock branch, attempt to open a *new* `Database` with `keys.sqlcipher_key` and run a sentinel query; on success, replace `db_slot`; on failure return `BadPassword`. Or simpler: always close-and-reopen on `unlock()`, accepting one extra Argon2 + open per re-unlock (typical interactive cost is acceptable). The current `lock()` keeps the SQLCipher connection alive specifically to avoid this re-derivation, so the simple fix changes behavior — pick whichever tradeoff matches your operational story. |
| V-2 | INFO | Hand-rolled `Debug` on `Vault` (lines 49-59). Surfaces only path, locked/unlocked flag, session count. No internal state leaks. |
| V-3 | INFO | Salt sidecar written with `0o600` on Unix (lines 246-250). Best-effort failure is logged-and-continued, which is appropriate (filesystem perms can fail on weird mounts). |
| V-4 | LOW | The vault DB file itself is created by SQLite/SQLCipher and inherits the user's umask. The checklist requires `0600` for the vault file. Add an explicit `set_permissions(0o600)` after `Database::open` succeeds during `Vault::create`, mirroring the sidecar code path. Minor — on a default macOS umask of `0022` the file is `0644`, world-readable but only as ciphertext, so confidentiality holds; integrity/availability concerns remain. |
| V-5 | LOW | `MIN_PASSWORD_LEN = 8` enforced by length only, not by zxcvbn score (the checklist asks for "zxcvbn score ≥ 3"). The `WeakPassword` variant exists, just not driven by zxcvbn. Acceptable for personal-use M1; tracked-for-M2 in backlog should be enough. |
| V-6 | INFO | `get_key_value` calls `aes_key_for(token)` which routes through `SessionStore::aes_key` which validates session+expiry atomically. Then encrypts/decrypts; then best-effort `update_last_used` and `touch` — both ignored on error so a stale session can't block the read. No session-validation bypass. |
| V-7 | INFO | `add_key` / `delete_key` / `list_keys` all start with session validation. Verified by `add_without_session_fails`, `lock_invalidates_token` tests. |
| V-8 | INFO | `unlock` does not log the password or the derived key. `BadPassword` and `WeakPassword` errors carry no plaintext. |
| V-9 | INFO | Salt sidecar pathing handles the typical case correctly (`vault.db` → `vault.db.salt`). The `unwrap_or("vault")` on a non-UTF-8 path name is defensive but practically unreachable on macOS. |
| V-10 | INFO | Full lifecycle test (`full_lifecycle_create_unlock_add_get_lock_unlock_get`) covers create → unlock → add → get → lock → re-unlock → get. Demonstrates persistence + token rotation. |

### `crates/holster-vault/src/models.rs`

| # | Severity | Finding |
|---|---|---|
| M-1 | INFO | `AddKeyInput` has hand-rolled `Debug` redacting `key_value` (lines 121-132). Verified by `add_key_input_debug_redacts_value`. Comment explicitly forbids `#[derive(Debug)]`. |
| M-2 | INFO | `KeyMetadata` derives `Debug` and `Serialize` and contains no plaintext field — verified by `key_metadata_serializes_without_plaintext` and the `list_returns_metadata_without_plaintext` integration test. |
| M-3 | INFO | `Provider` and `KeyStatus` serialize as snake_case with explicit overrides for `OpenAI`/`ElevenLabs` to keep them lowercase canonical. Roundtrip tested. |
| M-4 | LOW | `AddKeyInput::key_value` is a plain `String` (heap-allocated, not zeroized on drop). When the CLI builds an `AddKeyInput`, the plaintext lives on the heap until the struct drops. For M1 personal-use this is acceptable (the lifetime is short — encrypt-and-discard within `Vault::add_key`). For hardening, wrap in `Secret<String>` or `secrecy::SecretString`. Tracked for M2. |

### `crates/holster-vault/src/error.rs`

| # | Severity | Finding |
|---|---|---|
| E-1 | INFO | No variant carries plaintext key material. `KeyNotFound(Uuid)` carries only the ID (non-secret). `Crypto(String)` and `Migration(String)` carry constructed strings — call sites verified to format only counts/lengths/library errors, never `Secret` contents. |
| E-2 | INFO | `Db(rusqlite::Error)` and `Io(std::io::Error)` use `#[from]` — these wrapped errors do not contain key material in normal paths. (Theoretical: a malformed query string could echo back, but no query string is constructed from secret data in this codebase.) |
| E-3 | INFO | Distinct variants for `BadPassword`, `WeakPassword`, `VaultNotFound`, `VaultAlreadyExists`, `InvalidSession`, `SessionExpired`. **Note re. enumeration:** the checklist warns that distinguishing "wrong password" from "vault not found" enables enumeration. In a local single-user vault on the user's own filesystem this is *not* a meaningful threat — the attacker would already need filesystem read access to even probe. Flagging here for completeness, not as a finding. |
| E-4 | INFO | All variants implement `Display` via `thiserror`; all `Display` strings are static or wrap a non-secret message. No internal state leakage. |

### `apps/cli/src/main.rs`

| # | Severity | Finding |
|---|---|---|
| L-1 | INFO | Master password and key value both read via `rpassword::prompt_password` — terminal echo disabled, no shell history capture (input is not on argv). Good. |
| L-2 | INFO | `cmd_get` prints the decrypted secret to stdout via `println!("{}", secret.expose_secret())`. This is the documented behavior of a CLI `get` subcommand and is appropriate for a test harness. Users redirecting stdout to a file will store plaintext; calling it via shell substitution may capture into shell history depending on the shell's settings. Not a code bug; documentation in the subcommand help could note "redirect at your own risk." Not blocking. |
| L-3 | INFO | Errors print via `eprintln!` and only show `anyhow` chain context — no plaintext or password material is included. The `.context("unlock failed (wrong password?)")` wraps the error message generically. |
| L-4 | INFO | `vault.lock(token).ok()` at end of each subcommand revokes the session before exit. Drops `Secret` → zeroize. Good hygiene for a CLI process even though the OS reclaims pages on exit. |
| L-5 | LOW | `cmd_create` confirms password match by `pw != confirm` — this is `String` PartialEq, not constant-time. For password-confirmation against the user's own input, timing leakage is irrelevant (the attacker is the user). Non-issue. Mentioned only because the rubric asks for constant-time comparisons "where appropriate" — this is one of the places it's *not* appropriate. |
| L-6 | INFO | No `unsafe`, no `panic!`, no `dbg!`. All `unwrap`/`expect` are in cargo-managed init paths (clap parser). |

---

## Cargo test results

```
cargo test --workspace
→ 48 passed, 0 failed, 0 ignored (holster-vault)
→ 0 tests in holster-cli, holster-desktop (expected for M1 — desktop is a stub)
```

Coverage highlights:
- Crypto: roundtrip, wrong-password decrypt failure, nonce uniqueness, salt determinism + uniqueness.
- DB: open/migrate, salt set/get, insert + select, missing-id error, list, update last-used, delete, duplicate-id error, **SQL-injection resistance test**.
- Session: create/validate/touch/revoke, expired-session removal, debug redaction, zeroize-path smoke test.
- Vault: short-password reject, existing-path reject, missing-path reject, wrong-password reject, add+get roundtrip, list-without-plaintext, delete, lock invalidates token, add-without-session fails, full lifecycle.

Negative-path coverage: present and adequate for M1.

## Clippy results

```
cargo clippy --workspace --all-targets -- -D warnings
→ Finished `dev` profile (no warnings)
```

Library code has `#![warn(clippy::unwrap_used, clippy::expect_used, clippy::panic)]`; clippy is clean against this stricter set.

## Cargo fmt

```
cargo fmt --all -- --check
→ clean (no diff)
```

## Cargo audit

```
cargo audit
→ 1058 advisories loaded; 168 crates scanned; exit 0 (no vulnerabilities)
```

---

## Recommended next actions

Blocking (none).

Non-blocking — recommended fixes before storing real production secrets:

1. **(MEDIUM, V-1)** Fix the re-unlock wrong-password path in `Vault::unlock`. Either close + reopen the SQLCipher connection on every unlock (simplest, ~Argon2 cost), or attempt a sentinel `Database::open` with the freshly-derived `sqlcipher_key` and only swap the slot if it succeeds. Add a unit test: `unlock_wrong_password_fails_after_prior_lock`.

2. **(LOW, V-4)** Explicitly `chmod 0600` the vault DB file in `Vault::create` after `Database::open`, mirroring the sidecar treatment.

3. **(LOW, M-4)** Migrate `AddKeyInput::key_value` to `secrecy::SecretString` so the plaintext zeroizes when the input struct drops — defense in depth.

4. **(LOW, V-5)** Wire zxcvbn into `Vault::create` so `WeakPassword` is driven by the same scoring the checklist mandates. Backlog item is fine.

5. **(LOW, D-5)** Consider adding `VaultError::Internal(String)` for poisoned-mutex and other infrastructure errors so they don't masquerade as `Crypto`.

6. **(LOW, D-8)** Add a manual smoke test (or an integration test that shells out) confirming the vault file is unreadable via plain `sqlite3` CLI without SQLCipher — checks the bundled-sqlcipher build is what's actually wired in.

Polish — track for M2:

7. Wire `tracing` subscriber into the CLI and the eventual desktop app, with a redaction layer that strips fields named `password`, `secret`, `key_value`, `token`, `master_key`, `aes_key`, `sqlcipher_key`. Currently no `tracing::*` calls exist in the codebase, so the redaction layer can be set up before the first call site lands.

---

## Sign-off

M1 deliverables meet the security bar for a personal-use vault storing API keys for the Nauta factory. The crypto core (Argon2id + AES-256-GCM + SQLCipher), session management, and Debug/error hygiene are well-implemented and well-tested. One MEDIUM logic flaw in the re-unlock path should be patched before this becomes the only line of defense, but it does not breach confidentiality — only the wrong-password UX. All universal checks in `07_SECURITY_REVIEW_CHECKLIST.md` pass; no BLOCKERs found.

**Approved for M1 sign-off, conditional on tracking V-1 and V-4 in the M2 backlog.**

— Claude Code, 2026-04-26

---

## Resolution (2026-04-26)

Both recommended fixes from this review landed in M1 the same day, ahead of
the M2 cycle. M1 is now signed off without outstanding security debt from
this audit.

### V-1 (MEDIUM) — wrong-password re-unlock — **FIXED**

Commit: `sec: V-1 — detect wrong password on re-unlock when db slot is populated`
(`crates/holster-vault/src/vault.rs`).

`Vault::unlock` now opens a *fresh* `Database` with the freshly-derived
SQLCipher key and runs a sentinel `get_salt()` query before swapping the
db slot. On failure it returns `VaultError::BadPassword` (or the underlying
`Db` error) and the prior connection is preserved untouched. On success the
prior connection (if any) is dropped and replaced. The cost is one extra
Argon2 + SQLCipher open per re-unlock — acceptable for an interactive flow
and a fair trade for the UX correctness gain.

Regression test: `vault::tests::unlock_wrong_password_fails_after_prior_lock`
covers unlock → lock → wrong-unlock (asserts `BadPassword`/`Db`) →
correct-unlock (asserts a usable session, proving the AES key was correctly
re-derived).

### V-4 (LOW) — vault DB file mode 0600 — **FIXED**

Commit: `sec: V-4 — chmod 0600 vault DB file on create`
(`crates/holster-vault/src/vault.rs`).

`Vault::create` now calls a new `set_vault_file_perms` helper after
`Database::open` + `set_salt`. Helper is `cfg(unix)` and uses
`std::os::unix::fs::PermissionsExt::from_mode(0o600)`; on non-Unix targets
it is a no-op. Failure is intentionally swallowed (best-effort) to mirror
the existing salt-sidecar policy — some filesystems do not honour Unix
mode bits.

Regression test: `vault::tests::create_sets_vault_file_mode_0600` (gated
on `cfg(unix)`) asserts both the vault DB and the salt sidecar are 0600
after `Vault::create`.

### Verification runs at resolution time

- `cargo test --workspace` → **50 passed, 0 failed, 0 ignored**
  (was 48 in the original review; +1 for V-1 regression, +1 for V-4 regression).
- `cargo clippy --workspace --all-targets -- -D warnings` → **clean**.
- `cargo fmt --all -- --check` → **clean**.

### Items intentionally deferred to M2 backlog

The remaining LOW/polish recommendations (C-6 `#[allow]` comment, D-5
`VaultError::Internal` variant, D-8 SQLCipher CLI smoke test, M-4
`SecretString` for `AddKeyInput::key_value`, V-5 zxcvbn-driven
`WeakPassword`, tracing redaction layer) are not blockers for M1
sign-off and remain on the M2 list.

— Claude Code, 2026-04-26 (resolution pass)
