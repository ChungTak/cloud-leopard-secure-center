-- expand: inbox messages and background jobs

CREATE TABLE IF NOT EXISTS infra.inbox_messages (
    inbox_message_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID,
    consumer_id TEXT NOT NULL,
    message_id UUID NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'completed', 'failed')),
    result_digest TEXT,
    attempts INT NOT NULL DEFAULT 0,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE NULLS NOT DISTINCT (tenant_id, consumer_id, message_id)
);

CREATE INDEX IF NOT EXISTS inbox_messages_lookup
    ON infra.inbox_messages (tenant_id, consumer_id, message_id);

CREATE INDEX IF NOT EXISTS inbox_messages_expires_at
    ON infra.inbox_messages (expires_at);

CREATE TABLE IF NOT EXISTS infra.jobs (
    job_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID,
    queue TEXT NOT NULL,
    payload JSONB NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'completed', 'failed', 'cancelled')),
    revision BIGINT NOT NULL DEFAULT 1,
    lease_owner TEXT,
    lease_until TIMESTAMPTZ,
    attempts INT NOT NULL DEFAULT 0,
    max_attempts INT NOT NULL DEFAULT 5,
    next_run TIMESTAMPTZ NOT NULL DEFAULT now(),
    deadline TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS jobs_claim_idx
    ON infra.jobs (tenant_id, queue, next_run)
    WHERE status IN ('pending', 'running');

CREATE INDEX IF NOT EXISTS jobs_deadline_idx
    ON infra.jobs (tenant_id, deadline)
    WHERE deadline IS NOT NULL;

GRANT SELECT, INSERT, UPDATE, DELETE ON infra.inbox_messages TO clsc_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON infra.jobs TO clsc_app;

ALTER TABLE infra.inbox_messages ENABLE ROW LEVEL SECURITY;
ALTER TABLE infra.inbox_messages FORCE ROW LEVEL SECURITY;
ALTER TABLE infra.jobs ENABLE ROW LEVEL SECURITY;
ALTER TABLE infra.jobs FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS inbox_messages_isolation ON infra.inbox_messages;
CREATE POLICY inbox_messages_isolation ON infra.inbox_messages
    USING (tenant_id IS NOT DISTINCT FROM app.current_tenant_id())
    WITH CHECK (tenant_id IS NOT DISTINCT FROM app.current_tenant_id());

DROP POLICY IF EXISTS jobs_isolation ON infra.jobs;
CREATE POLICY jobs_isolation ON infra.jobs
    USING (tenant_id IS NOT DISTINCT FROM app.current_tenant_id())
    WITH CHECK (tenant_id IS NOT DISTINCT FROM app.current_tenant_id());
