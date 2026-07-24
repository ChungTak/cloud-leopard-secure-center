-- Persist the per-target cleanup batch size alongside retention days.

ALTER TABLE audit.retention_policy
    ADD COLUMN IF NOT EXISTS max_batch_size BIGINT NOT NULL DEFAULT 1000
    CHECK (max_batch_size > 0);
