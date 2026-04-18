import logging
import random
from datetime import date, datetime

import aiosqlite
from aiohttp import web
from telegram.ext import Application

from .bot import format_duration
from .db import get_user_by_uid, reopen_checkin, today_record, upsert_check_in, upsert_check_out
from .workhours import format_balance, month_net_balance, required_seconds_for_date, seconds_worked

log = logging.getLogger(__name__)

EARLY_CHECKIN_CUTOFF = "10:00:00"
LATE_CHECKOUT_CUTOFF = "20:00:00"

_EARLY_CHECKIN_MESSAGES = [
    "Great timing — an early start sets the tone for the day!",
    "Nice, you're off to a flying start. Keep it up!",
    "Impressive start! Today's off to a great beginning.",
]

_LATE_CHECKOUT_MESSAGES = [
    "You worked well today. Time to rest!",
    "That's some serious dedication. Well done!",
    "Great work, you went the extra mile today.",
    "Impressive push today. Enjoy your evening!",
]


def _parse_time(ts: str) -> datetime | None:
    try:
        return datetime.fromisoformat(ts)
    except ValueError:
        pass
    try:
        return datetime.combine(date.today(), datetime.strptime(ts, "%H:%M:%S").time())
    except ValueError:
        return None


async def _notify_checkin(tg_app: Application, telegram_id: int, name: str, ci: str) -> None:
    text = f"Hello, {name}! Checked in at {ci}."
    if ci < EARLY_CHECKIN_CUTOFF:
        text += f"\n{random.choice(_EARLY_CHECKIN_MESSAGES)}"
    try:
        await tg_app.bot.send_message(chat_id=telegram_id, text=text)
    except Exception as exc:
        log.warning("Failed to send check-in notification to %d: %s", telegram_id, exc)


async def _notify_checkout(
    tg_app: Application,
    db: aiosqlite.Connection,
    telegram_id: int,
    uid: str,
    name: str,
    ci: str,
    co: str,
) -> None:
    today = date.today()
    req = await required_seconds_for_date(db, uid, today)

    if req is not None:
        worked = seconds_worked(ci, co)
        day_bal = format_balance(worked - req)
    else:
        day_bal = "non-workday"

    month_bal = await month_net_balance(db, uid, today.year, today.month)
    month_str = format_balance(month_bal)
    month_name = today.strftime("%B")

    text = (
        f"Checked out at {co}\n"
        f"Today: {day_bal}\n"
        f"{month_name}: {month_str}"
    )
    if co >= LATE_CHECKOUT_CUTOFF:
        text += f"\n{random.choice(_LATE_CHECKOUT_MESSAGES)}"

    try:
        await tg_app.bot.send_message(chat_id=telegram_id, text=text)
    except Exception as exc:
        log.warning("Failed to send check-out notification to %d: %s", telegram_id, exc)


async def handle_tap(request: web.Request) -> web.Response:
    db: aiosqlite.Connection = request.app["db"]
    secret: str = request.app["mcu_secret"]
    tg_app: Application = request.app["tg_app"]

    if secret and request.headers.get("X-Secret", "") != secret:
        return web.Response(status=401, text="Unauthorized")

    try:
        data = await request.json()
        uid: str = data["uid"]
        ts: str = data["time"]
    except (ValueError, KeyError):
        return web.Response(status=400, text='Bad request: need {"uid", "time"}')

    dt = _parse_time(ts)
    if dt is None:
        return web.Response(status=400, text="Bad time format")

    user = await get_user_by_uid(db, uid)
    if not user:
        return web.json_response({"status": "unknown_uid", "uid": uid})

    telegram_id, name, _ = user
    record = await today_record(db, uid)

    if record and record[1] and not record[2]:
        # Currently checked in → check out
        ci_str = await upsert_check_out(db, uid, dt)
        co_str = dt.strftime("%H:%M:%S")
        dur = format_duration(ci_str, co_str) if ci_str else "—"
        log.info("Check-out: %s at %s (total since first check-in: %s)", name, dt, dur)
        await _notify_checkout(tg_app, db, telegram_id, uid, name, ci_str, co_str)
        return web.json_response(
            {"status": "check_out", "name": name, "duration": dur, "check_in": ci_str, "check_out": co_str}
        )
    elif record and record[1] and record[2]:
        # Was checked out earlier today → re-check-in
        await reopen_checkin(db, uid)
        log.info("Re-check-in: %s at %s", name, dt)
        await _notify_checkin(tg_app, telegram_id, name, record[1])
        return web.json_response({"status": "check_in", "name": name, "check_in": record[1]})
    else:
        # No record yet → first check-in of the day
        await upsert_check_in(db, uid, dt)
        ci_str = dt.strftime("%H:%M:%S")
        log.info("Check-in: %s at %s", name, dt)
        await _notify_checkin(tg_app, telegram_id, name, ci_str)
        return web.json_response({"status": "check_in", "name": name, "check_in": ci_str})


def create_mcu_app(db: aiosqlite.Connection, mcu_secret: str, tg_app: Application) -> web.Application:
    app = web.Application()
    app["db"] = db
    app["mcu_secret"] = mcu_secret
    app["tg_app"] = tg_app
    app.router.add_post("/tap", handle_tap)
    return app
