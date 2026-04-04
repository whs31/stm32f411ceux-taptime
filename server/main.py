import asyncio
import logging

import aiosqlite
from aiohttp import web
from telegram.ext import Application

from taptime.bot import register_handlers
from taptime.config import BOT_TOKEN, DB_PATH, MCU_HOST, MCU_PORT, MCU_SECRET
from taptime.db import init_db
from taptime.mcu import create_mcu_app

logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(message)s")
log = logging.getLogger(__name__)


async def main() -> None:
    async with aiosqlite.connect(DB_PATH) as db:
        await init_db(db)

        tg_app = Application.builder().token(BOT_TOKEN).build()
        tg_app.bot_data["db"] = db
        register_handlers(tg_app)

        await tg_app.initialize()
        await tg_app.start()
        await tg_app.updater.start_polling(drop_pending_updates=True)
        log.info("Telegram bot started")

        mcu_app = create_mcu_app(db, MCU_SECRET, tg_app)
        runner = web.AppRunner(mcu_app)
        await runner.setup()
        await web.TCPSite(runner, MCU_HOST, MCU_PORT).start()
        log.info("MCU HTTP server listening on %s:%d", MCU_HOST, MCU_PORT)

        try:
            await asyncio.Event().wait()
        finally:
            await tg_app.updater.stop()
            await tg_app.stop()
            await tg_app.shutdown()
            await runner.cleanup()


if __name__ == "__main__":
    asyncio.run(main())
