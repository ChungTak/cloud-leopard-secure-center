-- expand: idempotency records and outbox messages

CREATE TABLE IF NOT EXISTS infra.idempotency_records (
    idempotency_record_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID,
    principal_id UUID NOT NULL,
    endpoint_scope TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    request_digest TEXT NOT NULL,
    response_status INT,
    response_body TEXT,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE NULLS NOT DISTINCT (tenant_id, principal_id, endpoint_scope, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idempotency_records_expires_at
    ON infra.idempotency_records (expires_at);

CREATE TABLE IF NOT EXISTS infra.outbox_messages (
    message_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID,
    aggregate_type TEXT NOT NULL,
    aggregate_id TEXT NOT NULL,
    aggregate_sequence BIGINT NOT NULL,
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL,
    available_at TIMESTAMPTZ NOT NULL,
    attempts INT NOT NULL DEFAULT 0,
    published_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS outbox_messages_unpublished
    ON infra.outbox_messages (tenant_id, available_at)
    WHERE published_at IS NULL;

GRANT SELECT, INSERT, UPDATE, DELETE ON infra.idempotency_records TO clsc_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON infra.outbox_messages TO clsc_app;

ALTER TABLE infra.idempotency_records ENABLE ROW LEVEL SECURITY;
ALTER TABLE infra.idempotency_records FORCE ROW LEVEL SECURITY;
ALTER TABLE infra.outbox_messages ENABLE ROW LEVEL SECURITY;
ALTER TABLE infra.outbox_messages FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS idempotency_records_isolation ON infra.idempotency_records;
CREATE POLICY idempotency_records_isolation ON infra.idempotency_records
    USING (tenant_id IS NOT DISTINCT FROM app.current_tenant_id())
    WITH CHECK (tenant_id IS NOT DISTINCT FROM app.current_tenant_id());

DROP POLICY IF EXISTS outbox_messages_isolation ON infra.outbox_messages;
CREATE POLICY outbox_messages_isolation ON infra.outbox_messages
    USING (tenant_id IS NOT DISTINCT FROM app.current_tenant_id())
    WITH CHECK (tenant_id IS NOT DISTINCT FROM app.current_tenant_id());
