from __future__ import annotations

import os
from pathlib import Path
from typing import Any

try:  # Python 3.11+
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - Python 3.10 fallback
    import tomli as tomllib  # type: ignore[no-redef]


def holster_home() -> Path:
    return Path(os.environ.get("HOLSTER_HOME", "~/.holster")).expanduser()


def config_path() -> Path:
    return Path(os.environ.get("HOLSTER_CONFIG_PATH", holster_home() / "config.toml")).expanduser()


def load_config() -> dict[str, Any]:
    path = config_path()
    if not path.is_file():
        return {}
    with path.open("rb") as handle:
        data = tomllib.load(handle)
    return data if isinstance(data, dict) else {}


def config_value(*keys: str) -> str | None:
    data: Any = load_config()
    for key in keys:
        if not isinstance(data, dict) or key not in data:
            return None
        data = data[key]
    return data if isinstance(data, str) and data else None


def resolve_vault_path(explicit: str | None = None) -> Path:
    raw = (
        explicit
        or os.environ.get("HOLSTER_VAULT_PATH")
        or config_value("vault_path")
        or config_value("vault", "path")
    )
    if not raw:
        raise ValueError("vault_path_required")
    path = Path(raw).expanduser()
    if not path.is_absolute():
        raise ValueError("vault_path_must_be_absolute")
    return path


def resolve_license_key(explicit: str | None = None) -> str | None:
    return explicit or os.environ.get("HOLSTER_LICENSE_KEY") or config_value("license_key") or config_value(
        "license", "key"
    )
