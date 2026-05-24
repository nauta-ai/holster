# Holster

Local-first API key manager for AI developers.

## Install

### macOS Apple Silicon (M-series)

1. Download `holster-cli-macos-arm64.tar.gz` + `.sha256` from the latest release: https://github.com/nauta-ai/holster/releases/latest
2. Extract: `tar xzf holster-cli-macos-arm64.tar.gz`
3. Verify: `shasum -a 256 -c holster-cli-macos-arm64.tar.gz.sha256`
4. Install: `sudo mv holster-cli /usr/local/bin/`
5. If macOS blocks first run: `xattr -d com.apple.quarantine /usr/local/bin/holster-cli`
6. Verify: `holster-cli --version`

### macOS Intel (x86_64, since v0.7.0)

1. Download `holster-cli-macos-x86_64.tar.gz` + `.sha256` from the latest release.
2. Same extract / verify / install steps as above.

### Linux x86_64

1. Download `holster-cli-linux-x86_64.tar.gz` + `.sha256` from the latest release.
2. Extract: `tar xzf holster-cli-linux-x86_64.tar.gz`
3. Verify: `shasum -a 256 -c holster-cli-linux-x86_64.tar.gz.sha256`
4. Install: `sudo mv holster-cli /usr/local/bin/`
5. Verify: `holster-cli --version`

Password handling on Linux: macOS Keychain is not available. Use `--password-env <ENV_NAME>` to read the vault password from an environment variable, or `--password-stdin` to pipe it via stdin.

### Windows x86_64 (since v0.7.0)

1. Download `holster-cli-windows-x86_64.zip` + `.sha256` from the latest release.
2. Verify: in PowerShell, `(Get-FileHash -Algorithm SHA256 holster-cli-windows-x86_64.zip).Hash` should match the value in the `.sha256` file.
3. Extract: `Expand-Archive holster-cli-windows-x86_64.zip -DestinationPath .`
4. Move `holster-cli.exe` somewhere on `$env:Path` (e.g. `C:\Users\<you>\bin\` after adding that folder to PATH).
5. Verify: `holster-cli.exe --version`

Password handling on Windows: macOS Keychain is not available. Use `--password-env <ENV_NAME>` or `--password-stdin`.

## Rotating the master password (since v0.7.0)

Holster lets you change the vault's master password without re-creating the vault or losing entries:

```bash
holster-cli rotate-master /path/to/vault
# Prompts for OLD master, then NEW master twice (confirm)
```

This re-encrypts every entry under the new master and regenerates the vault salt under a single SQLite transaction. If the rotation fails or is interrupted, the vault remains intact under the old master.

If you cache the master password in macOS Keychain (common for daemon usage), pass `--keychain-update SERVICE,ACCOUNT` to update the cached entry in the same command:

```bash
holster-cli rotate-master /path/to/vault \
  --keychain-update holster-personas-vault,admin
```

For unattended / scripted rotation:

```bash
# OLD pw from Keychain, NEW pw from env
holster-cli rotate-master /path/to/vault \
  --old-password-keychain-service holster-personas-vault \
  --old-password-keychain-account admin \
  --new-password-env NEW_HOLSTER_MASTER

# Both from stdin (one line each, OLD first then NEW)
printf '%s\n%s\n' "$OLD_PW" "$NEW_PW" | \
  holster-cli rotate-master /path/to/vault \
  --old-password-stdin --new-password-stdin
```

**Rotate the master password:**
- After any suspected credential exposure
- After a team member who knew the master leaves
- Quarterly as routine hygiene
- Whenever the master appears in plaintext somewhere (logs, configs, transcripts)

## Password sources (since v0.7.0)

`get`, `add`, `rotate-master`, and `exec-env` now accept the master password from multiple sources. Precedence (first non-None wins):

1. `--password-env <NAME>` — read from an environment variable
2. `--password-stdin` — read first line of stdin
3. `--password-keychain-service <SVC> [--password-keychain-account <ACCT>]` — macOS Keychain
4. Interactive TTY prompt (fallback)

This makes Holster scriptable from CI, launchd, systemd, and daemon contexts without requiring an interactive TTY.

## Documentation

Read `docs/framework/00_README.md` to start.

## Status

Pre-alpha. v0.7.0 adds rotate-master + scriptable password sources + Windows/macOS-x86_64 builds. Vault crate stable since M1.
