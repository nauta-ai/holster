from __future__ import annotations

from mcp.server.fastmcp import FastMCP

from holster_mcp.free import check_gitignore, rotation_playbook, scan_repo

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


def main() -> None:
    mcp.run()


if __name__ == "__main__":
    main()
