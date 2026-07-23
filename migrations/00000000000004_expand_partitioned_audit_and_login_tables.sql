-- expand: partitioned audit events and login attempts with bounded cleanup.

CREATE TABLE IF NOT EXISTS audit.events (
    id BIGSERIAL,
    tenant_id UUID NOT NULL,
    event_type TEXT NOT NULL,
    actor UUID,
    resource_type TEXT,
    resource_id UUID,
    details JSONB,
    CONSTRAINT events_details_is_object CHECK (details IS NULL OR jsonb_typeof(details) = 'object'),
    CONSTRAINT events_details_size_limit CHECK (details IS NULL OR length(details::text) < 65536),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
) PARTITION BY RANGE (created_at);

CREATE TABLE IF NOT EXISTS audit.events_default PARTITION OF audit.events DEFAULT;
COMMENT ON TABLE audit.events_default IS 'Default partition for audit.events. Used only for alerting; rows here indicate a missing monthly partition or clock skew.';

CREATE TABLE IF NOT EXISTS audit.login_attempts (
    id BIGSERIAL,
    tenant_id UUID NOT NULL,
    identity TEXT NOT NULL,
    success BOOLEAN NOT NULL,
    ip_address TEXT,
    user_agent TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
) PARTITION BY RANGE (created_at);

CREATE TABLE IF NOT EXISTS audit.login_attempts_default PARTITION OF audit.login_attempts DEFAULT;
COMMENT ON TABLE audit.login_attempts_default IS 'Default partition for audit.login_attempts. Used only for alerting; rows here indicate a missing monthly partition or clock skew.';

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

        p_name := 'events_' || to_char(p_start, 'YYYY_MM');
        EXECUTE format(
            'CREATE TABLE IF NOT EXISTS audit.%I PARTITION OF audit.events FOR VALUES FROM (%L) TO (%L)',
            p_name, p_start, p_end
        );

        p_name := 'login_attempts_' || to_char(p_start, 'YYYY_MM');
        EXECUTE format(
            'CREATE TABLE IF NOT EXISTS audit.%I PARTITION OF audit.login_attempts FOR VALUES FROM (%L) TO (%L)',
            p_name, p_start, p_end
        );
    END LOOP;
END
$$;

-- Cleanup checkpoint for resumable, bounded batch operations.
CREATE TABLE IF NOT EXISTS audit.cleanup_checkpoint (
    table_name TEXT NOT NULL,
    partition_name TEXT NOT NULL,
    cutoff TIMESTAMPTZ NOT NULL,
    last_id BIGINT NOT NULL DEFAULT 0,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    PRIMARY KEY (table_name, partition_name)
);

-- Purge old rows from a partition in bounded batches, updating a checkpoint so the
-- work can be resumed after an interruption. Returns the number of rows deleted.
CREATE OR REPLACE FUNCTION audit.purge_partition(
    p_table TEXT,
    p_partition TEXT,
    p_cutoff TIMESTAMPTZ,
    p_batch_size BIGINT DEFAULT 1000
)
RETURNS BIGINT AS $$
DECLARE
    v_deleted BIGINT := 0;
    v_batch_count BIGINT;
    v_batch_max BIGINT;
    v_last_id BIGINT := 0;
    v_sql TEXT;
BEGIN
    INSERT INTO audit.cleanup_checkpoint (table_name, partition_name, cutoff)
    VALUES (p_table, p_partition, p_cutoff)
    ON CONFLICT (table_name, partition_name)
    DO UPDATE SET started_at = now(), completed_at = NULL;

    SELECT last_id INTO v_last_id
    FROM audit.cleanup_checkpoint
    WHERE table_name = p_table AND partition_name = p_partition;

    v_sql := format(
        'WITH del AS (
            DELETE FROM %I.%I
            WHERE id IN (
                SELECT id FROM %I.%I
                WHERE id > $1 AND created_at < $2
                ORDER BY id
                LIMIT $3
            )
            RETURNING id
        ) SELECT count(*), max(id) FROM del',
        'audit', p_partition, 'audit', p_partition
    );

    LOOP
        EXECUTE v_sql INTO v_batch_count, v_batch_max USING v_last_id, p_cutoff, p_batch_size;
        EXIT WHEN v_batch_count IS NULL OR v_batch_count = 0;

        v_deleted := v_deleted + v_batch_count;
        v_last_id := v_batch_max;

        UPDATE audit.cleanup_checkpoint
        SET last_id = v_last_id, updated_at = now()
        WHERE table_name = p_table AND partition_name = p_partition;
    END LOOP;

    UPDATE audit.cleanup_checkpoint
    SET completed_at = now(), updated_at = now()
    WHERE table_name = p_table AND partition_name = p_partition;

    RETURN v_deleted;
END;
$$ LANGUAGE plpgsql;
