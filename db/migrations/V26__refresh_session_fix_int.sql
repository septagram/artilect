DROP FUNCTION refresh_session(session_id integer, duration interval);

CREATE FUNCTION refresh_session(session_id UUID, duration interval) RETURNS TABLE(user_id UUID, account_id UUID, expires_at timestamp without time zone)
    LANGUAGE plpgsql
AS $$
DECLARE
    v_user_id UUID;
    v_account_id UUID;
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
$$;
