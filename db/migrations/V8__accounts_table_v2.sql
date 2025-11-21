-- Drop existing table and enum
DROP TABLE accounts;
DROP TYPE auth_provider;

-- Create provider enum type
CREATE TYPE auth_provider AS ENUM ('Telegram');

-- Create accounts table
CREATE TABLE accounts (
    id uuid DEFAULT gen_random_uuid() NOT NULL PRIMARY KEY,
    provider auth_provider NOT NULL,
    provider_user_id varchar(255) NOT NULL,
    provider_username varchar(255),
    provider_display_name varchar(255),
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at timestamptz DEFAULT CURRENT_TIMESTAMP NOT NULL,
    updated_at timestamptz DEFAULT CURRENT_TIMESTAMP NOT NULL,
    CONSTRAINT accounts_provider_user_unique UNIQUE (provider, provider_user_id)
);

-- Create index for provider -> user lookups
CREATE INDEX idx_accounts_provider_user ON accounts (provider, provider_user_id);

-- Grant permissions
GRANT SELECT,INSERT,DELETE,UPDATE ON TABLE accounts TO user_manager;
