SELECT event_kind,
       hour,
       minute,
       second
FROM events
WHERE user_id = $1
  AND date_days = $2
ORDER BY hour, minute, second
