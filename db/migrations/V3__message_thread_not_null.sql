-- Make messages.thread_id NOT NULL
ALTER TABLE messages 
    ALTER COLUMN thread_id SET NOT NULL,
    DROP CONSTRAINT messages_thread_id_fkey,
    ADD CONSTRAINT messages_thread_id_fkey 
        FOREIGN KEY (thread_id) 
        REFERENCES threads(id) 
        ON DELETE RESTRICT; 