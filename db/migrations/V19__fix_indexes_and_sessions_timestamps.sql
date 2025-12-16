-- Fix duplicate and unnecessary indexes
-- Add created_at to sessions, remove deleted_at from sessions

-- ============================================================================
-- ACCOUNTS: Fix indexes
-- ============================================================================

-- Drop duplicate index (unique constraint already creates an index)
DROP INDEX IF EXISTS idx_accounts_provider_user;

-- Drop unnecessary deleted_at index
DROP INDEX IF EXISTS idx_accounts_deleted_at;

-- Replace unique constraint with partial unique index
-- (allows same provider+user_id after soft delete)
ALTER TABLE accounts DROP CONSTRAINT IF EXISTS accounts_provider_user_unique;
CREATE UNIQUE INDEX idx_accounts_provider_user_active
    ON accounts (provider, provider_user_id)
    WHERE deleted_at IS NULL;

-- ============================================================================
-- SESSIONS: Fix schema
-- ============================================================================

-- Remove deleted_at from sessions (sessions expire, not soft delete)
ALTER TABLE sessions DROP COLUMN IF EXISTS deleted_at;

-- Add created_at to sessions
ALTER TABLE sessions ADD COLUMN created_at timestamp with time zone DEFAULT CURRENT_TIMESTAMP NOT NULL;

-- Add index for querying account's sessions, sorted by expiration
CREATE INDEX idx_sessions_account_expires
    ON sessions (account_id, expires_at DESC);
