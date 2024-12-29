-- Create tables
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    email VARCHAR(255) NOT NULL UNIQUE
);

CREATE TABLE threads (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL
);

CREATE TABLE thread_participants (
    thread_id UUID REFERENCES threads(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    PRIMARY KEY (thread_id, user_id)
);

CREATE TABLE messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    thread_id UUID REFERENCES threads(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    content TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes
CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_messages_thread_id ON messages(thread_id);
CREATE INDEX idx_messages_created_at ON messages(created_at);
CREATE INDEX idx_thread_participants_user_id ON thread_participants(user_id);

-- Create roles and grant permissions
-- User Manager role
CREATE ROLE user_manager;
GRANT SELECT, INSERT, UPDATE, DELETE ON users TO user_manager;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO user_manager;

-- Thread Manager role
CREATE ROLE thread_manager;
GRANT SELECT ON users TO thread_manager;  -- Read-only access to users
GRANT SELECT, INSERT, UPDATE, DELETE ON threads TO thread_manager;
GRANT SELECT, INSERT, UPDATE, DELETE ON thread_participants TO thread_manager;
GRANT SELECT, INSERT, UPDATE, DELETE ON messages TO thread_manager;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO thread_manager;

-- Set default privileges to be restrictive
ALTER DEFAULT PRIVILEGES IN SCHEMA public 
    REVOKE ALL ON TABLES FROM PUBLIC;
ALTER DEFAULT PRIVILEGES IN SCHEMA public 
    REVOKE ALL ON SEQUENCES FROM PUBLIC; 