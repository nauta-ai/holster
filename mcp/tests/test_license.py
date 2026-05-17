from __future__ import annotations

from types import SimpleNamespace

import pytest

from holster_mcp.paid import license as license_module


NOW = 1_800_000_000
LIVE_KEY = "holster_live_ABCDEFGHIJKLMNOPQRSTUVWX"
TRIAL_KEY = "holster_trial_ABCDEFGHIJKLMNOPQRSTUVWX"


@pytest.fixture(autouse=True)
def isolated_license_db(monkeypatch: pytest.MonkeyPatch, tmp_path):
    monkeypatch.setenv("HOLSTER_LICENSE_DB", str(tmp_path / "licenses.db"))
    monkeypatch.delenv("HOLSTER_LICENSE_KEY", raising=False)
    monkeypatch.delenv("STRIPE_API_KEY", raising=False)
    monkeypatch.delenv("HOLSTER_STRIPE_CUSTOMER_ID", raising=False)
    license_module._poller_started = False


def test_valid_live_license_with_active_subscription(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("STRIPE_API_KEY", "sk_test_fake")
    monkeypatch.setenv("HOLSTER_STRIPE_CUSTOMER_ID", "cus_123")
    monkeypatch.setattr(
        license_module.stripe.Customer,
        "retrieve",
        lambda customer_id: SimpleNamespace(id=customer_id, metadata={"holster_license_key": LIVE_KEY}),
    )
    monkeypatch.setattr(
        license_module.stripe.Subscription,
        "list",
        lambda **kwargs: SimpleNamespace(data=[SimpleNamespace(id="sub_123", status="active")]),
    )

    status = license_module.is_valid(LIVE_KEY, now=NOW)

    assert status.ok is True
    assert status.customer_id == "cus_123"


def test_expired_license_inside_grace_runs_with_warning() -> None:
    license_module.seed_license(
        LIVE_KEY,
        customer_id="cus_123",
        valid_until=NOW - 60,
        last_checked=NOW - 60,
        trial=False,
    )

    status = license_module.is_valid(LIVE_KEY, now=NOW, create_trial=False)

    assert status.ok is True
    assert status.warning == "license_grace_period"


def test_expired_license_past_grace_refused() -> None:
    license_module.seed_license(
        LIVE_KEY,
        customer_id="cus_123",
        valid_until=NOW - license_module.GRACE_SECONDS - 1,
        last_checked=NOW - license_module.GRACE_SECONDS - 1,
        trial=False,
    )

    status = license_module.is_valid(LIVE_KEY, now=NOW, create_trial=False)

    assert status.ok is False
    assert status.error in {"stripe_api_key_missing", "license_required_or_expired"}


def test_trial_within_seven_days_runs() -> None:
    license_module.seed_license(
        TRIAL_KEY,
        valid_until=NOW + 10,
        last_checked=NOW - 10,
        trial=True,
    )

    status = license_module.is_valid(TRIAL_KEY, now=NOW, create_trial=False)

    assert status.ok is True
    assert status.warning == "trial_active"


def test_trial_past_seven_days_refused() -> None:
    license_module.seed_license(
        TRIAL_KEY,
        valid_until=NOW - 1,
        last_checked=NOW - license_module.TRIAL_SECONDS,
        trial=True,
    )

    status = license_module.is_valid(TRIAL_KEY, now=NOW, create_trial=False)

    assert status.ok is False
    assert status.error == "license_required_or_expired"


def test_missing_license_auto_creates_trial() -> None:
    status = license_module.is_valid(None, now=NOW)

    assert status.ok is True
    assert status.license_key is not None
    assert status.license_key.startswith("holster_trial_")
