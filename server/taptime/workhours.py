import calendar
from dataclasses import dataclass
from datetime import date, datetime

import aiosqlite

from .db import (
    get_day_offs_for_month,
    get_non_remote_day_overrides_for_month,
    get_overrides_for_month,
    get_records_for_month,
    get_remote_day_overrides_for_month,
    get_remote_workdays,
    get_required_hours_override,
    get_user_required_seconds,
)

DEFAULT_REQUIRED_SECONDS = 8 * 3600 + 30 * 60  # 8h 30m

WEEKDAY_ABBR: list[str] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
WEEKDAY_FROM_ABBR: dict[str, int] = {v.upper(): i for i, v in enumerate(WEEKDAY_ABBR)}


@dataclass
class DayRow:
    d: date
    weekday_abbr: str
    is_day_off: bool = False
    is_remote: bool = False      # remote weekday — balance is always 0
    is_weekend: bool = False     # natural weekend without an override
    check_in: str | None = None
    check_out: str | None = None
    required_seconds: int = 0
    balance_seconds: int | None = None  # None = not yet countable (today in-progress, or untracked weekend)


async def user_default_seconds(db: aiosqlite.Connection, uid: str) -> int:
    """Return the user's per-day required work seconds, falling back to the global default."""
    val = await get_user_required_seconds(db, uid)
    return val if val is not None else DEFAULT_REQUIRED_SECONDS


async def required_seconds_for_date(
    db: aiosqlite.Connection, uid: str, d: date
) -> int | None:
    """Return required work seconds for a date, or None if it is a non-workday."""
    override = await get_required_hours_override(db, uid, d.isoformat())
    if override is not None:
        return override
    if d.weekday() >= 5:  # Saturday=5, Sunday=6
        return None
    return await user_default_seconds(db, uid)


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


def format_delta(seconds: int) -> str:
    """Short signed delta for per-day or per-month display."""
    if seconds == 0:
        return "±0"
    abs_s = abs(seconds)
    h, rem = divmod(abs_s, 3600)
    m, s = divmod(rem, 60)
    sign = "+" if seconds > 0 else "−"
    if h > 0:
        return f"{sign}{h}h {m:02d}m"
    return f"{sign}{m}m {s:02d}s"


async def month_rows(
    db: aiosqlite.Connection, uid: str, year: int, month: int
) -> list[DayRow]:
    """
    Per-day rows up to and including today.

    - Workdays (Mon–Fri): always included.
    - Remote weekdays: included, balance forced to 0 regardless of records.
    - Day-offs: included, balance forced to 0.
    - Weekends with an override: included, treated as workdays.
    - Weekends without override: included only when a record exists.
    """
    records = await get_records_for_month(db, uid, year, month)
    overrides = await get_overrides_for_month(db, uid, year, month)
    remote_wdays = set(await get_remote_workdays(db, uid))
    remote_day_dates = set(await get_remote_day_overrides_for_month(db, uid, year, month))
    non_remote_day_dates = set(await get_non_remote_day_overrides_for_month(db, uid, year, month))
    day_off_dates = set(await get_day_offs_for_month(db, uid, year, month))
    user_req = await user_default_seconds(db, uid)

    record_dict: dict[str, tuple[str | None, str | None]] = {
        row[0]: (row[1], row[2]) for row in records
    }
    override_dict: dict[str, int] = {row[0]: row[1] for row in overrides}

    today = date.today()
    num_days = calendar.monthrange(year, month)[1]
    rows: list[DayRow] = []

    for day_num in range(1, num_days + 1):
        d = date(year, month, day_num)
        if d > today:
            break

        d_str = d.isoformat()
        wd = d.weekday()
        ci, co = record_dict.get(d_str, (None, None))

        if d_str in day_off_dates:
            rows.append(DayRow(
                d=d, weekday_abbr=WEEKDAY_ABBR[wd], is_day_off=True,
                check_in=ci, check_out=co, balance_seconds=0,
            ))
            continue

        if d_str in override_dict:
            req = override_dict[d_str]
            is_remote = (wd in remote_wdays or d_str in remote_day_dates) and d_str not in non_remote_day_dates
            if is_remote:
                bal: int | None = 0
            elif ci and co:
                bal = seconds_worked(ci, co) - req
            elif d < today:
                bal = -req
            else:
                bal = None
            rows.append(DayRow(
                d=d, weekday_abbr=WEEKDAY_ABBR[wd], is_remote=is_remote,
                check_in=ci, check_out=co, required_seconds=req, balance_seconds=bal,
            ))
            continue

        if wd >= 5:
            # Natural weekend — treated as remote if override set, otherwise only include if there's a record
            if d_str in remote_day_dates and d_str not in non_remote_day_dates:
                rows.append(DayRow(
                    d=d, weekday_abbr=WEEKDAY_ABBR[wd], is_remote=True,
                    check_in=ci, check_out=co, required_seconds=0, balance_seconds=0,
                ))
            elif ci or co:
                rows.append(DayRow(
                    d=d, weekday_abbr=WEEKDAY_ABBR[wd], is_weekend=True,
                    check_in=ci, check_out=co, balance_seconds=None,
                ))
            continue

        # Regular weekday (Mon–Fri)
        req = user_req
        is_remote = (wd in remote_wdays or d_str in remote_day_dates) and d_str not in non_remote_day_dates
        if is_remote:
            bal = 0
        elif ci and co:
            bal = seconds_worked(ci, co) - req
        elif d < today:
            bal = -req
        else:
            bal = None  # today, still in progress or no check-in yet
        rows.append(DayRow(
            d=d, weekday_abbr=WEEKDAY_ABBR[wd], is_remote=is_remote,
            check_in=ci, check_out=co, required_seconds=req, balance_seconds=bal,
        ))

    return rows


async def month_net_balance(
    db: aiosqlite.Connection, uid: str, year: int, month: int
) -> int:
    """
    Return net balance in seconds for the given month (positive = overtime, negative = undertime).
    """
    rows = await month_rows(db, uid, year, month)
    return sum(r.balance_seconds for r in rows if r.balance_seconds is not None)
