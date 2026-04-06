CREATE TABLE IF NOT EXISTS users (
    telegram_id INTEGER PRIMARY KEY,
    name        TEXT NOT NULL,
    uid         TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS records (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    uid       TEXT NOT NULL,
    date      TEXT NOT NULL,
    check_in  TEXT,
    check_out TEXT,
    UNIQUE(uid, date)
);

CREATE TABLE IF NOT EXISTS work_hour_overrides (
    uid              TEXT NOT NULL,
    date             TEXT NOT NULL,
    required_seconds INTEGER NOT NULL,
    PRIMARY KEY (uid, date)
);

CREATE TABLE IF NOT EXISTS remote_workdays (
    uid     TEXT NOT NULL,
    weekday INTEGER NOT NULL,
    PRIMARY KEY (uid, weekday)
);

CREATE TABLE IF NOT EXISTS day_offs (
    uid  TEXT NOT NULL,
    date TEXT NOT NULL,
    PRIMARY KEY (uid, date)
);

CREATE TABLE IF NOT EXISTS user_settings (
    uid              TEXT PRIMARY KEY,
    required_seconds INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS remote_day_overrides (
    uid  TEXT NOT NULL,
    date TEXT NOT NULL,
    PRIMARY KEY (uid, date)
);
