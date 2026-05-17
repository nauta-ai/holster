from __future__ import annotations

import json
import os
import shutil
import subprocess
from pathlib import Path
from typing import Any


class DoctorError(RuntimeError):
    pass


def repo_root() -> Path:
    return Path(__file__).resolve().parents[5]


def packaged_binary() -> Path:
    return Path(__file__).resolve().parents[1] / "bin" / "holster-doctor"


def find_holster_doctor() -> Path:
    env_path = os.environ.get("HOLSTER_DOCTOR_BIN")
    if env_path:
        candidate = Path(env_path).expanduser()
        if candidate.is_file():
            return candidate
        raise DoctorError(f"HOLSTER_DOCTOR_BIN does not point to a file: {candidate}")

    packaged = packaged_binary()
    if packaged.is_file():
        return packaged

    path_hit = shutil.which("holster-doctor")
    if path_hit:
        return Path(path_hit)

    root = repo_root()
    for candidate in [
        root / "target" / "release" / "holster-doctor",
        root / "target" / "debug" / "holster-doctor",
    ]:
        if candidate.is_file():
            return candidate

    raise DoctorError("holster-doctor binary not found")


def run_doctor(args: list[str], timeout: int = 60) -> dict[str, Any]:
    binary = find_holster_doctor()
    cmd = [str(binary), *args]
    try:
        completed = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired as exc:
        raise DoctorError(f"holster-doctor timed out after {timeout}s") from exc

    stdout = completed.stdout.strip()
    if completed.returncode != 0:
        error = _extract_error(stdout) or completed.stderr.strip() or "unknown doctor failure"
        raise DoctorError(error)

    try:
        data = json.loads(stdout)
    except json.JSONDecodeError as exc:
        raise DoctorError("holster-doctor returned non-JSON output") from exc

    if isinstance(data, dict) and data.get("ok") is False:
        raise DoctorError(str(data.get("error") or "holster-doctor returned ok=false"))
    if not isinstance(data, dict):
        raise DoctorError("holster-doctor returned unexpected JSON shape")
    return data


def _extract_error(body: str) -> str | None:
    try:
        data = json.loads(body)
    except json.JSONDecodeError:
        return None
    if isinstance(data, dict) and data.get("error"):
        return str(data["error"])
    return None
