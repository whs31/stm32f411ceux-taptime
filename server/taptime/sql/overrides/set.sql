INSERT INTO work_hour_overrides (uid, date, required_seconds) VALUES (?, ?, ?)
ON CONFLICT(uid, date) DO UPDATE SET required_seconds = excluded.required_seconds
