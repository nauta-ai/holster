# 07 — Security Review Checklist (CC's Rubric)

This is the rubric Claude Code uses for every milestone review. Each item is a yes/no check. Any "no" blocks merge until resolved.

CC produces a written report at `~/holster/docs/reviews/M<N>-review-YYYYMMDD.md` after each review.

---

## Universal checks (every milestone)

### Code hygiene
- [ ] `cargo fmt --all -- --check` clean
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] `cargo audit` clean (no known CVEs in dependencies)
- [ ] `cargo test --workspace` all passing
- [ ] No `unwrap()`, `expect()`, or `panic!()` introduced in security-critical paths (vault, crypto, session, ipc)
- [ ] No `dbg!()` macros left in code
- [ ] No `println!`/`eprintln!` for diagnostics (use `tracing`)

### Logging hygiene
- [ ] No log statement contains: master password, plaintext key value, session token value, ciphertext, nonce, or salt
- [ ] `tracing` filter strips fields named `password`, `secret`, `key_value`, `token`, `master_key`, `aes_key`, `sqlcipher_key`
- [ ] Verified by grepping the codebase for `tracing::(info|debug|warn|error)` and reading each call

### Error handling
- [ ] No error variant in `VaultError` carries plaintext key material
- [ ] Errors thrown to the frontend via IPC don't include sensitive data
- [ ] `Display` impls for error types don't reveal internal state

### Memory hygiene
- [ ] All structs holding plaintext key material wrapped in `secrecy::Secret<>`
- [ ] `Drop` or `Zeroize` impl on session state
- [ ] No `Clone` on key material that would create unzeroed copies (use `Arc<Secret<>>` if shared)

---

## Crypto-specific checks (M1)

### Argon2id parameters
- [ ] `ARGON2_MEMORY_KB == 65_536` (64 MB)
- [ ] `ARGON2_TIME_COST == 3`
- [ ] `ARGON2_PARALLELISM == 4`
- [ ] `Algorithm::Argon2id` (NOT `Argon2i` or `Argon2d`)
- [ ] `Version::V0x13` (latest)

### AES-GCM
- [ ] `Aes256Gcm` (256-bit key, NOT 128)
- [ ] Nonce is 12 bytes (`NONCE_LEN == 12`)
- [ ] Each encryption generates a fresh nonce via `OsRng`
- [ ] Nonce is stored alongside ciphertext (recoverable for decryption)
- [ ] No nonce reuse across encryptions with the same key (verified by code review — fresh nonce per call)

### Randomness
- [ ] All cryptographic randomness uses `OsRng` (NOT `thread_rng`)
- [ ] Salt generation uses `OsRng`
- [ ] UUID generation uses `Uuid::new_v4()` (which uses OsRng internally — verify version)

### SQLCipher
- [ ] `PRAGMA key` is set with the derived 32-byte key in hex format (`x'...'`)
- [ ] `PRAGMA cipher_compatibility = 4` (latest format)
- [ ] `PRAGMA kdf_iter` not overridden (use SQLCipher 4 defaults)
- [ ] Vault file is unreadable via plain `sqlite3` CLI (manual test)

### Salt handling
- [ ] Salt is 16 bytes
- [ ] Salt is generated once per vault, stored in `vault_meta` table
- [ ] Salt is never reused across vaults

### Password validation
- [ ] zxcvbn score ≥ 3 enforced at vault creation
- [ ] Password is consumed (moved or zeroized) after key derivation, not held longer than necessary

---

## IPC-specific checks (M2 onward)

### Session token enforcement
- [ ] EVERY IPC command that touches key material checks session token first
- [ ] Token validation includes idle timeout check
- [ ] Invalid token returns `VaultError::InvalidSession` (not a generic error)

### Frontend boundary
- [ ] Plaintext key values are never sent over IPC except in response to:
  - `get_key_value` (one-shot, frontend uses immediately and discards)
  - `copy_key_to_clipboard` (handled entirely in Rust, frontend gets only success/fail)
- [ ] No key value is stored in React state, Zustand store, localStorage, or sessionStorage
- [ ] Verified by grep through frontend code: no variable named `keyValue`, `secret`, `apiKey` holds a plaintext value past one render cycle

### Clipboard handling
- [ ] Clipboard write is followed by a TTL timer that overwrites with empty string
- [ ] TTL timer is in Rust (not JavaScript) — frontend cannot extend it
- [ ] On lock or session expiry, any pending clipboard timer fires immediately
- [ ] Verified manually: copy a key, lock vault, paste — should be empty

---

## Network checks (M5 onward)

### TLS configuration
- [ ] `reqwest` configured with `rustls-tls` (NOT native-tls / openssl)
- [ ] Minimum TLS version 1.2, prefer 1.3
- [ ] No `danger_accept_invalid_certs` anywhere
- [ ] No `danger_accept_invalid_hostnames` anywhere

### Provider endpoint URLs
- [ ] Anthropic usage endpoint URL verified against current 2026 documentation
- [ ] OpenAI usage endpoint URL verified against current 2026 documentation
- [ ] No URLs hardcoded in multiple places — single source of truth in `providers/<name>.rs`

### Data exfiltration check
- [ ] Holster makes outbound network calls ONLY to:
  - Provider usage endpoints (when usage tracking enabled per key)
  - License validation endpoint (Pro only)
  - Update check endpoint (Tauri updater)
- [ ] No analytics, no error reporting service, no third-party SDKs that phone home
- [ ] If telemetry added (M8), payload is auditable and contains zero key material or counts that could fingerprint

---

## Filesystem checks

- [ ] Vault file path is `~/Library/Application Support/Holster/vault.db` (or sandbox-aware equivalent)
- [ ] Directory created with mode 0700
- [ ] Vault file created with mode 0600
- [ ] Backup files written to `~/Library/Application Support/Holster/backups/` with mode 0600
- [ ] No vault content written to `/tmp` or any world-readable location

---

## Leak scanner checks (M4)

- [ ] Regex patterns verified against current 2026 provider key formats
- [ ] Scanner respects `.gitignore`
- [ ] Scanner skips: `node_modules`, `target`, `.venv`, `dist`, `build`, `.git`
- [ ] Scanner does not follow symlinks outside the chosen root
- [ ] File size limit enforced (5MB max per file)
- [ ] Total scan time limit enforced (60s max, with progress)
- [ ] Findings display masks all but last 4 chars of matched key
- [ ] Path display sanitized (no terminal escape sequences, no path traversal in display strings)

---

## Build / release checks (M8)

- [ ] Release builds compiled with `panic = "abort"` (no unwinding through crypto code)
- [ ] Release binaries stripped of debug symbols
- [ ] Code signing identity is Apple Developer ID (not ad-hoc)
- [ ] Notarization ticket stapled
- [ ] DMG checksum published
- [ ] Auto-updater verifies signature on update payload before applying

---

## CC's review report template

```markdown
# Holster M<N> Security Review

**Reviewer:** Claude Code
**Date:** YYYY-MM-DD
**Branch:** milestone/M<N>-<name>
**Commit:** <sha>

## Summary
<one paragraph: approved / blocked / approved with notes>

## Checks
<list of each check from the relevant section, with ✅ or ❌>

## Findings
### Blocking
<numbered list of issues that must be fixed before merge>

### Non-blocking notes
<numbered list of issues that should be tracked for future cleanup>

## Test runs
- `cargo test --workspace`: <pass/fail with count>
- `cargo clippy`: <clean/warnings>
- `cargo audit`: <clean/vulns>
- Manual scenarios run: <list>

## Recommendation
<approved for merge / changes required>
```
