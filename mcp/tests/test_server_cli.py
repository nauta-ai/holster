from __future__ import annotations

import pytest

from holster_mcp import server


def test_help_exits_without_starting_server(capsys):
    server.main(["--help"])

    out = capsys.readouterr().out
    assert "holster-mcp" in out
    assert "Usage:" in out
    assert "stdio" in out


def test_version_exits_without_starting_server(capsys):
    server.main(["--version"])

    out = capsys.readouterr().out
    assert out.startswith("holster-mcp ")


def test_unknown_argument_exits_before_server_start():
    with pytest.raises(SystemExit) as exc:
        server.main(["--wat"])

    assert "unknown argument: --wat" in str(exc.value)
