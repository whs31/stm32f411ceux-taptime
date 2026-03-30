import os

from dotenv import load_dotenv

load_dotenv()

BOT_TOKEN: str = os.environ["BOT_TOKEN"]
MCU_SECRET: str = os.environ.get("MCU_SECRET", "")
MCU_HOST: str = os.environ.get("MCU_HOST", "0.0.0.0")
MCU_PORT: int = int(os.environ.get("MCU_PORT", "8080"))
DB_PATH: str = os.environ.get("DB_PATH", "taptime.db")
