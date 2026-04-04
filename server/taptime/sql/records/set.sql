INSERT INTO records (uid, date, check_in, check_out) VALUES (?, ?, ?, ?)
ON CONFLICT(uid, date) DO UPDATE
    SET check_in  = excluded.check_in,
        check_out = excluded.check_out
