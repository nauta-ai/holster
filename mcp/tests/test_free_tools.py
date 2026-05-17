from __future__ import annotations

import importlib
from pathlib import Path

import pytest

check_gitignore_module = importlib.import_module("holster_mcp.free.check_gitignore")
rotation_playbook_module = importlib.import_module("holster_mcp.free.rotation_playbook")
scan_repo_module = importlib.import_module("holster_mcp.free.scan_repo")


def test_scan_repo_maps_doctor_findings(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    calls: list[list[str]] = []

    def fake_run(args: list[str], timeout: int = 60) -> dict:
        calls.append(args)
        return {
            "ok": True,
            "scanned_files": 2,
            "findings": [
                {
                    "file": "main.py",
                    "line": 3,
                    "kind": "openai_api_key",
                    "severity": "critical",
                    "suggestion": "rotate",
                }
            ],
        }

    monkeypatch.setattr(scan_repo_module, "run_doctor", fake_run)
    result = scan_repo_module.scan_repo(str(tmp_path), depth=2)

    assert calls == [["scan", str(tmp_path.resolve()), "--depth", "2", "--json"]]
    assert result["ok"] is True
    assert result["scanned_files"] == 2
    assert result["findings"][0] == {
        "file": "main.py",
        "line": 3,
        "secret_kind": "openai_api_key",
        "severity": "error",
        "suggestion": "rotate",
    }


def test_scan_repo_rejects_relative_path() -> None:
    with pytest.raises(ValueError, match="absolute"):
        scan_repo_module.scan_repo("relative/path")


def test_scan_repo_rejects_too_many_files(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    monkeypatch.setattr(
        scan_repo_module,
        "run_doctor",
        lambda args, timeout=60: {"ok": True, "scanned_files": 1001, "findings": []},
    )
    with pytest.raises(ValueError, match="1000"):
        scan_repo_module.scan_repo(str(tmp_path))


def test_check_gitignore_returns_contract(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    def fake_run(args: list[str], timeout: int = 60) -> dict:
        return {
            "ok": True,
            "missing_patterns": [".env.local"],
            "existing_safe": [".env"],
            "existing_unsafe": [],
            "suggested_append": ".env.local",
        }

    monkeypatch.setattr(check_gitignore_module, "run_doctor", fake_run)
    result = check_gitignore_module.check_gitignore(str(tmp_path))

    assert result == {
        "ok": True,
        "missing_patterns": [".env.local"],
        "existing_safe": [".env"],
        "existing_unsafe": [],
        "suggested_append": ".env.local",
    }


def test_rotation_playbook_supports_all_day1_providers() -> None:
    providers = {
        "github",
        "gitlab",
        "stripe",
        "aws",
        "gcp",
        "openai",
        "anthropic",
        "gemini",
        "brave",
        "pinterest",
    }
    assert set(rotation_playbook_module.PLAYBOOKS) == providers
    for provider in providers:
        result = rotation_playbook_module.rotation_playbook(provider)
        assert result["ok"] is True
        assert result["provider"] == provider
        assert len(result["steps"]) >= 5
        assert result["estimated_minutes"] > 0
        assert any("Reference docs:" in warning for warning in result["warnings"])


def test_rotation_playbook_rejects_unknown_provider() -> None:
    with pytest.raises(ValueError, match="unsupported provider"):
        rotation_playbook_module.rotation_playbook("not-a-provider")
