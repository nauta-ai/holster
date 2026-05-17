from __future__ import annotations

from pathlib import Path
from typing import Any

from holster_mcp.util.doctor_shim import run_doctor

MAX_DEPTH = 10
MAX_SCANNED_FILES = 1000


def scan_repo(path: str, depth: int = 3) -> dict[str, Any]:
    root = _absolute_dir(path)
    if depth < 1 or depth > MAX_DEPTH:
        raise ValueError(f"depth must be between 1 and {MAX_DEPTH}")

    data = run_doctor(["scan", str(root), "--depth", str(depth), "--json"], timeout=120)
    scanned_files = int(data.get("scanned_files", 0))
    if scanned_files > MAX_SCANNED_FILES:
        raise ValueError(f"scan exceeded {MAX_SCANNED_FILES} files")

    return {
        "ok": True,
        "scanned_files": scanned_files,
        "findings": [_normalize_finding(finding) for finding in data.get("findings", [])],
    }


def _absolute_dir(path: str) -> Path:
    root = Path(path).expanduser()
    if not root.is_absolute():
        raise ValueError("path must be absolute")
    if not root.exists():
        raise ValueError(f"path does not exist: {root}")
    if not root.is_dir():
        raise ValueError(f"path is not a directory: {root}")
    return root.resolve()


def _normalize_finding(finding: dict[str, Any]) -> dict[str, Any]:
    return {
        "file": finding.get("file"),
        "line": finding.get("line"),
        "secret_kind": finding.get("kind") or finding.get("secret_kind"),
        "severity": _normalize_severity(str(finding.get("severity") or "")),
        "suggestion": finding.get("suggestion") or "",
    }


def _normalize_severity(severity: str) -> str:
    lowered = severity.lower()
    if lowered in {"critical", "high", "error"}:
        return "error"
    if lowered in {"medium", "warn", "warning"}:
        return "warn"
    return "info"
