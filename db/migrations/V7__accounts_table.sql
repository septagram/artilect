-- Create provider enum type
CREATE TYPE auth_provider AS ENUM ('Google');

-- Create accounts table
CREATE TABLE accounts (
    id uuid DEFAULT gen_random_uuid() NOT NULL PRIMARY KEY,
    login varchar(255) NOT NULL,
    provider auth_provider NOT NULL,
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    UNIQUE (provider, login)
);

-- Create index for provider -> login lookups
CREATE INDEX idx_accounts_provider_login ON accounts (provider, login);

-- Migrate existing users to accounts
INSERT INTO accounts (user_id, login, provider)
SELECT id, email, 'Google'
FROM users;

-- Remove Artilect's account
DELETE FROM accounts WHERE user_id = '00000000-0000-0000-0000-000000000000';

-- Remove email from users
ALTER TABLE users DROP COLUMN email;

-- Grant permissions
GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE accounts TO user_manager; 