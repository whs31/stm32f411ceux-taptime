SELECT date, time, event FROM tap_log WHERE uid = ? AND date >= ? AND date < ? ORDER BY date, time;
