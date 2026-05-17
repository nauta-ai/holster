from __future__ import annotations

from typing import Any

from holster_mcp.free.rotation_playbook import rotation_playbook
from holster_mcp.paid.vault_add import vault_add
from holster_mcp.util import cli_shim
from holster_mcp.util.config import resolve_vault_path

SUPPORTED_PROVIDERS = {"github", "stripe"}


def vault_rotate(
    provider: str,
    account: str,
    vault_entry_id: str,
    *,
    new_secret: str | None = None,
    label: str | None = None,
    vault_path: str | None = None,
) -> dict[str, Any]:
    key = provider.strip().lower()
    if key not in SUPPORTED_PROVIDERS:
        return {
            "ok": False,
            "steps_completed": [],
            "steps_waiting_on_user": [],
            "error": f"provider_not_supported_day2: {key}",
        }

    playbook = rotation_playbook(key)
    if not new_secret:
        return {
            "ok": True,
            "steps_completed": ["rotation_playbook_prepared"],
            "steps_waiting_on_user": playbook["steps"],
            "new_vault_entry_id": None,
        }

    add_result = vault_add(
        key,
        account,
        new_secret,
        label or f"{key}-{account}-rotated",
        vault_path=vault_path,
    )
    if not add_result.get("ok"):
        return {
            "ok": False,
            "steps_completed": ["rotation_playbook_prepared"],
            "steps_waiting_on_user": ["new_secret_paste_or_vault_add_failed"],
            "error": add_result.get("error") or "vault_add_failed",
        }

    new_id = str(add_result["vault_entry_id"])
    supersede = _mark_superseded(vault_entry_id, new_id, vault_path)
    steps_completed = ["rotation_playbook_prepared", "replacement_secret_added_to_vault"]
    if supersede.get("ok"):
        steps_completed.append("old_entry_marked_superseded")
    return {
        "ok": True,
        "steps_completed": steps_completed,
        "steps_waiting_on_user": ["revoke_old_provider_credential_after_validation"],
        "new_vault_entry_id": new_id,
        "warning": None if supersede.get("ok") else supersede.get("error"),
    }


def _mark_superseded(old_entry_id: str, new_entry_id: str, vault_path: str | None) -> dict[str, Any]:
    try:
        return cli_shim.mark_superseded(
            vault_path=resolve_vault_path(vault_path),
            old_entry_id=old_entry_id,
            new_entry_id=new_entry_id,
        )
    except Exception as exc:  # noqa: BLE001
        return {"ok": False, "error": str(exc) or type(exc).__name__}
