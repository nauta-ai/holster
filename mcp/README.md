# Holster MCP

Holster MCP exposes local Holster Doctor checks to MCP-compatible agents. The server is local-first: free tools shell out to the bundled `holster-doctor` binary and never send repository contents to a network service.

## Install

Development install:

```bash
cd /path/to/holster/mcp
python3.11 -m pip install -e '.[dev]'
```

Runtime command:

```bash
holster-mcp
```

MCP client config:

```json
{
  "mcpServers": {
    "holster": {
      "command": "uvx",
      "args": ["holster-mcp"]
    }
  }
}
```

## Binary Resolution

The server locates `holster-doctor` in this order:

1. `HOLSTER_DOCTOR_BIN`
2. Packaged wheel binary at `holster_mcp/bin/holster-doctor`
3. `holster-doctor` on `PATH`
4. Repo-local `target/release/holster-doctor`
5. Repo-local `target/debug/holster-doctor`

## Free Tools

### `holster.scan_repo`

Input:

```json
{"path": "/absolute/repo/path", "depth": 3}
```

Output:

```json
{
  "ok": true,
  "scanned_files": 15,
  "findings": [
    {
      "file": "src/main.py",
      "line": 12,
      "secret_kind": "openai_api_key",
      "severity": "error",
      "suggestion": "Rotate this key and move it into a local vault."
    }
  ]
}
```

### `holster.check_gitignore`

Input:

```json
{"path": "/absolute/repo/path"}
```

Output:

```json
{
  "ok": true,
  "missing_patterns": [".env.local"],
  "existing_safe": [".env"],
  "existing_unsafe": [],
  "suggested_append": ".env.local\n*.pem"
}
```

### `holster.rotation_playbook`

Input:

```json
{"provider": "github"}
```

Output:

```json
{
  "ok": true,
  "provider": "github",
  "steps": ["Create a replacement token...", "Update local consumers...", "Revoke the old token..."],
  "estimated_minutes": 15,
  "warnings": ["Do not revoke the old credential until the replacement has been verified."]
}
```

## Platform Wheels

Day 1 wheels are built for:

- macOS ARM (`aarch64-apple-darwin`)
- Linux x86_64 (`x86_64-unknown-linux-gnu`)

The wheels bundle the platform's `holster-doctor` binary and are intentionally not universal.
