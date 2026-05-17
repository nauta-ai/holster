from __future__ import annotations

import base64
import os
import secrets
import sqlite3
import threading
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import stripe

from holster_mcp.util.config import holster_home, resolve_license_key

LIVE_PREFIX = "holster_live_"
TRIAL_PREFIX = "holster_trial_"
TRIAL_SECONDS = 7 * 24 * 60 * 60
GRACE_SECONDS = 7 * 24 * 60 * 60
REFRESH_SECONDS = 10 * 60
TRIAL_URL = "https://nautaai.com/holster"
_poller_started = False


@dataclass(frozen=True)
class LicenseStatus:
    ok: bool
    license_key: str | None = None
    customer_id: str | None = None
    warning: str | None = None
    error: str | None = None
    trial_url: str = TRIAL_URL

    def as_error(self) -> dict[str, Any]:
        return {"ok": False, "error": self.error or "license_required_or_expired", "trial_url": self.trial_url}


def license_db_path() -> Path:
    return Path(os.environ.get("HOLSTER_LICENSE_DB", holster_home() / "licenses.db")).expanduser()


def ensure_schema(path: Path | None = None) -> Path:
    db_path = path or license_db_path()
    db_path.parent.mkdir(parents=True, exist_ok=True)
    with sqlite3.connect(db_path) as db:
        db.execute(
            """
            CREATE TABLE IF NOT EXISTS licenses (
                license_key TEXT PRIMARY KEY,
                customer_id TEXT,
                valid_until INTEGER,
                last_checked INTEGER,
                trial INTEGER
            )
            """
        )
        db.commit()
    return db_path


def is_valid(license_key: str | None = None, *, now: int | None = None, create_trial: bool = True) -> LicenseStatus:
    now_ts = int(now or time.time())
    key = license_key or resolve_license_key()
    if not key and create_trial:
        key = get_or_create_trial(now=now_ts)
    if not key:
        return LicenseStatus(ok=False, error="license_required_or_expired")
    if not _valid_format(key):
        return LicenseStatus(ok=False, license_key=key, error="license_format_invalid")

    row = _get_row(key)
    if key.startswith(TRIAL_PREFIX):
        if row is None and create_trial:
            row = _insert_license(key, customer_id="", valid_until=now_ts + TRIAL_SECONDS, last_checked=now_ts, trial=True)
        if row and now_ts <= int(row["valid_until"]):
            return LicenseStatus(ok=True, license_key=key, warning="trial_active")
        return LicenseStatus(ok=False, license_key=key, error="license_required_or_expired")

    if row and _within_valid_or_grace(row, now_ts):
        warning = "license_grace_period" if now_ts > int(row["valid_until"]) else None
        if now_ts - int(row["last_checked"]) >= REFRESH_SECONDS:
            start_background_poll()
        return LicenseStatus(ok=True, license_key=key, customer_id=row["customer_id"], warning=warning)

    refreshed = refresh_live_license(key, cached_row=row, now=now_ts)
    if refreshed.ok:
        return refreshed
    if row and _within_valid_or_grace(row, now_ts):
        return LicenseStatus(ok=True, license_key=key, customer_id=row["customer_id"], warning="license_grace_period")
    return refreshed


def require_valid_license(license_key: str | None = None) -> LicenseStatus:
    status = is_valid(license_key)
    if status.ok:
        start_background_poll()
    return status


def get_or_create_trial(*, now: int | None = None) -> str:
    now_ts = int(now or time.time())
    ensure_schema()
    with sqlite3.connect(license_db_path()) as db:
        db.row_factory = sqlite3.Row
        row = db.execute("SELECT license_key FROM licenses WHERE trial = 1 ORDER BY valid_until DESC LIMIT 1").fetchone()
        if row:
            return str(row["license_key"])
    key = TRIAL_PREFIX + _base32_token()
    _insert_license(key, customer_id="", valid_until=now_ts + TRIAL_SECONDS, last_checked=now_ts, trial=True)
    return key


def refresh_live_license(
    license_key: str,
    *,
    cached_row: dict[str, Any] | None = None,
    now: int | None = None,
) -> LicenseStatus:
    now_ts = int(now or time.time())
    customer_id = (
        (cached_row or {}).get("customer_id")
        or os.environ.get("HOLSTER_STRIPE_CUSTOMER_ID")
        or ""
    )
    if not customer_id:
        return LicenseStatus(ok=False, license_key=license_key, error="license_not_cached")
    if not os.environ.get("STRIPE_API_KEY"):
        return LicenseStatus(ok=False, license_key=license_key, customer_id=customer_id, error="stripe_api_key_missing")

    stripe.api_key = os.environ["STRIPE_API_KEY"]
    try:
        customer = stripe.Customer.retrieve(customer_id)
        subscriptions = stripe.Subscription.list(customer=customer_id, status="active", limit=1)
    except Exception as exc:  # noqa: BLE001 - SDK surfaces many typed subclasses.
        return LicenseStatus(
            ok=False,
            license_key=license_key,
            customer_id=customer_id,
            error=f"stripe_refresh_failed: {type(exc).__name__}",
        )

    if not _customer_matches_license(customer, license_key):
        return LicenseStatus(ok=False, license_key=license_key, customer_id=customer_id, error="license_customer_mismatch")
    if not getattr(subscriptions, "data", []):
        return LicenseStatus(ok=False, license_key=license_key, customer_id=customer_id, error="subscription_inactive")

    valid_until = now_ts + REFRESH_SECONDS
    _insert_license(license_key, customer_id=customer_id, valid_until=valid_until, last_checked=now_ts, trial=False)
    return LicenseStatus(ok=True, license_key=license_key, customer_id=customer_id)


def seed_license(
    license_key: str,
    *,
    customer_id: str = "",
    valid_until: int,
    last_checked: int,
    trial: bool,
) -> None:
    _insert_license(license_key, customer_id=customer_id, valid_until=valid_until, last_checked=last_checked, trial=trial)


def start_background_poll() -> None:
    global _poller_started
    if _poller_started:
        return
    _poller_started = True
    thread = threading.Thread(target=_poll_forever, name="holster-license-poller", daemon=True)
    thread.start()


def _poll_forever() -> None:
    while True:
        time.sleep(REFRESH_SECONDS)
        for row in _live_rows():
            refresh_live_license(str(row["license_key"]), cached_row=dict(row))


def _get_row(license_key: str) -> dict[str, Any] | None:
    ensure_schema()
    with sqlite3.connect(license_db_path()) as db:
        db.row_factory = sqlite3.Row
        row = db.execute("SELECT * FROM licenses WHERE license_key = ?", (license_key,)).fetchone()
        return dict(row) if row else None


def _live_rows() -> list[dict[str, Any]]:
    ensure_schema()
    with sqlite3.connect(license_db_path()) as db:
        db.row_factory = sqlite3.Row
        rows = db.execute("SELECT * FROM licenses WHERE trial = 0").fetchall()
        return [dict(row) for row in rows]


def _insert_license(
    license_key: str,
    *,
    customer_id: str,
    valid_until: int,
    last_checked: int,
    trial: bool,
) -> dict[str, Any]:
    ensure_schema()
    with sqlite3.connect(license_db_path()) as db:
        db.execute(
            """
            INSERT INTO licenses (license_key, customer_id, valid_until, last_checked, trial)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(license_key) DO UPDATE SET
                customer_id = excluded.customer_id,
                valid_until = excluded.valid_until,
                last_checked = excluded.last_checked,
                trial = excluded.trial
            """,
            (license_key, customer_id, int(valid_until), int(last_checked), 1 if trial else 0),
        )
        db.commit()
    return {
        "license_key": license_key,
        "customer_id": customer_id,
        "valid_until": int(valid_until),
        "last_checked": int(last_checked),
        "trial": 1 if trial else 0,
    }


def _within_valid_or_grace(row: dict[str, Any], now_ts: int) -> bool:
    return now_ts <= int(row["valid_until"]) + GRACE_SECONDS


def _customer_matches_license(customer: Any, license_key: str) -> bool:
    metadata = getattr(customer, "metadata", None)
    if isinstance(metadata, dict) and metadata.get("holster_license_key"):
        return metadata["holster_license_key"] == license_key
    return True


def _valid_format(license_key: str) -> bool:
    if license_key.startswith(LIVE_PREFIX):
        token = license_key[len(LIVE_PREFIX) :]
    elif license_key.startswith(TRIAL_PREFIX):
        token = license_key[len(TRIAL_PREFIX) :]
    else:
        return False
    return len(token) == 24 and all(ch in "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567" for ch in token)


def _base32_token() -> str:
    return base64.b32encode(secrets.token_bytes(15)).decode("ascii").rstrip("=")[:24]
