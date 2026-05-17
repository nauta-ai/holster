from __future__ import annotations

from typing import Any

from holster_mcp.util import cli_shim
from holster_mcp.util.config import resolve_vault_path


def audit_log(
    provider: str | None = None,
    account: str | None = None,
    since_days: int = 30,
    *,
    vault_path: str | None = None,
) -> dict[str, Any]:
    if since_days < 1 or since_days > 3650:
        return {"ok": False, "events": [], "error": "since_days_out_of_range"}
    try:
        return cli_shim.audit_log(
            vault_path=resolve_vault_path(vault_path),
            provider=provider,
            account=account,
            since_days=since_days,
        )
    except Exception as exc:  # noqa: BLE001
        return {"ok": False, "events": [], "error": str(exc) or type(exc).__name__}
