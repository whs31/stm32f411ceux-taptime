INSERT INTO records (uid, date, check_out) VALUES (?, ?, ?)
ON CONFLICT(uid, date) DO UPDATE SET check_out = excluded.check_out
