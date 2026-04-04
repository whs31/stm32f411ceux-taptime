from datetime import date, datetime, timedelta

import aiosqlite
from telegram import Update
from telegram.ext import Application, CommandHandler, ContextTypes

from .db import (
    delete_record,
    delete_user,
    get_records,
    get_user_by_telegram,
    get_user_by_uid,
    register_user,
    set_record,
    set_required_hours_override,
)


def format_duration(ci: str, co: str) -> str:
    fmt = "%H:%M:%S"
    delta = datetime.strptime(co, fmt) - datetime.strptime(ci, fmt)
    total = int(delta.total_seconds())
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
    await update.message.reply_text(f"Name: {name}\nUID: {uid}")


async def cmd_time(update: Update, ctx: ContextTypes.DEFAULT_TYPE) -> None:
    db: aiosqlite.Connection = ctx.bot_data["db"]
    user = await get_user_by_telegram(db, update.effective_user.id)
    if not user:
        await update.message.reply_text("You are not registered. Use /register <NAME> <UID>.")
        return

    _, name, uid = user
    args = ctx.args or []

    if args:
        try:
            since = date.today() - timedelta(days=int(args[0]))
        except ValueError:
            await update.message.reply_text("Usage: /time [DAYS]\nExample: /time 7")
            return
    else:
        since = date.today() - timedelta(days=30)

    rows = await get_records(db, uid, since)
    if not rows:
        await update.message.reply_text("No records found for this period.")
        return

    lines = [f"Records for {name}:"]
    for d, ci, co in rows:
        dur = format_duration(ci, co) if ci and co else "—"
        lines.append(f"{d}  in: {ci or '—'}  out: {co or '—'}  ({dur})")

    await update.message.reply_text("\n".join(lines))


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
            "Usage: /settime <DATE> <IN> <OUT>\nExample: /settime 2024-01-15 09:00:00 17:30:00"
        )
        return

    d, ci, co = args[0], args[1], args[2]
    try:
        datetime.strptime(d, "%Y-%m-%d")
        datetime.strptime(ci, "%H:%M:%S")
        datetime.strptime(co, "%H:%M:%S")
    except ValueError:
        await update.message.reply_text("Invalid format. Use DATE=YYYY-MM-DD, IN/OUT=HH:MM:SS")
        return

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
        await update.message.reply_text("Usage: /reset <DATE>\nExample: /reset 2024-01-15")
        return

    d = args[0]
    try:
        datetime.strptime(d, "%Y-%m-%d")
    except ValueError:
        await update.message.reply_text("Invalid date format. Use YYYY-MM-DD")
        return

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
            "Example: /setrequiredworkhours 2024-01-20 04:00:00"
        )
        return

    d, hms = args[0], args[1]
    try:
        datetime.strptime(d, "%Y-%m-%d")
        t = datetime.strptime(hms, "%H:%M:%S").time()
    except ValueError:
        await update.message.reply_text("Invalid format. Use DATE=YYYY-MM-DD, HH:MM:SS")
        return

    required_seconds = t.hour * 3600 + t.minute * 60 + t.second
    await set_required_hours_override(db, uid, d, required_seconds)
    h, rem = divmod(required_seconds, 3600)
    m, s = divmod(rem, 60)
    await update.message.reply_text(
        f"Required work hours for {d} set to {h}h {m}m {s}s."
    )


def register_handlers(app: Application) -> None:
    app.add_handler(CommandHandler("register", cmd_register))
    app.add_handler(CommandHandler("me", cmd_me))
    app.add_handler(CommandHandler("time", cmd_time))
    app.add_handler(CommandHandler("settime", cmd_settime))
    app.add_handler(CommandHandler("reset", cmd_reset))
    app.add_handler(CommandHandler("setrequiredworkhours", cmd_setrequiredworkhours))
    app.add_handler(CommandHandler("unregister", cmd_unregister))
