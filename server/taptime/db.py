from datetime import date, datetime

import aiosqlite


async def init_db(db: aiosqlite.Connection) -> None:
    await db.executescript("""
        CREATE TABLE IF NOT EXISTS users (
            telegram_id INTEGER PRIMARY KEY,
            name        TEXT NOT NULL,
            uid         TEXT NOT NULL UNIQUE
        );
        CREATE TABLE IF NOT EXISTS records (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            uid         TEXT NOT NULL,
            date        TEXT NOT NULL,
            check_in    TEXT,
            check_out   TEXT,
            UNIQUE(uid, date)
        );
        CREATE TABLE IF NOT EXISTS work_hour_overrides (
            uid              TEXT NOT NULL,
            date             TEXT NOT NULL,
            required_seconds INTEGER NOT NULL,
            PRIMARY KEY (uid, date)
        );
    """)
    await db.commit()


async def get_user_by_telegram(db: aiosqlite.Connection, telegram_id: int):
    async with db.execute(
        "SELECT telegram_id, name, uid FROM users WHERE telegram_id = ?", (telegram_id,)
    ) as cur:
        return await cur.fetchone()


async def get_user_by_uid(db: aiosqlite.Connection, uid: str):
    async with db.execute(
        "SELECT telegram_id, name, uid FROM users WHERE uid = ?", (uid,)
    ) as cur:
        return await cur.fetchone()


async def register_user(db: aiosqlite.Connection, telegram_id: int, name: str, uid: str) -> None:
    await db.execute(
        "INSERT INTO users (telegram_id, name, uid) VALUES (?, ?, ?)",
        (telegram_id, name, uid),
    )
    await db.commit()


async def get_records(db: aiosqlite.Connection, uid: str, since: date):
    async with db.execute(
        "SELECT date, check_in, check_out FROM records "
        "WHERE uid = ? AND date >= ? ORDER BY date DESC",
        (uid, since.isoformat()),
    ) as cur:
        return await cur.fetchall()


async def upsert_check_in(db: aiosqlite.Connection, uid: str, dt: datetime) -> None:
    await db.execute(
        """INSERT INTO records (uid, date, check_in) VALUES (?, ?, ?)
           ON CONFLICT(uid, date) DO UPDATE SET check_in = excluded.check_in""",
        (uid, dt.date().isoformat(), dt.strftime("%H:%M:%S")),
    )
    await db.commit()


async def upsert_check_out(db: aiosqlite.Connection, uid: str, dt: datetime) -> str | None:
    d = dt.date().isoformat()
    await db.execute(
        """INSERT INTO records (uid, date, check_out) VALUES (?, ?, ?)
           ON CONFLICT(uid, date) DO UPDATE SET check_out = excluded.check_out""",
        (uid, d, dt.strftime("%H:%M:%S")),
    )
    await db.commit()
    async with db.execute(
        "SELECT check_in FROM records WHERE uid = ? AND date = ?", (uid, d)
    ) as cur:
        row = await cur.fetchone()
    return row[0] if row else None


async def set_record(
    db: aiosqlite.Connection, uid: str, d: str, check_in: str, check_out: str
) -> None:
    await db.execute(
        """INSERT INTO records (uid, date, check_in, check_out) VALUES (?, ?, ?, ?)
           ON CONFLICT(uid, date) DO UPDATE
               SET check_in = excluded.check_in, check_out = excluded.check_out""",
        (uid, d, check_in, check_out),
    )
    await db.commit()


async def today_record(db: aiosqlite.Connection, uid: str):
    async with db.execute(
        "SELECT date, check_in, check_out FROM records WHERE uid = ? AND date = ?",
        (uid, date.today().isoformat()),
    ) as cur:
        return await cur.fetchone()


async def reopen_checkin(db: aiosqlite.Connection, uid: str) -> None:
    """Clear check_out so the user is marked checked-in again (preserves original check_in)."""
    await db.execute(
        "UPDATE records SET check_out = NULL WHERE uid = ? AND date = ?",
        (uid, date.today().isoformat()),
    )
    await db.commit()


async def delete_record(db: aiosqlite.Connection, uid: str, d: str) -> bool:
    cursor = await db.execute(
        "DELETE FROM records WHERE uid = ? AND date = ?", (uid, d)
    )
    await db.commit()
    return cursor.rowcount > 0


async def set_required_hours_override(
    db: aiosqlite.Connection, uid: str, d: str, required_seconds: int
) -> None:
    await db.execute(
        """INSERT INTO work_hour_overrides (uid, date, required_seconds) VALUES (?, ?, ?)
           ON CONFLICT(uid, date) DO UPDATE SET required_seconds = excluded.required_seconds""",
        (uid, d, required_seconds),
    )
    await db.commit()


async def get_required_hours_override(
    db: aiosqlite.Connection, uid: str, d: str
) -> int | None:
    async with db.execute(
        "SELECT required_seconds FROM work_hour_overrides WHERE uid = ? AND date = ?",
        (uid, d),
    ) as cur:
        row = await cur.fetchone()
    return row[0] if row else None


async def get_records_for_month(
    db: aiosqlite.Connection, uid: str, year: int, month: int
):
    month_start = f"{year:04d}-{month:02d}-01"
    next_year, next_month = (year + 1, 1) if month == 12 else (year, month + 1)
    month_end = f"{next_year:04d}-{next_month:02d}-01"
    async with db.execute(
        "SELECT date, check_in, check_out FROM records "
        "WHERE uid = ? AND date >= ? AND date < ?",
        (uid, month_start, month_end),
    ) as cur:
        return await cur.fetchall()


async def get_overrides_for_month(
    db: aiosqlite.Connection, uid: str, year: int, month: int
):
    month_start = f"{year:04d}-{month:02d}-01"
    next_year, next_month = (year + 1, 1) if month == 12 else (year, month + 1)
    month_end = f"{next_year:04d}-{next_month:02d}-01"
    async with db.execute(
        "SELECT date, required_seconds FROM work_hour_overrides "
        "WHERE uid = ? AND date >= ? AND date < ?",
        (uid, month_start, month_end),
    ) as cur:
        return await cur.fetchall()
