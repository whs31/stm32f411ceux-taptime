# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repo layout

TapTime is two separate sub-projects in one repo:

- **Firmware** (`src/`, `Cargo.toml`, `memory.x`, `Embed.toml`, `.cargo/config.toml`) — Rust/Embassy app for an STM32F411CEUx. No-std with a 1 KiB `embedded-alloc` heap.
- **Server** (`server/`) — Python 3.12 service (Telegram bot + aiohttp `/tap` endpoint), SQLite-backed. Managed with `uv`.
- `pcb/` holds the board artwork/gerbers. `extern/ssd1306` and `extern/ds3231` are vendored driver crates patched in by `Cargo.toml` / paths.

The two halves communicate over HTTP: the MCU POSTs `{"uid", "time"}` to the server's `/tap`. The server decides check-in vs. check-out from today's record state and replies with a status/name/duration, which the firmware displays on the OLED.

## Firmware (Rust)

Build target is `thumbv7em-none-eabihf` (set in `.cargo/config.toml`); the runner is `probe-rs`. `flip-link` is required in PATH.

```
cargo build                         # debug build
cargo build --release               # release (opt-level=z, LTO)
cargo run                           # flash + attach defmt-rtt via probe-rs
cargo run --features clock_set      # one-shot: program the DS3231 from host build time, then halt
cargo fmt                           # uses rustfmt.toml (2-space tabs, crate-granular imports)
```

`Cargo.toml` has `exclude = ["bot"]` — the old name for `server/` — so Cargo will not walk into the server tree even though both live under the repo root.

### Firmware architecture

- `src/main.rs` owns hardware bring-up and only hardware bring-up: it instantiates peripherals (I2C1 for OLED + DS3231 on a shared `RefCell` bus, SPI1 for the MFRC522 reader, USART1+DMA for the Wi-Fi module, TIM4 PWM for the buzzer) and passes them into `Firmware::init`.
- `src/firmware.rs` is the async orchestrator. It re-exports the per-peripheral wrappers from `src/firmware/{buzzer,oled,onboard_led,rfid,rtc,wifi}.rs` and drives the main loop. `Firmware::run` connects Wi-Fi with a spinner animation then loops `tick()`; `tick()` polls RFID for ~900 ms, and on a tap it beeps, builds a JSON body with the RTC timestamp, POSTs to `/tap`, and routes the `status` field (`check_in`/`check_out`/unknown) into the right OLED screen. Between taps it just refreshes the clock display.
- Network target and Wi-Fi credentials are **hardcoded constants at the top of `firmware.rs`** (`WIFI_SSID`, `WIFI_PASSWORD`, `SERVER_HOST`, `SERVER_PORT`, `SERVER_SECRET`) — edit them in-tree rather than looking for a config file.
- The `clock_set` Cargo feature is a build-time RTC programmer: `configure_clock(build_time::build_time_local!())` writes the host's build timestamp into the DS3231 once and returns, so the normal loop never runs. Use it by flashing once with the feature enabled, then reflashing without it.
- JSON parsing is intentionally primitive (`json_field` does a key-string search) because the heap is only 1024 bytes; keep allocations minimal when extending the protocol.

## Server (Python)

```bash
cd server
uv run python main.py                # local dev
docker compose up -d                 # prod-like run; DB persisted under ./data
```

Required env (via `.env`): `BOT_TOKEN`. Optional: `MCU_SECRET`, `MCU_HOST`, `MCU_PORT`, `DB_PATH` (see `server/taptime/config.py`).

There are no tests in this repo.

### Server architecture

- `server/main.py` runs the Telegram bot (`python-telegram-bot` polling) and the aiohttp MCU endpoint **concurrently against a single shared `aiosqlite` connection** stored on both `tg_app.bot_data["db"]` and `mcu_app["db"]`. Anything added must be async and must not block that connection.
- `taptime/mcu.py` is the `/tap` handler. It is stateful over `today_record`: if there's an open check-in, the tap becomes a check-out; if today is already closed, the tap reopens it (preserving the original `check_in`); otherwise it creates a fresh record. The records table only ever stores **first check-in and latest check-out** per day, so "worked time" is always the timespan between them.
- `taptime/bot.py` holds all `/`-command handlers and the month/year text tables rendered back to the user.
- `taptime/workhours.py` centralises the balance rules: per-day required seconds fall back through (per-date override → weekend → user default → `DEFAULT_REQUIRED_SECONDS`). `month_rows()` classifies each day as day-off / remote / weekend / regular and emits a `DayRow` with `balance_seconds`. Missing past workdays (no check-in and no check-out) get `-(required - lunch)` rather than `-required`. **Change balance logic here, not in callers.**
- `taptime/chart.py` uses matplotlib with the `Agg` backend and returns a `BytesIO` — do not change the backend or import `pyplot` before setting it.
- `taptime/db.py` loads every query from a `.sql` file under `taptime/sql/` via `_q()`. When adding a query, create a new file and load it the same way rather than inlining SQL. The schema uses `CREATE TABLE IF NOT EXISTS`, so schema changes to existing tables must be accompanied by an idempotent `ALTER TABLE` in `_migrate_user_settings` (or a new `_migrate_*` helper invoked from `init_db`) — editing `schema.sql` alone will not update deployed DBs.
- `user_settings` is an upsert-merged row per uid: `set.sql` and `set_lunch.sql` both use `ON CONFLICT(uid) DO UPDATE SET <only_that_column>` so setting one field does not clobber the other. `set_lunch` additionally passes `DEFAULT_REQUIRED_SECONDS` as the INSERT-path default because pre-migration columns lack a schema default for `required_seconds`.
- "Remote day" state is tri-valued: weekly pattern (`remote_workdays`), positive override (`remote_day_overrides`), and negative override (`non_remote_day_overrides`). The last wins — see the `is_remote = ... and d_str not in non_remote_day_dates` guards in `month_rows`.
