INSERT INTO user_settings (uid, required_seconds, lunch_seconds) VALUES (?, ?, ?)
ON CONFLICT(uid) DO UPDATE SET lunch_seconds = excluded.lunch_seconds
