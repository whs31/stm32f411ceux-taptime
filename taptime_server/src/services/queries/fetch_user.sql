SELECT id,
       name,
       email,
       organization,
       time_zone,
       created_at,
       last_seen,
       rfid_uid,
       required_work_hours_secs,
       lunch_break_duration_secs,
       weekends,
       remote_days
FROM users
WHERE id = $1