CREATE TABLE IF NOT EXISTS day_flags (
  user_id   UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  date_days INT  NOT NULL,
  flags     INT  NOT NULL DEFAULT 0,
  PRIMARY KEY (user_id, date_days)
);

CREATE TABLE IF NOT EXISTS events (
  id         UUID     PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id    UUID     NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  date_days  INT      NOT NULL,
  event_kind SMALLINT NOT NULL,
  hour       SMALLINT NOT NULL,
  minute     SMALLINT NOT NULL,
  second     SMALLINT NOT NULL
);

CREATE INDEX IF NOT EXISTS events_user_date ON events (user_id, date_days);
