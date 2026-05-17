from __future__ import annotations

import importlib
from pathlib import Path

import pytest

from holster_mcp.paid import license as license_module
from holster_mcp.paid import vault_add as vault_add_func
from holster_mcp.paid import vault_rotate as vault_rotate_func
from holster_mcp.paid.audit_log import audit_log
from holster_mcp.util import cli_shim

vault_add_module = importlib.import_module("holster_mcp.paid.vault_add")
vault_rotate_module = importlib.import_module("holster_mcp.paid.vault_rotate")


LIVE_KEY = "holster_live_ABCDEFGHIJKLMNOPQRSTUVWX"


@pytest.fixture(autouse=True)
def paid_env(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    monkeypatch.setenv("HOLSTER_LICENSE_DB", str(tmp_path / "licenses.db"))
    monkeypatch.setenv("HOLSTER_VAULT_PATH", str(tmp_path / "vault.db"))
    monkeypatch.delenv("HOLSTER_CLI_BIN", raising=False)
    license_module.seed_license(
        LIVE_KEY,
        customer_id="cus_123",
        valid_until=1_800_000_000,
        last_checked=1_799_999_000,
        trial=False,
    )


def test_vault_add_calls_cli_without_secret_in_args(monkeypatch: pytest.MonkeyPatch) -> None:
    captured: dict[str, object] = {}

    def fake_run_cli(args, *, stdin=None, timeout=60, redactions=None):
        captured["args"] = args
        captured["stdin"] = stdin
        captured["redactions"] = redactions
        return {
            "ok": True,
            "stdout_tail": "id: 123e4567-e89b-12d3-a456-426614174000\n",
            "stderr_tail": "",
            "error": None,
        }

    monkeypatch.setattr(cli_shim, "run_cli", fake_run_cli)
    secret = "super-secret-value-123"

    result = vault_add_func("github", "nauta-ai", secret, "primary")

    assert result == {"ok": True, "vault_entry_id": "123e4567-e89b-12d3-a456-426614174000", "error": None}
    assert secret not in " ".join(captured["args"])  # type: ignore[arg-type]
    assert captured["redactions"] == [secret]


def test_vault_add_secret_value_not_leaked(monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]) -> None:
    secret = "NEVER_PRINT_THIS_SECRET_VALUE"

    def fake_run_cli(args, *, stdin=None, timeout=60, redactions=None):
        return {
            "ok": True,
            "stdout_tail": "added 123e4567-e89b-12d3-a456-426614174001\n",
            "stderr_tail": "",
            "error": None,
        }

    monkeypatch.setattr(cli_shim, "run_cli", fake_run_cli)
    result = vault_add_func("stripe", "acct", secret, "stripe-live")
    captured = capsys.readouterr()

    assert result["ok"] is True
    assert secret not in captured.out
    assert secret not in captured.err


def test_vault_add_returns_structured_error_on_cli_failure(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(
        cli_shim,
        "run_cli",
        lambda *args, **kwargs: {"ok": False, "stdout_tail": "", "stderr_tail": "", "error": "unlock_failed"},
    )

    result = vault_add_func("github", "acct", "secret", "label")

    assert result["ok"] is False
    assert result["error"] == "unlock_failed"


def test_vault_rotate_without_new_secret_waits_for_user() -> None:
    result = vault_rotate_func("github", "acct", "old-id")

    assert result["ok"] is True
    assert result["new_vault_entry_id"] is None
    assert result["steps_completed"] == ["rotation_playbook_prepared"]
    assert result["steps_waiting_on_user"]


def test_vault_rotate_with_new_secret_adds_and_marks_superseded(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(
        vault_rotate_module,
        "vault_add",
        lambda *args, **kwargs: {"ok": True, "vault_entry_id": "new-id", "error": None},
    )
    monkeypatch.setattr(cli_shim, "mark_superseded", lambda **kwargs: {"ok": True, "error": None})

    result = vault_rotate_func("stripe", "acct", "old-id", new_secret="new-secret")

    assert result["ok"] is True
    assert result["new_vault_entry_id"] == "new-id"
    assert "old_entry_marked_superseded" in result["steps_completed"]


def test_vault_rotate_rejects_non_day2_provider() -> None:
    result = vault_rotate_func("openai", "acct", "old-id")

    assert result["ok"] is False
    assert result["error"].startswith("provider_not_supported_day2")


def test_audit_log_calls_cli(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(
        cli_shim,
        "audit_log",
        lambda **kwargs: {
            "ok": True,
            "events": [{"ts": "2026-05-17T00:00:00Z", "action": "add", "provider": "github", "account": "acct", "ok": True}],
            "error": None,
        },
    )

    result = audit_log(provider="github", account="acct", since_days=30)

    assert result["ok"] is True
    assert result["events"][0]["provider"] == "github"


def test_server_paid_gate_refuses_invalid_license(monkeypatch: pytest.MonkeyPatch) -> None:
    server = importlib.import_module("holster_mcp.server")
    monkeypatch.setattr(server.license_gate, "require_valid_license", lambda key=None: license_module.LicenseStatus(ok=False, error="license_required_or_expired"))

    result = server.tool_vault_add("github", "acct", "secret", "label", license_key="bad")

    assert result["ok"] is False
    assert result["error"] == "license_required_or_expired"


def test_server_paid_gate_allows_valid_license(monkeypatch: pytest.MonkeyPatch) -> None:
    server = importlib.import_module("holster_mcp.server")
    monkeypatch.setattr(server.license_gate, "require_valid_license", lambda key=None: license_module.LicenseStatus(ok=True))
    monkeypatch.setattr(server, "vault_add", lambda **kwargs: {"ok": True, "vault_entry_id": "id", "error": None})

    result = server.tool_vault_add("github", "acct", "secret", "label", license_key=LIVE_KEY)

    assert result["ok"] is True
    assert result["vault_entry_id"] == "id"
