from __future__ import annotations

from typing import Any

from holster_mcp.util import cli_shim
from holster_mcp.util.config import resolve_vault_path


def vault_add(
    provider: str,
    account: str,
    secret: str,
    label: str,
    *,
    vault_path: str | None = None,
) -> dict[str, Any]:
    if not provider.strip():
        return {"ok": False, "vault_entry_id": "", "error": "provider_required"}
    if not account.strip():
        return {"ok": False, "vault_entry_id": "", "error": "account_required"}
    if not label.strip():
        return {"ok": False, "vault_entry_id": "", "error": "label_required"}
    if not secret:
        return {"ok": False, "vault_entry_id": "", "error": "empty_secret_rejected"}

    try:
        resolved_vault = resolve_vault_path(vault_path)
        return cli_shim.add_secret(
            vault_path=resolved_vault,
            provider=provider.strip(),
            account=account.strip(),
            secret=secret,
            label=label.strip(),
        )
    except Exception as exc:  # noqa: BLE001 - tool boundary returns structured errors.
        return {"ok": False, "vault_entry_id": "", "error": _safe_error(exc)}


def _safe_error(exc: Exception) -> str:
    message = str(exc) or type(exc).__name__
    return message if len(message) < 512 else message[:512]
