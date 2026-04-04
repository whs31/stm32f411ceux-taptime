import calendar
from datetime import date, datetime

import aiosqlite

from .db import (
    get_overrides_for_month,
    get_records_for_month,
    get_required_hours_override,
)

DEFAULT_REQUIRED_SECONDS = 8 * 3600 + 30 * 60  # 8h 30m


async def required_seconds_for_date(
    db: aiosqlite.Connection, uid: str, d: date
) -> int | None:
    """Return required work seconds for a date, or None if it is a non-workday."""
    override = await get_required_hours_override(db, uid, d.isoformat())
    if override is not None:
        return override
    if d.weekday() >= 5:  # Saturday=5, Sunday=6
        return None
    return DEFAULT_REQUIRED_SECONDS


def seconds_worked(ci: str, co: str) -> int:
    fmt = "%H:%M:%S"
    delta = datetime.strptime(co, fmt) - datetime.strptime(ci, fmt)
    return int(delta.total_seconds())


def format_balance(seconds: int) -> str:
    """Format a signed balance: positive → overtime, negative → undertime."""
    abs_s = abs(seconds)
    h, rem = divmod(abs_s, 3600)
    m, s = divmod(rem, 60)
    sign = "+" if seconds >= 0 else "-"
    label = "overtime" if seconds >= 0 else "undertime"
    return f"{sign}{h}h {m}m {s}s ({label})"


async def month_net_balance(
    db: aiosqlite.Connection, uid: str, year: int, month: int
) -> int:
    """
    Return net balance in seconds for the given month (positive = overtime, negative = undertime).

    - Counts every workday up to and including today.
    - Days with both check_in and check_out: balance = worked - required.
    - Past workdays with no completed session: full required seconds → undertime.
    - Today with no check_out: not counted yet.
    - Weekend/holiday with no override: skipped.
    """
    records = await get_records_for_month(db, uid, year, month)
    overrides = await get_overrides_for_month(db, uid, year, month)

    record_dict: dict[str, tuple[str | None, str | None]] = {
        row[0]: (row[1], row[2]) for row in records
    }
    override_dict: dict[str, int] = {row[0]: row[1] for row in overrides}

    today = date.today()
    num_days = calendar.monthrange(year, month)[1]
    net = 0

    for day_num in range(1, num_days + 1):
        d = date(year, month, day_num)
        if d > today:
            break

        d_str = d.isoformat()

        if d_str in override_dict:
            req = override_dict[d_str]
        elif d.weekday() >= 5:
            continue
        else:
            req = DEFAULT_REQUIRED_SECONDS

        ci, co = record_dict.get(d_str, (None, None))

        if ci and co:
            net += seconds_worked(ci, co) - req
        elif d < today:
            net -= req
        # today without check_out: skip

    return net
