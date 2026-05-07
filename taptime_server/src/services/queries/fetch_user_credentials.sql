SELECT user_id, password_hash
FROM user_credentials
WHERE email = $1