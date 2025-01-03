-- Make messages.user_id NOT NULL and change ON DELETE behavior
ALTER TABLE messages 
    ALTER COLUMN user_id SET NOT NULL,
    DROP CONSTRAINT messages_user_id_fkey,
    ADD CONSTRAINT messages_user_id_fkey 
        FOREIGN KEY (user_id) 
        REFERENCES users(id) 
        ON DELETE RESTRICT;

-- Make messages.updated_at nullable
ALTER TABLE messages 
    ALTER COLUMN updated_at DROP NOT NULL,
    ALTER COLUMN updated_at DROP DEFAULT;

-- Make thread name nullable
ALTER TABLE threads
    ALTER COLUMN name DROP NOT NULL;

-- Replace thread_id index with composite index
DROP INDEX idx_messages_thread_id;
CREATE INDEX idx_messages_thread_created 
    ON messages(thread_id, created_at);

-- Add thread_id index to thread_participants
CREATE INDEX idx_thread_participants_thread_id 
    ON thread_participants(thread_id);

-- Replace old user_id-only index with composite one
DROP INDEX idx_thread_participants_user_id;
CREATE INDEX idx_thread_participants_both
    ON thread_participants(user_id, thread_id); 