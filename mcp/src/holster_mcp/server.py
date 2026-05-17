from __future__ import annotations

from mcp.server.fastmcp import FastMCP

from holster_mcp.free import check_gitignore, rotation_playbook, scan_repo
from holster_mcp.paid import audit_log, vault_add, vault_rotate
from holster_mcp.paid import license as license_gate

mcp = FastMCP("holster")


@mcp.tool(name="holster.scan_repo")
def tool_scan_repo(path: str, depth: int = 3) -> dict:
    """Scan a local repository for secret-shaped values."""

    return scan_repo(path=path, depth=depth)


@mcp.tool(name="holster.check_gitignore")
def tool_check_gitignore(path: str) -> dict:
    """Audit .gitignore coverage for common secret-bearing files."""

    return check_gitignore(path=path)


@mcp.tool(name="holster.rotation_playbook")
def tool_rotation_playbook(provider: str) -> dict:
    """Return a local provider-specific credential rotation playbook."""

    return rotation_playbook(provider=provider)


@mcp.tool(name="holster.vault_add")
def tool_vault_add(
    provider: str,
    account: str,
    secret: str,
    label: str,
    license_key: str | None = None,
) -> dict:
    """Add a secret to the user's local Holster vault."""

    status = license_gate.require_valid_license(license_key)
    if not status.ok:
        return status.as_error()
    result = vault_add(provider=provider, account=account, secret=secret, label=label)
    if status.warning and result.get("ok"):
        result["license_warning"] = status.warning
    return result


@mcp.tool(name="holster.vault_rotate")
def tool_vault_rotate(
    provider: str,
    account: str,
    vault_entry_id: str,
    new_secret: str | None = None,
    label: str | None = None,
    license_key: str | None = None,
) -> dict:
    """Prepare a rotation flow and optionally store a user-pasted replacement secret."""

    status = license_gate.require_valid_license(license_key)
    if not status.ok:
        return status.as_error()
    result = vault_rotate(
        provider=provider,
        account=account,
        vault_entry_id=vault_entry_id,
        new_secret=new_secret,
        label=label,
    )
    if status.warning and result.get("ok"):
        result["license_warning"] = status.warning
    return result


@mcp.tool(name="holster.audit_log")
def tool_audit_log(
    provider: str | None = None,
    account: str | None = None,
    since_days: int = 30,
    license_key: str | None = None,
) -> dict:
    """Read local Holster vault audit events."""

    status = license_gate.require_valid_license(license_key)
    if not status.ok:
        return status.as_error()
    result = audit_log(provider=provider, account=account, since_days=since_days)
    if status.warning and result.get("ok"):
        result["license_warning"] = status.warning
    return result


def main() -> None:
    mcp.run()


if __name__ == "__main__":
    main()
