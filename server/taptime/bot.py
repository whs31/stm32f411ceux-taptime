from datetime import date, datetime

import aiosqlite
from telegram import Update
from telegram.ext import Application, CommandHandler, ContextTypes

from .db import (
    add_day_off,
    add_remote_day_override,
    add_weekend_override,
    delete_record,
    delete_user,
    get_remote_workdays,
    get_tap_log_for_month,
    get_user_by_telegram,
    get_user_by_uid,
    get_user_required_seconds,
    register_user,
    remove_day_off,
    remove_remote_day_override,
    remove_weekend_override,
    set_record,
    set_remote_workdays,
    set_required_hours_override,
    set_user_lunch_seconds,
    set_user_required_seconds,
    today_record,
    upsert_check_in,
    upsert_check_out,
)
from .chart import render_month, render_month_timeline, render_year
from .workhours import (
    DEFAULT_REQUIRED_SECONDS,
    WEEKDAY_ABBR,
    WEEKDAY_FROM_ABBR,
    format_balance,
    month_net_balance,
    month_rows,
    required_seconds_for_date,
    seconds_worked,
    user_default_seconds,
    user_lunch_seconds,
)


def parse_time(s: str) -> str | None:
    """Parse HH:MM or HH:MM:SS, return normalized HH:MM:SS string or None."""
    for fmt in ("%H:%M:%S", "%H:%M"):
        try:
            return datetime.strptime(s, fmt).strftime("%H:%M:%S")
        except ValueError:
            pass
    return None


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
    lines = [f"{month_name} {year} / {name} / req {req_h:.1f}h", ""]

    for r in rows:
        prefix = f"{r.d.day:2d} {r.weekday_abbr}"
        if r.is_day_off:
            detail = "[off]"
        elif r.is_remote:
            detail = "[remote]"
        elif r.is_weekend and not (r.check_in and r.check_out):
            detail = "[weekend]" if not r.check_in else f"{r.check_in[:5]}->..."
        elif r.check_in and r.check_out:
            w = seconds_worked(r.check_in, r.check_out)
            worked = _fmt_hms(w)
            time_range = f"{r.check_in[:5]}-{r.check_out[:5]}"
            if r.is_weekend:
                detail = f"{time_range}  {worked} [wknd]"
            elif r.balance_seconds is not None:
                detail = f"{time_range}  {worked} {_fmt_delta(r.balance_seconds)}"
            else:
                detail = f"{time_range}  {worked}"
        elif r.check_in:
            detail = f"{r.check_in[:5]}->..."
        else:
            detail = "--"
        lines.append(f"{prefix}  {detail}")

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

    lines.append("")
    lines.append(f"Worked {worked_days}d  Off {day_off_count}d  Balance {_fmt_delta(net)}")
    extras = []
    if total_ot:
        extras.append(f"OT +{_fmt_hms(total_ot)}")
    if total_ut:
        extras.append(f"UT -{_fmt_hms(total_ut)}")
    if weekend_secs:
        extras.append(f"Wknd {_fmt_hms(weekend_secs)}")
    if extras:
        lines.append("  ".join(extras))

    return "\n".join(lines)


def _year_table(month_data: list[tuple[str, int]], name: str, year: int) -> str:
    lines = [f"{year} / {name}", ""]
    total = 0
    for mname, net_s in month_data:
        total += net_s
        lines.append(f"{mname:<3}  {_fmt_delta(net_s)}")
    lines.append("---  ---------")
    lines.append(f"Tot  {_fmt_delta(total)}")
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

    lunch = await user_lunch_seconds(db, uid)
    lh, lrem = divmod(lunch, 3600)
    lm, ls = divmod(lrem, 60)
    if lh:
        lunch_str = f"{lh}h {lm}m" if ls == 0 else f"{lh}h {lm}m {ls}s"
    else:
        lunch_str = f"{lm}m" if ls == 0 else f"{lm}m {ls}s"

    remote_wdays = await get_remote_workdays(db, uid)
    if remote_wdays:
        remote_str = ", ".join(WEEKDAY_ABBR[wd] for wd in sorted(remote_wdays))
    else:
        remote_str = "none"

    await update.message.reply_markdown_v2(
        f"Name: **{name}**\n"
        f"UID: `{uid}`\n\n"
        f"Required work hours: {req_str} per day\n"
        f"Lunch time: {lunch_str}\n"
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
    lunch = await user_lunch_seconds(db, uid)

    chart_buf = render_month(rows, name, month_name, year, user_req, lunch)
    await update.message.reply_photo(photo=chart_buf)

    tap_log_rows = await get_tap_log_for_month(db, uid, year, month)
    if any(r.check_in for r in rows):
        timeline_buf = render_month_timeline(rows, tap_log_rows, name, month_name, year)
        await update.message.reply_photo(photo=timeline_buf)

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

    try:
        parsed = datetime.strptime(arg, "%Y-%m")
        await _send_month_view(update, db, uid, name, parsed.year, parsed.month)
        return
    except ValueError:
        pass

    await update.message.reply_text(
        "Usage: /time [YYYY] or /time [YYYY-MM]\n"
        "Examples: /time 2026    /time 2026-03"
    )


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

    ci = parse_time(args[1])
    co = parse_time(args[2])
    if ci is None or co is None:
        await update.message.reply_text("Invalid time format. Use HH:MM or HH:MM:SS")
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


async def cmd_setlunchtime(update: Update, ctx: ContextTypes.DEFAULT_TYPE) -> None:
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
        lunch = await user_lunch_seconds(db, uid)
        lh, lrem = divmod(lunch, 3600)
        lm, ls = divmod(lrem, 60)
        if lh:
            lunch_str = f"{lh}h {lm}m" if ls == 0 else f"{lh}h {lm}m {ls}s"
        else:
            lunch_str = f"{lm}m" if ls == 0 else f"{lm}m {ls}s"
        await update.message.reply_text(f"Current lunch time: {lunch_str}.")
        return

    hms = parse_time(args[0])
    if hms is None:
        await update.message.reply_text(
            "Usage: /setlunchtime <HH:MM or HH:MM:SS>\n"
            "Example: /setlunchtime 00:30"
        )
        return

    t = datetime.strptime(hms, "%H:%M:%S").time()
    seconds = t.hour * 3600 + t.minute * 60 + t.second
    await set_user_lunch_seconds(db, uid, seconds, DEFAULT_REQUIRED_SECONDS)
    h, rem = divmod(seconds, 3600)
    m, s = divmod(rem, 60)
    if h:
        lunch_str = f"{h}h {m}m" if s == 0 else f"{h}h {m}m {s}s"
    else:
        lunch_str = f"{m}m" if s == 0 else f"{m}m {s}s"
    await update.message.reply_text(f"Lunch time set to {lunch_str}.")


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


async def cmd_checkin(update: Update, ctx: ContextTypes.DEFAULT_TYPE) -> None:
    db: aiosqlite.Connection = ctx.bot_data["db"]
    user = await get_user_by_telegram(db, update.effective_user.id)
    if not user:
        await update.message.reply_text(
            "You are not registered. Use /register <NAME> <UID>."
        )
        return

    _, name, uid = user
    args = ctx.args or []
    if not args:
        await update.message.reply_text(
            "Usage: /checkin <HH:MM[:SS]>\nExample: /checkin 09:00"
        )
        return

    t = parse_time(args[0])
    if t is None:
        await update.message.reply_text("Invalid time format. Use HH:MM or HH:MM:SS")
        return

    dt = datetime.combine(date.today(), datetime.strptime(t, "%H:%M:%S").time())
    record = await today_record(db, uid)
    if record and record[1] and record[2]:
        # Already checked out today → reopen, preserving original check_in
        await reopen_checkin(db, uid, dt)
    else:
        await upsert_check_in(db, uid, dt)
    await update.message.reply_text(f"Checked in at {t}.")


async def cmd_checkout(update: Update, ctx: ContextTypes.DEFAULT_TYPE) -> None:
    db: aiosqlite.Connection = ctx.bot_data["db"]
    user = await get_user_by_telegram(db, update.effective_user.id)
    if not user:
        await update.message.reply_text(
            "You are not registered. Use /register <NAME> <UID>."
        )
        return

    _, name, uid = user
    args = ctx.args or []
    if not args:
        await update.message.reply_text(
            "Usage: /checkout <HH:MM[:SS]>\nExample: /checkout 17:30"
        )
        return

    t = parse_time(args[0])
    if t is None:
        await update.message.reply_text("Invalid time format. Use HH:MM or HH:MM:SS")
        return

    today = date.today()
    dt = datetime.combine(today, datetime.strptime(t, "%H:%M:%S").time())
    ci_str = await upsert_check_out(db, uid, dt)
    co_str = dt.strftime("%H:%M:%S")

    if ci_str:
        req = await required_seconds_for_date(db, uid, today)
        if req is not None:
            worked = seconds_worked(ci_str, co_str)
            day_bal = format_balance(worked - req)
        else:
            day_bal = "non-workday"

        month_bal = await month_net_balance(db, uid, today.year, today.month)
        month_str = format_balance(month_bal)
        month_name = today.strftime("%B")

        await update.message.reply_text(
            f"Checked out at {co_str}\n"
            f"Today: {day_bal}\n"
            f"{month_name}: {month_str}"
        )
    else:
        await update.message.reply_text(f"Checked out at {co_str}.")


async def cmd_setweekend(update: Update, ctx: ContextTypes.DEFAULT_TYPE) -> None:
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
            "Usage: /setweekend <DATE> [true|false]\n"
            "  true  (default) — treat date as a non-workday weekend\n"
            "  false           — treat date as a regular workday\n"
            "Examples:\n"
            "  /setweekend 2026-05-09\n"
            "  /setweekend 2026-05-10 false"
        )
        return

    d_obj = parse_date(args[0])
    if d_obj is None:
        await update.message.reply_text("Invalid date. Use YYYY-MM-DD or 'today'.")
        return

    is_weekend = True
    if len(args) >= 2:
        v = args[1].lower()
        if v in ("false", "0", "no", "off"):
            is_weekend = False
        elif v not in ("true", "1", "yes", "on"):
            await update.message.reply_text("Second argument must be true or false.")
            return

    d = d_obj.isoformat()
    wd = d_obj.weekday()

    if is_weekend:
        await add_weekend_override(db, uid, d)
        await update.message.reply_text(f"{d} marked as weekend.")
    else:
        await remove_weekend_override(db, uid, d)
        if wd >= 5:
            # For a natural Sat/Sun, add a work-hour override so it becomes a workday
            user_req = await user_default_seconds(db, uid)
            await set_required_hours_override(db, uid, d, user_req)
            h, rem = divmod(user_req, 3600)
            m = rem // 60
            await update.message.reply_text(
                f"{d} marked as workday (required: {h}h {m:02d}m)."
            )
        else:
            await update.message.reply_text(f"{d} marked as regular workday.")


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
    app.add_handler(CommandHandler("checkin", cmd_checkin))
    app.add_handler(CommandHandler("checkout", cmd_checkout))
    app.add_handler(CommandHandler("setlunchtime", cmd_setlunchtime))
    app.add_handler(CommandHandler("setweekend", cmd_setweekend))
