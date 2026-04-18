INSERT INTO user_settings (uid, required_seconds) VALUES (?, ?)
ON CONFLICT(uid) DO UPDATE SET required_seconds = excluded.required_seconds
