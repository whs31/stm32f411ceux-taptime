# taptime — server

Telegram bot + HTTP endpoint for RFID-based check-in/check-out tracking.

## Architecture

```
server/
├── main.py              # entry point
├── taptime/
│   ├── config.py        # env vars
│   ├── db.py            # SQLite queries (aiosqlite)
│   ├── bot.py           # Telegram command handlers
│   └── mcu.py           # HTTP endpoint for MCU taps
├── compose.yaml
├── Dockerfile
├── .env.example
└── .env                 # not committed
```

## Configuration

Copy `.env.example` to `.env` and fill in the values:

```bash
cp .env.example .env
```

| Variable     | Required | Default        | Description                                      |
|--------------|----------|----------------|--------------------------------------------------|
| `BOT_TOKEN`  | yes      | —              | Telegram bot token from [@BotFather](https://t.me/BotFather) |
| `MCU_SECRET` | no       | *(empty)*      | Shared secret sent by the MCU as `X-Secret` header |
| `MCU_HOST`   | no       | `0.0.0.0`      | HTTP server bind address                         |
| `MCU_PORT`   | no       | `8080`         | HTTP server port                                 |
| `DB_PATH`    | no       | `taptime.db`   | Path to the SQLite database file                 |

## Running

### uv (development)

```bash
uv run python main.py
```

### Docker Compose (recommended)

```bash
mkdir -p data
docker compose up -d
docker compose logs -f
```

### Docker

```bash
docker build -t taptime .
mkdir -p data
docker run -d \
  --name taptime \
  --env-file .env \
  -v $(pwd)/data:/data \
  -p 8080:8080 \
  taptime
```

### Podman

```bash
podman build -t taptime .
mkdir -p data
podman run -d \
  --name taptime \
  --env-file .env \
  -v $(pwd)/data:/data:Z \
  -p 8080:8080 \
  taptime
```

## Telegram commands

| Command | Description |
|---|---|
| `/register <NAME> <UID>` | Link your Telegram account to an RFID UID. Fails if already registered or UID is taken. |
| `/me` | Show your registered name and UID. |
| `/time [DAYS]` | Show check-in/check-out history. Defaults to the last 30 days. |
| `/settime <YYYY-MM-DD> <HH:MM:SS> <HH:MM:SS>` | Force-set check-in and check-out times for a date. |

## MCU HTTP endpoint

**`POST /tap`**

The MCU sends a tap event. The server determines check-in or check-out based on whether the UID already has an open record for today.

Request body:
```json
{ "uid": "AABBCCDD", "time": "2024-01-15T09:30:00" }
```
`time` accepts ISO 8601 datetime or bare `HH:MM:SS` (assumes today).

If `MCU_SECRET` is set, include the header:
```
X-Secret: <secret>
```

Responses:

```json
{ "status": "check_in",  "name": "Alice", "check_in": "09:30:00" }
{ "status": "check_out", "name": "Alice", "check_in": "09:30:00", "check_out": "17:45:00", "duration": "8h 15m 0s" }
{ "status": "unknown_uid", "uid": "AABBCCDD" }
```
