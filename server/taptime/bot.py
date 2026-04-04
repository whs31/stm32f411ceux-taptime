from datetime import date, datetime

import aiosqlite
from telegram import Update
from telegram.ext import Application, CommandHandler, ContextTypes

from .db import (
    add_day_off,
    delete_record,
    delete_user,
    get_remote_workdays,
    get_user_by_telegram,
    get_user_by_uid,
    get_user_required_seconds,
    register_user,
    set_record,
    set_remote_workdays,
    set_required_hours_override,
    set_user_required_seconds,
)
from .workhours import (
    DEFAULT_REQUIRED_SECONDS,
    WEEKDAY_ABBR,
    WEEKDAY_FROM_ABBR,
    format_balance,
    format_delta,
    month_rows,
    seconds_worked,
    user_default_seconds,
)


def parse_date(s: str) -> date | None:
    if s.lower() == "today":
        return date.today()
    try:
        return datetime.strptime(s, "%Y-%m-%d").date()
    except ValueError:
        return None


def format_duration(ci: str, co: str) -> str:
    total = seconds_worked(ci, co)
    h, rem = divmod(total, 3600)
    m, s = divmod(rem, 60)
    return f"{h}h {m}m {s}s"


async def cmd_register(update: Update, ctx: ContextTypes.DEFAULT_TYPE) -> None:
    db: aiosqlite.Connection = ctx.bot_data["db"]
    args = ctx.args or []
    if len(args) < 2:
        await update.message.reply_text("Usage: /register <NAME> <UID>")
        return

    name, uid = args[0], args[1]
    telegram_id = update.effective_user.id

    existing = await get_user_by_telegram(db, telegram_id)
    if existing:
        await update.message.reply_text(
            f"You are already registered as {existing[1]} (UID: {existing[2]})."
        )
        return

    if await get_user_by_uid(db, uid):
        await update.message.reply_text(f"UID {uid} is already registered to another user.")
        return

    await register_user(db, telegram_id, name, uid)
    await update.message.reply_text(f"Registered: {name} with UID {uid}.")


async def cmd_me(update: Update, ctx: ContextTypes.DEFAULT_TYPE) -> None:
    db: aiosqlite.Connection = ctx.bot_data["db"]
    user = await get_user_by_telegram(db, update.effective_user.id)
    if not user:
        await update.message.reply_text(
            "You are not registered yet.\nUse /register <NAME> <UID> to register."
        )
        return
    _, name, uid = user

    req = await user_default_seconds(db, uid)
    rh, rrem = divmod(req, 3600)
    rm, rs = divmod(rrem, 60)
    req_str = f"{rh}h {rm}m" if rs == 0 else f"{rh}h {rm}m {rs}s"

    remote_wdays = await get_remote_workdays(db, uid)
    if remote_wdays:
        remote_str = ", ".join(WEEKDAY_ABBR[wd] for wd in sorted(remote_wdays))
    else:
        remote_str = "none"

    await update.message.reply_text(
        f"Name: {name}\n"
        f"UID: {uid}\n"
        f"Required: {req_str} per day\n"
        f"Remote days: {remote_str}"
    )


async def _send_month_view(
    update: Update,
    db: aiosqlite.Connection,
    uid: str,
    name: str,
    year: int,
    month: int,
) -> None:
    today = date.today()
    if year > today.year or (year == today.year and month > today.month):
        month_name = date(year, month, 1).strftime("%B")
        await update.message.reply_text(f"No data for {month_name} {year} (future month).")
        return

    rows = await month_rows(db, uid, year, month)
    month_name = date(year, month, 1).strftime("%B")

    lines = [f"{month_name} {year} — {name}"]

    for r in rows:
        prefix = f"{r.d.day:02d} {r.weekday_abbr}"
        if r.is_day_off:
            line = f"{prefix}  [day off]"
        elif r.is_remote:
            if r.check_in and r.check_out:
                line = f"{prefix}  {r.check_in} → {r.check_out}  [remote]"
            else:
                line = f"{prefix}  [remote]"
        elif r.is_weekend:
            if r.check_in and r.check_out:
                dur = format_duration(r.check_in, r.check_out)
                line = f"{prefix}  {r.check_in} → {r.check_out}  ({dur})  [weekend]"
            elif r.check_in:
                line = f"{prefix}  {r.check_in} → …  [weekend]"
            else:
                line = f"{prefix}  [weekend]"
        elif r.check_in and r.check_out:
            dur = format_duration(r.check_in, r.check_out)
            delta_str = f"  {format_delta(r.balance_seconds)}" if r.balance_seconds is not None else ""
            line = f"{prefix}  {r.check_in} → {r.check_out}  ({dur}){delta_str}"
        elif r.check_in:
            line = f"{prefix}  {r.check_in} → …"
        else:
            line = f"{prefix}  —"
        lines.append(line)

    day_off_count = sum(1 for r in rows if r.is_day_off)
    net = sum(r.balance_seconds for r in rows if r.balance_seconds is not None)

    weekend_seconds = sum(
        seconds_worked(r.check_in, r.check_out)
        for r in rows
        if r.is_weekend and r.check_in and r.check_out
    )

    lines.append("")
    if day_off_count:
        lines.append(f"Day-offs: {day_off_count}")
    if weekend_seconds > 0:
        user_req = await user_default_seconds(db, uid)
        wh, wrem = divmod(weekend_seconds, 3600)
        wm, ws = divmod(wrem, 60)
        days_eq = weekend_seconds / user_req
        lines.append(f"Weekend work: {wh}h {wm}m ({days_eq:.1f} days)")
    lines.append(f"Balance: {format_balance(net)}")

    await update.message.reply_text("\n".join(lines))


async def _send_year_view(
    update: Update,
    db: aiosqlite.Connection,
    uid: str,
    name: str,
    year: int,
) -> None:
    today = date.today()
    if year > today.year:
        await update.message.reply_text(f"No data for future year {year}.")
        return

    last_month = today.month if year == today.year else 12

    lines = [f"{year} — {name}"]
    total = 0
    total_weekend_seconds = 0

    for m in range(1, last_month + 1):
        month_name = date(year, m, 1).strftime("%b")
        rows = await month_rows(db, uid, year, m)
        net = sum(r.balance_seconds for r in rows if r.balance_seconds is not None)
        total += net
        total_weekend_seconds += sum(
            seconds_worked(r.check_in, r.check_out)
            for r in rows
            if r.is_weekend and r.check_in and r.check_out
        )
        lines.append(f"{month_name}  {format_delta(net)}")

    lines.append("")
    if total_weekend_seconds > 0:
        user_req = await user_default_seconds(db, uid)
        wh, wrem = divmod(total_weekend_seconds, 3600)
        wm, ws = divmod(wrem, 60)
        days_eq = total_weekend_seconds / user_req
        lines.append(f"Weekend work: {wh}h {wm}m ({days_eq:.1f} days)")
    lines.append(f"Total: {format_balance(total)}")

    await update.message.reply_text("\n".join(lines))


async def cmd_time(update: Update, ctx: ContextTypes.DEFAULT_TYPE) -> None:
    db: aiosqlite.Connection = ctx.bot_data["db"]
    user = await get_user_by_telegram(db, update.effective_user.id)
    if not user:
        await update.message.reply_text("You are not registered. Use /register <NAME> <UID>.")
        return

    _, name, uid = user
    args = ctx.args or []
    today = date.today()

    if not args:
        await _send_month_view(update, db, uid, name, today.year, today.month)
        return

    arg = args[0]
    if len(arg) == 4 and arg.isdigit():
        await _send_year_view(update, db, uid, name, int(arg))
        return

    await update.message.reply_text("Usage: /time [YYYY]\nExample: /time 2026")


async def cmd_settime(update: Update, ctx: ContextTypes.DEFAULT_TYPE) -> None:
    db: aiosqlite.Connection = ctx.bot_data["db"]
    user = await get_user_by_telegram(db, update.effective_user.id)
    if not user:
        await update.message.reply_text("You are not registered. Use /register <NAME> <UID>.")
        return

    _, _name, uid = user
    args = ctx.args or []
    if len(args) < 3:
        await update.message.reply_text(
            "Usage: /settime <DATE> <IN> <OUT>\n"
            "Example: /settime today 09:00:00 17:30:00"
        )
        return

    d_obj = parse_date(args[0])
    if d_obj is None:
        await update.message.reply_text("Invalid date. Use YYYY-MM-DD or 'today'.")
        return

    ci, co = args[1], args[2]
    try:
        datetime.strptime(ci, "%H:%M:%S")
        datetime.strptime(co, "%H:%M:%S")
    except ValueError:
        await update.message.reply_text("Invalid time format. Use HH:MM:SS")
        return

    d = d_obj.isoformat()
    await set_record(db, uid, d, ci, co)
    await update.message.reply_text(f"Set {d}: in={ci} out={co} ({format_duration(ci, co)})")


async def cmd_reset(update: Update, ctx: ContextTypes.DEFAULT_TYPE) -> None:
    db: aiosqlite.Connection = ctx.bot_data["db"]
    user = await get_user_by_telegram(db, update.effective_user.id)
    if not user:
        await update.message.reply_text("You are not registered. Use /register <NAME> <UID>.")
        return

    _, _name, uid = user
    args = ctx.args or []
    if not args:
        await update.message.reply_text(
            "Usage: /reset <DATE>\nExample: /reset today"
        )
        return

    d_obj = parse_date(args[0])
    if d_obj is None:
        await update.message.reply_text("Invalid date. Use YYYY-MM-DD or 'today'.")
        return

    d = d_obj.isoformat()
    if await delete_record(db, uid, d):
        await update.message.reply_text(f"Record for {d} has been reset.")
    else:
        await update.message.reply_text(f"No record found for {d}.")


async def cmd_unregister(update: Update, ctx: ContextTypes.DEFAULT_TYPE) -> None:
    db: aiosqlite.Connection = ctx.bot_data["db"]
    telegram_id = update.effective_user.id
    user = await get_user_by_telegram(db, telegram_id)
    if not user:
        await update.message.reply_text("You are not registered.")
        return
    _, name, _ = user
    await delete_user(db, telegram_id)
    await update.message.reply_text(f"Unregistered {name}. Your records are preserved.")


async def cmd_setrequiredworkhours(update: Update, ctx: ContextTypes.DEFAULT_TYPE) -> None:
    db: aiosqlite.Connection = ctx.bot_data["db"]
    user = await get_user_by_telegram(db, update.effective_user.id)
    if not user:
        await update.message.reply_text("You are not registered. Use /register <NAME> <UID>.")
        return

    _, _name, uid = user
    args = ctx.args or []
    if len(args) < 2:
        await update.message.reply_text(
            "Usage: /setrequiredworkhours <DATE> <HH:MM:SS>\n"
            "Example: /setrequiredworkhours today 04:00:00"
        )
        return

    d_obj = parse_date(args[0])
    if d_obj is None:
        await update.message.reply_text("Invalid date. Use YYYY-MM-DD or 'today'.")
        return

    hms = args[1]
    try:
        t = datetime.strptime(hms, "%H:%M:%S").time()
    except ValueError:
        await update.message.reply_text("Invalid format. Use HH:MM:SS")
        return

    required_seconds = t.hour * 3600 + t.minute * 60 + t.second
    await set_required_hours_override(db, uid, d_obj.isoformat(), required_seconds)
    h, rem = divmod(required_seconds, 3600)
    m, s = divmod(rem, 60)
    await update.message.reply_text(
        f"Required work hours for {d_obj.isoformat()} set to {h}h {m}m {s}s."
    )


async def cmd_setrequiredworktimeforaday(update: Update, ctx: ContextTypes.DEFAULT_TYPE) -> None:
    db: aiosqlite.Connection = ctx.bot_data["db"]
    user = await get_user_by_telegram(db, update.effective_user.id)
    if not user:
        await update.message.reply_text("You are not registered. Use /register <NAME> <UID>.")
        return

    _, _name, uid = user
    args = ctx.args or []

    if not args:
        req = await user_default_seconds(db, uid)
        rh, rrem = divmod(req, 3600)
        rm, rs = divmod(rrem, 60)
        req_str = f"{rh}h {rm}m" if rs == 0 else f"{rh}h {rm}m {rs}s"
        await update.message.reply_text(f"Current required work time: {req_str} per day.")
        return

    try:
        t = datetime.strptime(args[0], "%H:%M:%S").time()
    except ValueError:
        await update.message.reply_text(
            "Usage: /setrequiredworktimeforaday <HH:MM:SS>\n"
            "Example: /setrequiredworktimeforaday 08:00:00"
        )
        return

    seconds = t.hour * 3600 + t.minute * 60 + t.second
    await set_user_required_seconds(db, uid, seconds)
    h, rem = divmod(seconds, 3600)
    m, s = divmod(rem, 60)
    req_str = f"{h}h {m}m" if s == 0 else f"{h}h {m}m {s}s"
    await update.message.reply_text(f"Default required work time set to {req_str} per day.")


async def cmd_setremoteworkdays(update: Update, ctx: ContextTypes.DEFAULT_TYPE) -> None:
    db: aiosqlite.Connection = ctx.bot_data["db"]
    user = await get_user_by_telegram(db, update.effective_user.id)
    if not user:
        await update.message.reply_text("You are not registered. Use /register <NAME> <UID>.")
        return

    _, _name, uid = user
    args = ctx.args or []

    if not args:
        weekdays = await get_remote_workdays(db, uid)
        if not weekdays:
            await update.message.reply_text("No remote workdays set.")
        else:
            names = [WEEKDAY_ABBR[wd] for wd in sorted(weekdays)]
            await update.message.reply_text(f"Remote workdays: {', '.join(names)}")
        return

    weekdays = []
    for arg in args:
        wd = WEEKDAY_FROM_ABBR.get(arg.upper())
        if wd is None:
            await update.message.reply_text(
                f"Unknown day: {arg!r}\nValid: MON TUE WED THU FRI SAT SUN"
            )
            return
        weekdays.append(wd)

    await set_remote_workdays(db, uid, weekdays)
    names = [WEEKDAY_ABBR[wd] for wd in sorted(weekdays)]
    await update.message.reply_text(f"Remote workdays set to: {', '.join(names)}")


async def cmd_dayoff(update: Update, ctx: ContextTypes.DEFAULT_TYPE) -> None:
    db: aiosqlite.Connection = ctx.bot_data["db"]
    user = await get_user_by_telegram(db, update.effective_user.id)
    if not user:
        await update.message.reply_text("You are not registered. Use /register <NAME> <UID>.")
        return

    _, _name, uid = user
    args = ctx.args or []
    if not args:
        await update.message.reply_text(
            "Usage: /dayoff <DATE>\nExample: /dayoff today"
        )
        return

    d_obj = parse_date(args[0])
    if d_obj is None:
        await update.message.reply_text("Invalid date. Use YYYY-MM-DD or 'today'.")
        return

    await add_day_off(db, uid, d_obj.isoformat())
    await update.message.reply_text(f"Day off recorded for {d_obj.isoformat()}.")


def register_handlers(app: Application) -> None:
    app.add_handler(CommandHandler("register", cmd_register))
    app.add_handler(CommandHandler("me", cmd_me))
    app.add_handler(CommandHandler("time", cmd_time))
    app.add_handler(CommandHandler("settime", cmd_settime))
    app.add_handler(CommandHandler("reset", cmd_reset))
    app.add_handler(CommandHandler("setrequiredworkhours", cmd_setrequiredworkhours))
    app.add_handler(CommandHandler("unregister", cmd_unregister))
    app.add_handler(CommandHandler("setremoteworkdays", cmd_setremoteworkdays))
    app.add_handler(CommandHandler("dayoff", cmd_dayoff))
    app.add_handler(CommandHandler("setrequiredworktimeforaday", cmd_setrequiredworktimeforaday))
