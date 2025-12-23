CREATE FUNCTION refresh_session(session_id INTEGER, duration INTERVAL)
    RETURNS TABLE(user_id INTEGER, account_id INTEGER, expires_at TIMESTAMP) AS $$
DECLARE
    v_user_id INTEGER;
    v_account_id INTEGER;
    v_deleted_at TIMESTAMP;
    v_expires_at TIMESTAMP;
BEGIN
    SELECT a.user_id, a.id, a.deleted_at
    INTO v_user_id, v_account_id, v_deleted_at
    FROM sessions s
             JOIN accounts a ON s.account_id = a.id
    WHERE s.id = session_id
      AND s.expires_at > CURRENT_TIMESTAMP;

    IF v_deleted_at IS NULL THEN
        UPDATE sessions
        SET expires_at = CURRENT_TIMESTAMP + duration
        WHERE id = session_id
        RETURNING expires_at INTO v_expires_at;

        RETURN QUERY SELECT v_user_id, v_account_id, v_expires_at;
    END IF;
END;
$$ LANGUAGE plpgsql;
