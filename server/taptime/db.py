from datetime import date, datetime
from pathlib import Path

import aiosqlite

_SQL = Path(__file__).parent / "sql"


def _q(path: str) -> str:
    return (_SQL / path).read_text()


async def init_db(db: aiosqlite.Connection) -> None:
    await db.executescript(_q("schema.sql"))
    await db.commit()


async def get_user_by_telegram(db: aiosqlite.Connection, telegram_id: int):
    async with db.execute(_q("users/get_by_telegram.sql"), (telegram_id,)) as cur:
        return await cur.fetchone()


async def get_user_by_uid(db: aiosqlite.Connection, uid: str):
    async with db.execute(_q("users/get_by_uid.sql"), (uid,)) as cur:
        return await cur.fetchone()


async def register_user(db: aiosqlite.Connection, telegram_id: int, name: str, uid: str) -> None:
    await db.execute(_q("users/insert.sql"), (telegram_id, name, uid))
    await db.commit()


async def delete_user(db: aiosqlite.Connection, telegram_id: int) -> bool:
    cursor = await db.execute(_q("users/delete.sql"), (telegram_id,))
    await db.commit()
    return cursor.rowcount > 0


async def get_records(db: aiosqlite.Connection, uid: str, since: date):
    async with db.execute(_q("records/get_since.sql"), (uid, since.isoformat())) as cur:
        return await cur.fetchall()


async def upsert_check_in(db: aiosqlite.Connection, uid: str, dt: datetime) -> None:
    await db.execute(
        _q("records/upsert_check_in.sql"),
        (uid, dt.date().isoformat(), dt.strftime("%H:%M:%S")),
    )
    await db.commit()


async def upsert_check_out(db: aiosqlite.Connection, uid: str, dt: datetime) -> str | None:
    d = dt.date().isoformat()
    await db.execute(
        _q("records/upsert_check_out.sql"),
        (uid, d, dt.strftime("%H:%M:%S")),
    )
    await db.commit()
    async with db.execute(_q("records/get_check_in.sql"), (uid, d)) as cur:
        row = await cur.fetchone()
    return row[0] if row else None


async def set_record(
    db: aiosqlite.Connection, uid: str, d: str, check_in: str, check_out: str
) -> None:
    await db.execute(_q("records/set.sql"), (uid, d, check_in, check_out))
    await db.commit()


async def today_record(db: aiosqlite.Connection, uid: str):
    async with db.execute(
        _q("records/get_today.sql"), (uid, date.today().isoformat())
    ) as cur:
        return await cur.fetchone()


async def reopen_checkin(db: aiosqlite.Connection, uid: str) -> None:
    """Clear check_out so the user is marked checked-in again (preserves original check_in)."""
    await db.execute(_q("records/reopen.sql"), (uid, date.today().isoformat()))
    await db.commit()


async def delete_record(db: aiosqlite.Connection, uid: str, d: str) -> bool:
    cursor = await db.execute(_q("records/delete.sql"), (uid, d))
    await db.commit()
    return cursor.rowcount > 0


async def set_required_hours_override(
    db: aiosqlite.Connection, uid: str, d: str, required_seconds: int
) -> None:
    await db.execute(_q("overrides/set.sql"), (uid, d, required_seconds))
    await db.commit()


async def get_required_hours_override(
    db: aiosqlite.Connection, uid: str, d: str
) -> int | None:
    async with db.execute(_q("overrides/get.sql"), (uid, d)) as cur:
        row = await cur.fetchone()
    return row[0] if row else None


async def get_records_for_month(
    db: aiosqlite.Connection, uid: str, year: int, month: int
):
    month_start = f"{year:04d}-{month:02d}-01"
    next_year, next_month = (year + 1, 1) if month == 12 else (year, month + 1)
    month_end = f"{next_year:04d}-{next_month:02d}-01"
    async with db.execute(
        _q("records/get_for_month.sql"), (uid, month_start, month_end)
    ) as cur:
        return await cur.fetchall()


async def get_overrides_for_month(
    db: aiosqlite.Connection, uid: str, year: int, month: int
):
    month_start = f"{year:04d}-{month:02d}-01"
    next_year, next_month = (year + 1, 1) if month == 12 else (year, month + 1)
    month_end = f"{next_year:04d}-{next_month:02d}-01"
    async with db.execute(
        _q("overrides/get_for_month.sql"), (uid, month_start, month_end)
    ) as cur:
        return await cur.fetchall()


async def set_remote_workdays(db: aiosqlite.Connection, uid: str, weekdays: list[int]) -> None:
    await db.execute(_q("remote_workdays/delete_all.sql"), (uid,))
    for wd in weekdays:
        await db.execute(_q("remote_workdays/insert.sql"), (uid, wd))
    await db.commit()


async def get_remote_workdays(db: aiosqlite.Connection, uid: str) -> list[int]:
    async with db.execute(_q("remote_workdays/get_all.sql"), (uid,)) as cur:
        rows = await cur.fetchall()
    return [r[0] for r in rows]


async def add_day_off(db: aiosqlite.Connection, uid: str, d: str) -> None:
    await db.execute(_q("day_offs/add.sql"), (uid, d))
    await db.commit()


async def remove_day_off(db: aiosqlite.Connection, uid: str, d: str) -> None:
    await db.execute(_q("day_offs/delete.sql"), (uid, d))
    await db.commit()


async def get_day_offs_for_month(
    db: aiosqlite.Connection, uid: str, year: int, month: int
) -> list[str]:
    month_start = f"{year:04d}-{month:02d}-01"
    next_year, next_month = (year + 1, 1) if month == 12 else (year, month + 1)
    month_end = f"{next_year:04d}-{next_month:02d}-01"
    async with db.execute(
        _q("day_offs/get_for_month.sql"), (uid, month_start, month_end)
    ) as cur:
        rows = await cur.fetchall()
    return [r[0] for r in rows]


async def set_user_required_seconds(db: aiosqlite.Connection, uid: str, seconds: int) -> None:
    await db.execute(_q("user_settings/set.sql"), (uid, seconds))
    await db.commit()


async def get_user_required_seconds(db: aiosqlite.Connection, uid: str) -> int | None:
    async with db.execute(_q("user_settings/get.sql"), (uid,)) as cur:
        row = await cur.fetchone()
    return row[0] if row else None
