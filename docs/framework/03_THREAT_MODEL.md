# 03 — Threat Model

## Assets

| Asset | Sensitivity | Notes |
|---|---|---|
| Plaintext API keys | Critical | The whole product exists to protect these |
| Master password | Critical | Derives all encryption keys |
| Vault file (encrypted) | High | Without master password it's still encrypted, but should not leak |
| Per-key encryption key (in memory) | Critical | Held only while unlocked |
| Session tokens | Medium | Short-lived, revoked on lock |
| Usage data (token counts, spend) | Low-Medium | Reveals user activity patterns |

## Attackers in scope (we defend against)

### A1: Casual file-system access
Someone opens the user's `~/Library/Application Support/Holster/vault.db` via Finder, Time Machine browse, or a misconfigured backup tool.

**Defense:** SQLCipher whole-DB encryption + per-key AES-GCM. File is unreadable without master password.

### A2: Stolen/lost device, screen unlocked briefly then locked
Attacker has the device, doesn't know macOS login or master password.

**Defense:** Auto-lock on display lock, idle timeout, full-disk encryption assumed (FileVault). Master password required at next launch unless user opted into Keychain wrap (which is gated by macOS login).

### A3: Malicious Git push
User accidentally `git add .env` and pushes a key to a public repo.

**Defense:** Leak scanner pre-commit hook + on-demand repo scan that flags exposed keys.

### A4: Curious dev tools
A misbehaving Tauri devtools session, debug log, or crash report tries to leak a key value.

**Defense:** `secrecy::Secret<>` wrappers prevent accidental Debug formatting; logging filter strips known-sensitive field names; debug builds gated from production releases.

### A5: Clipboard sniffing by another running app
Another app polls the clipboard.

**Defense:** 30s auto-clear default, configurable down to 5s, never longer than 5 min.

### A6: Network MITM during usage polling
Attacker intercepts traffic when Holster polls Anthropic/OpenAI for usage data.

**Defense:** TLS 1.3 via `rustls`, certificate pinning for known provider endpoints, no key values transmitted (we send the user's API key as auth, that's the inherent provider trust model — not a Holster issue).

### A7: Compromised iCloud sync (if Pro user opts in)
Attacker has user's iCloud credentials.

**Defense:** End-to-end encryption — sync payload is the already-encrypted vault file. iCloud only sees ciphertext. Without the master password, iCloud access alone doesn't yield keys.

## Attackers out of scope (we explicitly do NOT defend against)

### O1: Fully compromised macOS
Root-level malware, kernel rootkit, hardware keylogger. If the OS is owned, no app survives.

### O2: Coerced unlock
User is forced to enter their master password. We are not a duress system. (May add duress password feature in v2.)

### O3: Master password compromise
If the user's master password is "password123" or written on a sticky note, no encryption helps. We require minimum strength (zxcvbn score ≥ 3) at vault creation.

### O4: Side-channel attacks
Timing attacks on Argon2, cache-timing on AES, EM analysis. We use audited crates with constant-time implementations where standard, but a determined nation-state with physical access is out of scope.

### O5: Provider-side key compromise
If Anthropic's database is breached, that's not a Holster failure. We surface rotation prompts to encourage hygiene.

### O6: User shares their vault file AND password
Self-explanatory. Out of scope.

## Trust boundaries

```
[ User Memory ] ↔ [ Master Password Input ]
                          ↓
                  [ Argon2id KDF ]
                          ↓
            [ Per-Key AES Key (in memory) ]
                          ↓
                  [ AES-GCM Decrypt ]
                          ↓
                  [ Plaintext Key (in Secret<>) ]
                          ↓
              [ Clipboard / IPC to Frontend ]   ← TRUST BOUNDARY
                          ↓
                    [ User pastes ]
```

The IPC boundary between Rust and React is a trust boundary. The frontend should be treated as semi-trusted: it's our code, but it runs in a webview that could in theory be compromised by a malicious dependency. **No raw key values flow through the frontend except for the brief moment of a copy-to-clipboard action, and even then the value is wrapped and zeroized after use.**

## Non-negotiable invariants

1. The master password never leaves the user's machine. Period.
2. Decrypted key values are never written to disk.
3. Decrypted key values are never logged.
4. Decrypted key values are never sent in IPC payloads except in response to a `get_key_value` or `copy_key_to_clipboard` command, and only after session token validation.
5. Idle timeout is enforced server-side (in the Rust backend), not by the frontend. Frontend timeouts are advisory only.
6. All randomness (nonces, salts, UUIDs) comes from `OsRng`, never from `rand::thread_rng()` for cryptographic uses.
7. No `unwrap()`, `expect()`, or `panic!()` in any code path that handles plaintext key material.
