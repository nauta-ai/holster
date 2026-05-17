from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
from pathlib import Path
from typing import Any


class CliError(RuntimeError):
    pass


UUID_RE = re.compile(r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}")


def repo_root() -> Path:
    return Path(__file__).resolve().parents[5]


def find_holster_cli() -> Path:
    env_path = os.environ.get("HOLSTER_CLI_BIN")
    if env_path:
        candidate = Path(env_path).expanduser()
        if candidate.is_file():
            return candidate
        raise CliError(f"HOLSTER_CLI_BIN does not point to a file: {candidate}")

    path_hit = shutil.which("holster-cli")
    if path_hit:
        return Path(path_hit)

    root = repo_root()
    for candidate in [
        root / "target" / "release" / "holster-cli",
        root / "target" / "debug" / "holster-cli",
    ]:
        if candidate.is_file():
            return candidate

    raise CliError("holster-cli binary not found")


def add_secret(
    *,
    vault_path: Path,
    provider: str,
    account: str,
    secret: str,
    label: str,
    timeout: int = 60,
) -> dict[str, Any]:
    if not secret:
        return {"ok": False, "vault_entry_id": "", "error": "empty_secret_rejected"}

    password = os.environ.get("HOLSTER_VAULT_PASSWORD")
    stdin = f"{password or ''}\n{secret}\n" if password is not None else f"\n{secret}\n"
    result = run_cli(
        [
            "add",
            str(vault_path),
            "--provider",
            provider,
            "--label",
            label,
            "--project",
            account,
        ],
        stdin=stdin,
        timeout=timeout,
        redactions=[secret],
    )
    if not result["ok"]:
        return {"ok": False, "vault_entry_id": "", "error": result["error"]}

    entry_id = _extract_entry_id(result["stdout_tail"])
    return {"ok": bool(entry_id), "vault_entry_id": entry_id, "error": None if entry_id else "vault_entry_id_missing"}


def mark_superseded(*, vault_path: Path, old_entry_id: str, new_entry_id: str, timeout: int = 30) -> dict[str, Any]:
    result = run_cli(
        ["supersede", str(vault_path), old_entry_id, "--replacement", new_entry_id],
        timeout=timeout,
        redactions=[],
    )
    return {"ok": result["ok"], "error": result["error"]}


def audit_log(
    *,
    vault_path: Path,
    provider: str | None = None,
    account: str | None = None,
    since_days: int = 30,
    timeout: int = 30,
) -> dict[str, Any]:
    args = ["audit-log", str(vault_path), "--since-days", str(since_days), "--json"]
    if provider:
        args.extend(["--provider", provider])
    if account:
        args.extend(["--account", account])
    result = run_cli(args, timeout=timeout, redactions=[])
    if not result["ok"]:
        return {"ok": False, "events": [], "error": result["error"]}
    try:
        data = json.loads(result["stdout_tail"])
    except json.JSONDecodeError:
        return {"ok": False, "events": [], "error": "audit_log_non_json"}
    events = data.get("events", []) if isinstance(data, dict) else []
    return {"ok": True, "events": events, "error": None}


def run_cli(
    args: list[str],
    *,
    stdin: str | None = None,
    timeout: int = 60,
    redactions: list[str] | None = None,
) -> dict[str, Any]:
    binary = find_holster_cli()
    try:
        completed = subprocess.run(
            [str(binary), *args],
            input=stdin,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired:
        return {"ok": False, "exit_code": None, "stdout_tail": "", "stderr_tail": "", "error": "holster_cli_timeout"}

    stdout_tail = _tail(_redact(completed.stdout, redactions or []))
    stderr_tail = _tail(_redact(completed.stderr, redactions or []))
    if completed.returncode != 0:
        return {
            "ok": False,
            "exit_code": completed.returncode,
            "stdout_tail": stdout_tail,
            "stderr_tail": stderr_tail,
            "error": stderr_tail or stdout_tail or f"holster_cli_exit_{completed.returncode}",
        }
    return {"ok": True, "exit_code": 0, "stdout_tail": stdout_tail, "stderr_tail": stderr_tail, "error": None}


def _extract_entry_id(output: str) -> str:
    match = UUID_RE.search(output)
    return match.group(0) if match else ""


def _redact(value: str, redactions: list[str]) -> str:
    out = value
    for secret in redactions:
        if secret:
            out = out.replace(secret, "<REDACTED>")
    return out


def _tail(value: str, limit: int = 16_384) -> str:
    if len(value) <= limit:
        return value
    return "...[truncated]\n" + value[-limit:]
