--
-- PostgreSQL database dump
--

-- Dumped from database version 17.2
-- Dumped by pg_dump version 17.2

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET transaction_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Name: auth_provider; Type: TYPE; Schema: public; Owner: postgres
--

CREATE TYPE public.auth_provider AS ENUM (
    'Telegram'
);


ALTER TYPE public.auth_provider OWNER TO postgres;

--
-- Name: get_user_from_login(public.auth_provider, character varying, character varying, character varying, interval); Type: FUNCTION; Schema: public; Owner: postgres
--

CREATE FUNCTION public.get_user_from_login(p_provider public.auth_provider, p_provider_user_id character varying, p_provider_username character varying, p_provider_display_name character varying, p_session_duration interval) RETURNS TABLE(user_id uuid, user_name character varying, account_id uuid, provider public.auth_provider, provider_username character varying, provider_display_name character varying, session_id uuid, session_expires_at timestamp with time zone)
    LANGUAGE plpgsql
    AS $$
DECLARE
    v_user_id uuid;
    v_user_name varchar;
    v_account_id uuid;
    v_session_id uuid;
    v_session_expires_at timestamptz;
BEGIN
    -- Try to find existing account
    SELECT u.id, u.name, a.id INTO v_user_id, v_user_name, v_account_id
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
    INSERT INTO sessions (user_id, account_id, expires_at)
    VALUES (v_user_id, v_account_id, v_session_expires_at)
    RETURNING sessions.id INTO v_session_id;

    -- Return all data (fixed: v_session_expires_at instead of v_session_expires)
    RETURN QUERY
    SELECT v_user_id, v_user_name, v_account_id, p_provider, p_provider_username, p_provider_display_name, v_session_id, v_session_expires_at;
END;
$$;


ALTER FUNCTION public.get_user_from_login(p_provider public.auth_provider, p_provider_user_id character varying, p_provider_username character varying, p_provider_display_name character varying, p_session_duration interval) OWNER TO postgres;

SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: accounts; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.accounts (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    provider public.auth_provider NOT NULL,
    provider_user_id character varying(255) NOT NULL,
    provider_username character varying(255),
    provider_display_name character varying(255),
    user_id uuid NOT NULL,
    created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL
);


ALTER TABLE public.accounts OWNER TO postgres;

--
-- Name: messages; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.messages (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    thread_id uuid NOT NULL,
    user_id uuid,
    content text NOT NULL,
    created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at timestamp with time zone
);


ALTER TABLE public.messages OWNER TO postgres;

--
-- Name: refinery_schema_history; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.refinery_schema_history (
    version integer NOT NULL,
    name character varying(255),
    applied_on character varying(255),
    checksum character varying(255)
);


ALTER TABLE public.refinery_schema_history OWNER TO postgres;

--
-- Name: sessions; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.sessions (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    user_id uuid NOT NULL,
    account_id uuid NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    deleted_at timestamp with time zone
);


ALTER TABLE public.sessions OWNER TO postgres;

--
-- Name: thread_participants; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.thread_participants (
    thread_id uuid NOT NULL,
    user_id uuid NOT NULL
);


ALTER TABLE public.thread_participants OWNER TO postgres;

--
-- Name: threads; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.threads (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255),
    owner_id uuid NOT NULL,
    created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    pending_updates boolean DEFAULT true NOT NULL
);


ALTER TABLE public.threads OWNER TO postgres;

--
-- Name: users; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.users (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    name character varying(255) NOT NULL
);


ALTER TABLE public.users OWNER TO postgres;

--
-- Name: accounts accounts_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.accounts
    ADD CONSTRAINT accounts_pkey PRIMARY KEY (id);


--
-- Name: accounts accounts_provider_user_unique; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.accounts
    ADD CONSTRAINT accounts_provider_user_unique UNIQUE (provider, provider_user_id);


--
-- Name: messages messages_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.messages
    ADD CONSTRAINT messages_pkey PRIMARY KEY (id);


--
-- Name: refinery_schema_history refinery_schema_history_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.refinery_schema_history
    ADD CONSTRAINT refinery_schema_history_pkey PRIMARY KEY (version);


--
-- Name: sessions sessions_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.sessions
    ADD CONSTRAINT sessions_pkey PRIMARY KEY (id);


--
-- Name: thread_participants thread_participants_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.thread_participants
    ADD CONSTRAINT thread_participants_pkey PRIMARY KEY (thread_id, user_id);


--
-- Name: threads threads_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.threads
    ADD CONSTRAINT threads_pkey PRIMARY KEY (id);


--
-- Name: users users_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_pkey PRIMARY KEY (id);


--
-- Name: idx_accounts_provider_user; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_accounts_provider_user ON public.accounts USING btree (provider, provider_user_id);


--
-- Name: idx_messages_created_at; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_messages_created_at ON public.messages USING btree (created_at);


--
-- Name: idx_messages_thread_created; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_messages_thread_created ON public.messages USING btree (thread_id, created_at);


--
-- Name: idx_thread_activity; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_thread_activity ON public.threads USING btree (updated_at DESC);


--
-- Name: idx_thread_participants_both; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_thread_participants_both ON public.thread_participants USING btree (user_id, thread_id);


--
-- Name: idx_thread_participants_thread_id; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_thread_participants_thread_id ON public.thread_participants USING btree (thread_id);


--
-- Name: idx_thread_pending_activity; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_thread_pending_activity ON public.threads USING btree (updated_at DESC) WHERE (pending_updates = true);


--
-- Name: accounts accounts_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.accounts
    ADD CONSTRAINT accounts_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: messages messages_thread_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.messages
    ADD CONSTRAINT messages_thread_id_fkey FOREIGN KEY (thread_id) REFERENCES public.threads(id) ON DELETE RESTRICT;


--
-- Name: messages messages_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.messages
    ADD CONSTRAINT messages_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE RESTRICT;


--
-- Name: sessions sessions_account_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.sessions
    ADD CONSTRAINT sessions_account_id_fkey FOREIGN KEY (account_id) REFERENCES public.accounts(id) ON DELETE CASCADE;


--
-- Name: sessions sessions_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.sessions
    ADD CONSTRAINT sessions_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: thread_participants thread_participants_thread_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.thread_participants
    ADD CONSTRAINT thread_participants_thread_id_fkey FOREIGN KEY (thread_id) REFERENCES public.threads(id) ON DELETE CASCADE;


--
-- Name: thread_participants thread_participants_user_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.thread_participants
    ADD CONSTRAINT thread_participants_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: threads threads_owner_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.threads
    ADD CONSTRAINT threads_owner_id_fkey FOREIGN KEY (owner_id) REFERENCES public.users(id) ON DELETE RESTRICT;


--
-- Name: TABLE accounts; Type: ACL; Schema: public; Owner: postgres
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE public.accounts TO user_manager;


--
-- Name: TABLE messages; Type: ACL; Schema: public; Owner: postgres
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE public.messages TO thread_manager;


--
-- Name: TABLE thread_participants; Type: ACL; Schema: public; Owner: postgres
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE public.thread_participants TO thread_manager;


--
-- Name: TABLE threads; Type: ACL; Schema: public; Owner: postgres
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE public.threads TO thread_manager;


--
-- Name: TABLE users; Type: ACL; Schema: public; Owner: postgres
--

GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE public.users TO user_manager;
GRANT SELECT ON TABLE public.users TO thread_manager;


--
-- PostgreSQL database dump complete
--

