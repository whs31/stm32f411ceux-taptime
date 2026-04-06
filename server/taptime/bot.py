from datetime import date, datetime

import aiosqlite
from telegram import Update
from telegram.ext import Application, CommandHandler, ContextTypes

from .db import (
    add_day_off,
    add_remote_day_override,
    delete_record,
    delete_user,
    get_remote_workdays,
    get_user_by_telegram,
    get_user_by_uid,
    get_user_required_seconds,
    register_user,
    remove_day_off,
    remove_remote_day_override,
    set_record,
    set_remote_workdays,
    set_required_hours_override,
    set_user_required_seconds,
)
from .chart import render_month, render_year
from .workhours import (
    WEEKDAY_ABBR,
    WEEKDAY_FROM_ABBR,
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


# ── table helpers ─────────────────────────────────────────────────────────────

def _fmt_hms(seconds: int) -> str:
    h, rem = divmod(abs(seconds), 3600)
    m = rem // 60
    return f"{h}h{m:02d}m"


def _fmt_delta(seconds: int) -> str:
    sign = "+" if seconds >= 0 else "-"
    return f"{sign}{_fmt_hms(seconds)}"


def _month_table(rows: list, name: str, month_name: str, year: int, user_req: int) -> str:
    req_h = user_req / 3600
    header = f"{month_name} {year} -- {name}  (req: {req_h:.1f}h/day)"
    cols = ("Date", "Type", "In", "Out", "Worked", "Balance")
    widths = [6, 7, 5, 5, 6, 8]
    sep = "+" + "+".join("-" * (w + 2) for w in widths) + "+"
    hrow = "|" + "|".join(f" {c:<{w}} " for c, w in zip(cols, widths)) + "|"

    def row_line(values: tuple) -> str:
        return "|" + "|".join(f" {str(v):<{w}} " for v, w in zip(values, widths)) + "|"

    lines = [header, sep, hrow, sep]
    for r in rows:
        date_str = f"{r.d.day:2d} {r.weekday_abbr}"
        if r.is_day_off:
            lines.append(row_line((date_str, "off", "", "", "", "")))
        elif r.is_remote:
            ci = r.check_in[:5] if r.check_in else ""
            co = r.check_out[:5] if r.check_out else ""
            lines.append(row_line((date_str, "remote", ci, co, "", "")))
        elif r.check_in and r.check_out:
            w = seconds_worked(r.check_in, r.check_out)
            typ = "wknd" if r.is_weekend else "work"
            ci, co = r.check_in[:5], r.check_out[:5]
            worked = _fmt_hms(w)
            delta = "" if r.is_weekend or r.balance_seconds is None else _fmt_delta(r.balance_seconds)
            lines.append(row_line((date_str, typ, ci, co, worked, delta)))
        elif r.check_in:
            typ = "wknd" if r.is_weekend else "work"
            lines.append(row_line((date_str, typ, r.check_in[:5], "...", "", "")))
        elif not r.is_weekend:
            lines.append(row_line((date_str, "missing", "", "", "", "")))
        else:
            lines.append(row_line((date_str, "wknd", "", "", "", "")))

    lines.append(sep)

    non_wknd = [r for r in rows if not r.is_weekend]
    day_off_count = sum(1 for r in non_wknd if r.is_day_off)
    worked_days = sum(
        1 for r in non_wknd
        if not r.is_day_off and not r.is_remote and r.check_in and r.check_out
    )
    weekend_secs = sum(
        seconds_worked(r.check_in, r.check_out)
        for r in rows if r.is_weekend and r.check_in and r.check_out
    )
    total_ot = sum(r.balance_seconds for r in rows if r.balance_seconds and r.balance_seconds > 0)
    total_ut = sum(abs(r.balance_seconds) for r in rows if r.balance_seconds and r.balance_seconds < 0)
    net = total_ot - total_ut

    summary_parts = [f"Worked: {worked_days}d", f"Day-offs: {day_off_count}d"]
    if weekend_secs:
        summary_parts.append(f"Weekend: {_fmt_hms(weekend_secs)}")
    if total_ot:
        summary_parts.append(f"OT: +{_fmt_hms(total_ot)}")
    if total_ut:
        summary_parts.append(f"UT: -{_fmt_hms(total_ut)}")
    summary_parts.append(f"Balance: {_fmt_delta(net)}")
    lines.append("  ".join(summary_parts))

    return "\n".join(lines)


def _year_table(month_data: list[tuple[str, int]], name: str, year: int) -> str:
    header = f"{year} -- {name}"
    cols = ("Month", "Balance")
    widths = [5, 9]
    sep = "+" + "+".join("-" * (w + 2) for w in widths) + "+"
    hrow = "|" + "|".join(f" {c:<{w}} " for c, w in zip(cols, widths)) + "|"

    def row_line(values: tuple) -> str:
        return "|" + "|".join(f" {str(v):<{w}} " for v, w in zip(values, widths)) + "|"

    lines = [header, sep, hrow, sep]
    total = 0
    for mname, net_s in month_data:
        total += net_s
        lines.append(row_line((mname, _fmt_delta(net_s))))
    lines.append(sep)
    lines.append(row_line(("Total", _fmt_delta(total))))
    lines.append(sep)
    return "\n".join(lines)


async def cmd_register(update: Update, ctx: ContextTypes.DEFAULT_TYPE) -> None:
    db: aiosqlite.Connection = ctx.bot_data["db"]
    args = ctx.args or []
    if len(args) < 2:
        await update.message.reply_text("Usage: /register <NAME> <UID>")
        return

    name, uid = args[0], " ".join(args[1:])
    telegram_id = update.effective_user.id

    existing = await get_user_by_telegram(db, telegram_id)
    if existing:
        await update.message.reply_text(
            f"You are already registered as {existing[1]} (UID: {existing[2]})."
        )
        return

    if await get_user_by_uid(db, uid):
        await update.message.reply_text(
            f"UID {uid} is already registered to another user."
        )
        return

    await register_user(db, telegram_id, name, uid)
    await update.message.reply_text(f"✅ Registered: {name} with UID {uid}.")


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

    await update.message.reply_markdown_v2(
        f"Name: **{name}**\n"
        f"UID: `{uid}`\n\n"
        f"Required work hours: {req_str} per day\n"
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
    user_req = await user_default_seconds(db, uid)

    chart_buf = render_month(rows, name, month_name, year, user_req)
    await update.message.reply_photo(photo=chart_buf)

    table = _month_table(rows, name, month_name, year, user_req)
    await update.message.reply_text("```\n" + table + "\n```", parse_mode="Markdown")


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

    month_data: list[tuple[str, int]] = []
    for m in range(1, last_month + 1):
        mname = date(year, m, 1).strftime("%b")
        mrows = await month_rows(db, uid, year, m)
        net = sum(r.balance_seconds for r in mrows if r.balance_seconds is not None)
        month_data.append((mname, net))

    chart_buf = render_year(month_data, name, year)
    await update.message.reply_photo(photo=chart_buf)

    table = _year_table(month_data, name, year)
    await update.message.reply_text("```\n" + table + "\n```", parse_mode="Markdown")


async def cmd_time(update: Update, ctx: ContextTypes.DEFAULT_TYPE) -> None:
    db: aiosqlite.Connection = ctx.bot_data["db"]
    user = await get_user_by_telegram(db, update.effective_user.id)
    if not user:
        await update.message.reply_text(
            "You are not registered. Use /register <NAME> <UID>."
        )
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
        await update.message.reply_text(
            "You are not registered. Use /register <NAME> <UID>."
        )
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
    await update.message.reply_text(
        f"Set {d}: in={ci} out={co} ({format_duration(ci, co)})"
    )


async def cmd_reset(update: Update, ctx: ContextTypes.DEFAULT_TYPE) -> None:
    db: aiosqlite.Connection = ctx.bot_data["db"]
    user = await get_user_by_telegram(db, update.effective_user.id)
    if not user:
        await update.message.reply_text(
            "You are not registered. Use /register <NAME> <UID>."
        )
        return

    _, _name, uid = user
    args = ctx.args or []
    if not args:
        await update.message.reply_text("Usage: /reset <DATE>\nExample: /reset today")
        return

    d_obj = parse_date(args[0])
    if d_obj is None:
        await update.message.reply_text("Invalid date. Use YYYY-MM-DD or 'today'.")
        return

    d = d_obj.isoformat()
    record_deleted = await delete_record(db, uid, d)
    await remove_day_off(db, uid, d)
    if record_deleted:
        await update.message.reply_text(f"Record for {d} has been reset.")
    else:
        await update.message.reply_text(f"No record found for {d}.")


_UNREGISTER_CONFIRM = "yes, i want to delete all my data"


async def cmd_unregister(update: Update, ctx: ContextTypes.DEFAULT_TYPE) -> None:
    db: aiosqlite.Connection = ctx.bot_data["db"]
    telegram_id = update.effective_user.id
    user = await get_user_by_telegram(db, telegram_id)
    if not user:
        await update.message.reply_text("You are not registered.")
        return
    _, name, uid = user

    confirmation = " ".join(ctx.args or []).lower()
    if confirmation != _UNREGISTER_CONFIRM:
        await update.message.reply_text(
            f"WARNING: This will unregister {name} (UID: {uid}).\n"
            "All your check-in/out records, settings, and overrides will be lost.\n\n"
            "To confirm, send:\n"
            "/unregister Yes, I want to delete all my data"
        )
        return

    await delete_user(db, telegram_id)
    await update.message.reply_text(f"Unregistered {name}. All your data has been deleted.")


async def cmd_setrequiredworkhours(
    update: Update, ctx: ContextTypes.DEFAULT_TYPE
) -> None:
    db: aiosqlite.Connection = ctx.bot_data["db"]
    user = await get_user_by_telegram(db, update.effective_user.id)
    if not user:
        await update.message.reply_text(
            "You are not registered. Use /register <NAME> <UID>."
        )
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


async def cmd_setrequiredworktimeforaday(
    update: Update, ctx: ContextTypes.DEFAULT_TYPE
) -> None:
    db: aiosqlite.Connection = ctx.bot_data["db"]
    user = await get_user_by_telegram(db, update.effective_user.id)
    if not user:
        await update.message.reply_text(
            "You are not registered. Use /register <NAME> <UID>."
        )
        return

    _, _name, uid = user
    args = ctx.args or []

    if not args:
        req = await user_default_seconds(db, uid)
        rh, rrem = divmod(req, 3600)
        rm, rs = divmod(rrem, 60)
        req_str = f"{rh}h {rm}m" if rs == 0 else f"{rh}h {rm}m {rs}s"
        await update.message.reply_text(
            f"Current required work time: {req_str} per day."
        )
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
    await update.message.reply_text(
        f"Default required work time set to {req_str} per day."
    )


async def cmd_setremoteworkdays(update: Update, ctx: ContextTypes.DEFAULT_TYPE) -> None:
    db: aiosqlite.Connection = ctx.bot_data["db"]
    user = await get_user_by_telegram(db, update.effective_user.id)
    if not user:
        await update.message.reply_text(
            "You are not registered. Use /register <NAME> <UID>."
        )
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
        await update.message.reply_text(
            "You are not registered. Use /register <NAME> <UID>."
        )
        return

    _, _name, uid = user
    args = ctx.args or []
    if not args:
        await update.message.reply_text("Usage: /dayoff <DATE>\nExample: /dayoff today")
        return

    d_obj = parse_date(args[0])
    if d_obj is None:
        await update.message.reply_text("Invalid date. Use YYYY-MM-DD or 'today'.")
        return

    await add_day_off(db, uid, d_obj.isoformat())
    await update.message.reply_text(f"Day off recorded for {d_obj.isoformat()}.")


async def cmd_setremoteday(update: Update, ctx: ContextTypes.DEFAULT_TYPE) -> None:
    db: aiosqlite.Connection = ctx.bot_data["db"]
    user = await get_user_by_telegram(db, update.effective_user.id)
    if not user:
        await update.message.reply_text(
            "You are not registered. Use /register <NAME> <UID>."
        )
        return

    _, _name, uid = user
    args = ctx.args or []
    if not args:
        await update.message.reply_text("Usage: /setremoteday <DATE>\nExample: /setremoteday today")
        return

    d_obj = parse_date(args[0])
    if d_obj is None:
        await update.message.reply_text("Invalid date. Use YYYY-MM-DD or 'today'.")
        return

    await add_remote_day_override(db, uid, d_obj.isoformat())
    await update.message.reply_text(f"{d_obj.isoformat()} marked as remote.")


async def cmd_unsetremoteday(update: Update, ctx: ContextTypes.DEFAULT_TYPE) -> None:
    db: aiosqlite.Connection = ctx.bot_data["db"]
    user = await get_user_by_telegram(db, update.effective_user.id)
    if not user:
        await update.message.reply_text(
            "You are not registered. Use /register <NAME> <UID>."
        )
        return

    _, _name, uid = user
    args = ctx.args or []
    if not args:
        await update.message.reply_text(
            "Usage: /unsetremoteday <DATE>\nExample: /unsetremoteday today"
        )
        return

    d_obj = parse_date(args[0])
    if d_obj is None:
        await update.message.reply_text("Invalid date. Use YYYY-MM-DD or 'today'.")
        return

    await remove_remote_day_override(db, uid, d_obj.isoformat())
    await update.message.reply_text(f"Remote override removed for {d_obj.isoformat()}.")


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
    app.add_handler(
        CommandHandler("setrequiredworktimeforaday", cmd_setrequiredworktimeforaday)
    )
    app.add_handler(CommandHandler("setremoteday", cmd_setremoteday))
    app.add_handler(CommandHandler("unsetremoteday", cmd_unsetremoteday))
