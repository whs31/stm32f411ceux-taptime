INSERT INTO day_flags (user_id, date_days, flags)
VALUES ($1, $2, $3)
ON CONFLICT (user_id, date_days) DO UPDATE SET flags = EXCLUDED.flags
