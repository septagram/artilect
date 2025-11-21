CREATE OR REPLACE FUNCTION get_user_from_login(
    p_provider auth_provider,
    p_provider_user_id varchar,
    p_provider_username varchar,
    p_provider_display_name varchar
) RETURNS TABLE(id uuid, name varchar) AS $$
DECLARE
    v_user_id uuid;
    v_user_name varchar;
BEGIN
    -- Try to find existing account
    SELECT u.id, u.name INTO v_user_id, v_user_name
    FROM users u
             INNER JOIN accounts a ON a.user_id = u.id
    WHERE a.provider = p_provider AND a.provider_user_id = p_provider_user_id;

    IF FOUND THEN
        -- Update cached provider info
        UPDATE accounts a
        SET provider_username = p_provider_username,
            provider_display_name = p_provider_display_name,
            updated_at = CURRENT_TIMESTAMP
        WHERE a.provider = p_provider AND a.provider_user_id = p_provider_user_id
          AND (a.provider_username IS DISTINCT FROM p_provider_username
            OR a.provider_display_name IS DISTINCT FROM p_provider_display_name);

        RETURN QUERY SELECT v_user_id, v_user_name;
    ELSE
        -- Create new user and account
        INSERT INTO users (name)
        VALUES (p_provider_display_name)
        RETURNING users.id, users.name INTO v_user_id, v_user_name;

        INSERT INTO accounts (provider, provider_user_id, provider_username, provider_display_name, user_id)
        VALUES (p_provider, p_provider_user_id, p_provider_username, p_provider_display_name, v_user_id);

        RETURN QUERY SELECT v_user_id, v_user_name;
    END IF;
END;
$$ LANGUAGE plpgsql;
