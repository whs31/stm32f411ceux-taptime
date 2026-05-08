SELECT flags
FROM day_flags
WHERE user_id = $1
  AND date_days = $2
