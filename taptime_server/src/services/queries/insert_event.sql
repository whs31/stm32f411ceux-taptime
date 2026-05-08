INSERT INTO events (id, user_id, date_days, event_kind, hour, minute, second)
VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6)
