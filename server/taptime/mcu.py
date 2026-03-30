import logging
from datetime import date, datetime

import aiosqlite
from aiohttp import web

from .bot import format_duration
from .db import get_user_by_uid, today_record, upsert_check_in, upsert_check_out

log = logging.getLogger(__name__)


def _parse_time(ts: str) -> datetime | None:
    try:
        return datetime.fromisoformat(ts)
    except ValueError:
        pass
    try:
        return datetime.combine(date.today(), datetime.strptime(ts, "%H:%M:%S").time())
    except ValueError:
        return None


async def handle_tap(request: web.Request) -> web.Response:
    db: aiosqlite.Connection = request.app["db"]
    secret: str = request.app["mcu_secret"]

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

    _, name, _ = user
    record = await today_record(db, uid)

    if record and record[1] and not record[2]:
        ci_str = await upsert_check_out(db, uid, dt)
        co_str = dt.strftime("%H:%M:%S")
        dur = format_duration(ci_str, co_str) if ci_str else "—"
        log.info("Check-out: %s at %s (duration %s)", name, dt, dur)
        return web.json_response(
            {"status": "check_out", "name": name, "duration": dur, "check_in": ci_str, "check_out": co_str}
        )
    else:
        await upsert_check_in(db, uid, dt)
        log.info("Check-in: %s at %s", name, dt)
        return web.json_response({"status": "check_in", "name": name, "check_in": dt.strftime("%H:%M:%S")})


def create_mcu_app(db: aiosqlite.Connection, mcu_secret: str) -> web.Application:
    app = web.Application()
    app["db"] = db
    app["mcu_secret"] = mcu_secret
    app.router.add_post("/tap", handle_tap)
    return app
