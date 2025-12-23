DROP FUNCTION public.get_user_from_login(p_provider character varying, p_provider_user_id character varying,
                                         p_provider_username character varying,
                                         p_provider_display_name character varying, p_session_duration interval);

CREATE FUNCTION public.get_user_from_login(p_provider character varying, p_provider_user_id character varying,
                                           p_provider_username character varying,
                                           p_provider_display_name character varying, p_session_duration interval)
    RETURNS TABLE
            (
                user_id            uuid,
                user_name          character varying,
                account_id         uuid,
                session_id         uuid,
                session_expires_at timestamp with time zone
            )
    LANGUAGE plpgsql
AS
$$
DECLARE
    v_user_id            uuid;
    v_user_name          varchar;
    v_account_id         uuid;
    v_session_id         uuid;
    v_session_created_at timestamptz;
    v_session_expires_at timestamptz;
BEGIN
    -- Try to find existing account (exclude soft-deleted accounts)
    SELECT u.id, u.name, a.id
    INTO v_user_id, v_user_name, v_account_id
    FROM users u
             INNER JOIN accounts a ON a.user_id = u.id
    WHERE a.provider = p_provider
      AND a.provider_user_id = p_provider_user_id
      AND a.deleted_at IS NULL;

    IF FOUND THEN
        -- Update cached provider info (only if changed)
        UPDATE accounts a
        SET provider_username     = p_provider_username,
            provider_display_name = p_provider_display_name
        WHERE a.provider = p_provider
          AND a.provider_user_id = p_provider_user_id
          AND a.deleted_at IS NULL
          AND (a.provider_username IS DISTINCT FROM p_provider_username
            OR a.provider_display_name IS DISTINCT FROM p_provider_display_name);
    ELSE
        -- Create new user and account
        INSERT INTO users (name)
        VALUES (p_provider_display_name)
        RETURNING users.id, users.name INTO v_user_id, v_user_name;

        INSERT INTO accounts (provider, provider_user_id, provider_username, provider_display_name, user_id)
        VALUES (p_provider, p_provider_user_id, p_provider_username, p_provider_display_name, v_user_id)
        RETURNING accounts.id INTO v_account_id;
    END IF;

    -- Create new session
    v_session_expires_at := CURRENT_TIMESTAMP + p_session_duration;
    INSERT INTO sessions (account_id, expires_at)
    VALUES (v_account_id, v_session_expires_at)
    RETURNING sessions.id, sessions.created_at INTO v_session_id, v_session_created_at;

    -- Return all data
    RETURN QUERY
        SELECT v_user_id,
               v_user_name,
               v_account_id,
               v_session_id,
               v_session_created_at,
               v_session_expires_at;
END;
$$;
