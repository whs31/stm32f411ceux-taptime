CREATE TABLE IF NOT EXISTS users (
  id                        UUID        PRIMARY KEY,
  name                      TEXT        NOT NULL,
  email                     TEXT        NOT NULL UNIQUE,
  organization              TEXT,
  time_zone                 TEXT        NOT NULL DEFAULT 'UTC',
  created_at                TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  last_seen                 TIMESTAMPTZ,
  rfid_uid                  BYTEA,
  required_work_hours_secs  BIGINT      NOT NULL DEFAULT 28800,
  lunch_break_duration_secs BIGINT      NOT NULL DEFAULT 1800,
  weekends                  INT[]       NOT NULL DEFAULT ARRAY[6, 7],
  remote_days               INT[]       NOT NULL DEFAULT ARRAY[]::INT[]
);

CREATE TABLE IF NOT EXISTS user_credentials (
  user_id       UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  email         TEXT NOT NULL UNIQUE,
  password_hash TEXT NOT NULL
);
