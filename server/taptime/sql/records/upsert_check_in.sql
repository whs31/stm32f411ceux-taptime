INSERT INTO records (uid, date, check_in) VALUES (?, ?, ?)
ON CONFLICT(uid, date) DO UPDATE SET check_in = excluded.check_in
