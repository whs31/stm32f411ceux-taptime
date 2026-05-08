SELECT required_work_hours_secs,
       lunch_break_duration_secs,
       weekends,
       remote_days
FROM users
WHERE id = $1
