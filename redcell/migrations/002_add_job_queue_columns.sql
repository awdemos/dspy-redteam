ALTER TABLE jobs ADD COLUMN claimed_at TIMESTAMPTZ;
ALTER TABLE jobs ADD COLUMN worker_id TEXT;
ALTER TABLE jobs ADD COLUMN attempts INTEGER NOT NULL DEFAULT 0;
ALTER TABLE jobs ADD COLUMN max_attempts INTEGER NOT NULL DEFAULT 3;
ALTER TABLE jobs ADD COLUMN run_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP;

-- Ensure existing jobs have a sensible status
UPDATE jobs SET status = 'queued' WHERE status = 'running';
