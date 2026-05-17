# Holster

Local-first API key manager for AI developers.

## Install

### Linux x86_64

1. Download `holster-cli-linux-x86_64.tar.gz` and `holster-cli-linux-x86_64.tar.gz.sha256` from the latest release: https://github.com/nauta-ai/holster/releases/latest
2. Extract: `tar xzf holster-cli-linux-x86_64.tar.gz`
3. Verify checksum: `shasum -a 256 -c holster-cli-linux-x86_64.tar.gz.sha256`
4. Move to PATH: `sudo mv holster-cli /usr/local/bin/`
5. Verify: `holster-cli --version`

Password handling on Linux: macOS Keychain is not available. Use `--password-env <ENV_NAME>` to read the vault password from an environment variable, or pipe the password via stdin.

### macOS ARM

1. Download `holster-cli-macos-arm64.tar.gz` and `holster-cli-macos-arm64.tar.gz.sha256` from the latest release: https://github.com/nauta-ai/holster/releases/latest
2. Extract: `tar xzf holster-cli-macos-arm64.tar.gz`
3. Verify checksum: `shasum -a 256 -c holster-cli-macos-arm64.tar.gz.sha256`
4. Move to PATH: `sudo mv holster-cli /usr/local/bin/`
5. If macOS blocks first run, clear quarantine: `xattr -d com.apple.quarantine /usr/local/bin/holster-cli`
6. Verify: `holster-cli --version`

Read `docs/framework/00_README.md` to start.

Status: pre-alpha, M1 scaffolded, T1.0 complete, T1.1+ pending.
