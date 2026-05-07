INSERT INTO users
(id, name, email, organization, time_zone, created_at, last_seen, rfid_uid,
 required_work_hours_secs, lunch_break_duration_secs, weekends, remote_days)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)