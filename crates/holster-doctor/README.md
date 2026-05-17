# holster-doctor

Standalone scanner library and CLI for Holster Doctor.

The JSON contract below is stable for the Python MCP server. Any future
breaking change to these shapes should be treated as a major version change.

## Commands

```text
holster-doctor scan <path> [--depth N] [--json]
holster-doctor gitignore-audit <path> [--json]
holster-doctor env-example <path> [--profile PROFILE] [--json]
holster-doctor preflight <path> [--profile PROFILE] [--json]
holster-doctor list-profiles [--json]
holster-doctor --version
holster-doctor --help
```

## JSON Output

`scan`:

```json
{
  "ok": true,
  "scanned_files": 12,
  "findings": [
    {
      "file": "src/main.rs",
      "line": 10,
      "kind": "openai_api_key",
      "severity": "critical",
      "suggestion": "Rotate immediately..."
    }
  ],
  "elapsed_ms": 42
}
```

`gitignore-audit`:

```json
{
  "ok": true,
  "missing_patterns": [".env", ".env.*"],
  "existing_safe": ["node_modules/"],
  "existing_unsafe": [],
  "suggested_append": ".env\n.env.*",
  "elapsed_ms": 8
}
```

`env-example`:

```json
{
  "ok": true,
  "generated_path": "/project/.env.example",
  "vars_extracted": ["OPENAI_API_KEY"],
  "elapsed_ms": 9
}
```

Without `--profile`, `env-example` derives names from `.env`, `.env.local`,
or `.env.development` in the target directory. With `--profile`, it uses the
static profile catalogue.

`preflight`:

```json
{
  "ok": true,
  "checks": [
    {"name": "wrapper_unpinned", "status": "risk", "detail": "Wrapper can fetch package code..."}
  ],
  "summary": "run=risky; share=safe; 1 finding(s)",
  "elapsed_ms": 4
}
```

`list-profiles`:

```json
{
  "ok": true,
  "profiles": ["Generic .env", "OpenClaw", "Claude Code", "Codex (OpenAI CLI)", "Hermes"]
}
```

Errors always return exit code `1`:

```json
{"ok": false, "error": "path does not exist: /missing"}
```
