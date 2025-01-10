-- Add owner_id and timestamps to threads
ALTER TABLE threads
    ADD COLUMN owner_id UUID NOT NULL DEFAULT '00000000-0000-0000-0000-000000000000'
        REFERENCES users(id) ON DELETE RESTRICT,
    ADD COLUMN created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    ADD COLUMN updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    ADD COLUMN pending_updates BOOLEAN NOT NULL DEFAULT TRUE;

-- Create index for general activity sorting
CREATE INDEX idx_thread_activity ON threads(updated_at DESC);

-- Create partial index for pending updates
CREATE INDEX idx_thread_pending_activity 
    ON threads(updated_at DESC) 
    WHERE pending_updates = true;

-- Backfill timestamps for existing threads
WITH message_stats AS (
    SELECT 
        thread_id,
        MIN(created_at) as first_message,
        MAX(created_at) as last_message
    FROM messages 
    GROUP BY thread_id
)
UPDATE threads t
SET 
    created_at = COALESCE(ms.first_message, '1970-01-01 00:00:00+00'::timestamptz),
    updated_at = COALESCE(ms.last_message, '1970-01-01 00:00:00+00'::timestamptz)
FROM message_stats ms
WHERE t.id = ms.thread_id;
