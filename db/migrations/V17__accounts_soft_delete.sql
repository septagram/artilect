-- Change accounts table from updated_at to deleted_at (soft delete pattern)

-- Drop updated_at column
ALTER TABLE public.accounts DROP COLUMN updated_at;

-- Add deleted_at column for soft deletes (nullable)
ALTER TABLE public.accounts ADD COLUMN deleted_at timestamp with time zone;

-- Add index on deleted_at for efficient queries filtering out deleted accounts
CREATE INDEX idx_accounts_deleted_at ON public.accounts(deleted_at) WHERE deleted_at IS NULL;
