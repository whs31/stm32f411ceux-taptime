SELECT date, required_seconds
FROM work_hour_overrides
WHERE uid = ? AND date >= ? AND date < ?
