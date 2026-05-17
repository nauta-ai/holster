from __future__ import annotations

from pathlib import Path
from typing import Any

from holster_mcp.util.doctor_shim import run_doctor


def check_gitignore(path: str) -> dict[str, Any]:
    root = _absolute_dir(path)
    data = run_doctor(["gitignore-audit", str(root), "--json"], timeout=60)
    return {
        "ok": True,
        "missing_patterns": list(data.get("missing_patterns", [])),
        "existing_safe": list(data.get("existing_safe", [])),
        "existing_unsafe": list(data.get("existing_unsafe", [])),
        "suggested_append": str(data.get("suggested_append") or ""),
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
