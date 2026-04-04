SELECT date, check_in, check_out
FROM records
WHERE uid = ? AND date >= ? AND date < ?
