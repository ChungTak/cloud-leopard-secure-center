-- expand: append-only audit records with monthly partitions

-- Dedicated role with INSERT/SELECT only; clsc_app can SET ROLE to it.
DO $$
BEGIN
    CREATE ROLE clsc_audit_writer NOLOGIN;
EXCEPTION
    WHEN unique_violation OR duplicate_object THEN
        NULL;
END
$$;

CREATE TABLE IF NOT EXISTS audit.records (
    id BIGSERIAL,
    tenant_id UUID NOT NULL,
    actor_type TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    action TEXT NOT NULL,
    target_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    result TEXT NOT NULL CHECK (result IN ('success', 'denied', 'failure')),
    risk TEXT NOT NULL CHECK (risk IN ('normal', 'high', 'critical')),
    request_id TEXT,
    trace_id TEXT,
    source_ip TEXT,
    before_digest TEXT,
    after_digest TEXT,
    occurred_at TIMESTAMPTZ NOT NULL,
    details JSONB NOT NULL,
    CONSTRAINT records_details_is_object CHECK (jsonb_typeof(details) = 'object'),
    CONSTRAINT records_details_size_limit CHECK (length(details::text) < 65536)
) PARTITION BY RANGE (occurred_at);

CREATE TABLE IF NOT EXISTS audit.records_default PARTITION OF audit.records DEFAULT;
COMMENT ON TABLE audit.records_default IS 'Default partition for audit.records. Rows here indicate a missing monthly partition or clock skew.';

CREATE INDEX IF NOT EXISTS records_tenant_occurred_idx ON audit.records (tenant_id, occurred_at);

-- Pre-create monthly partitions for the current month and the next 12 months.
DO $$
DECLARE
    start_date DATE := date_trunc('month', now())::date;
    p_start DATE;
    p_end DATE;
    p_name TEXT;
    i INT;
BEGIN
    FOR i IN 0..12 LOOP
        p_start := start_date + (i || ' months')::interval;
        p_end := p_start + interval '1 month';
        p_name := 'records_' || to_char(p_start, 'YYYY_MM');
        EXECUTE format(
            'CREATE TABLE IF NOT EXISTS audit.%I PARTITION OF audit.records FOR VALUES FROM (%L) TO (%L)',
            p_name, p_start, p_end
        );
    END LOOP;
END
$$;

GRANT USAGE ON SCHEMA audit TO clsc_audit_writer;
GRANT SELECT, INSERT ON audit.records TO clsc_audit_writer;
GRANT USAGE, SELECT ON SEQUENCE audit.records_id_seq TO clsc_audit_writer;
GRANT clsc_audit_writer TO clsc_app;

-- Remove UPDATE/DELETE privileges from the application role to enforce append-only writes.
-- Privilege changes on a partitioned table propagate to existing partitions.
REVOKE UPDATE, DELETE ON audit.records FROM clsc_app;
