-- expand: retention policies, legal holds, and cleanup worker

-- Create a dedicated cleanup worker that can bypass RLS and delete audit data.
DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'clsc_cleanup_worker') THEN
        CREATE ROLE clsc_cleanup_worker NOLOGIN BYPASSRLS;
    END IF;
END $$;
-- Allow the application role to switch into the cleanup worker without inheriting its DELETE privileges.
GRANT clsc_cleanup_worker TO clsc_app WITH INHERIT FALSE;

-- Replace the tenant-RLS trigger so that audit tables do not receive UPDATE/DELETE grants from clsc_app.
CREATE OR REPLACE FUNCTION infra.auto_enable_tenant_rls()
RETURNS event_trigger AS $$
DECLARE
    cmd record;
    has_tenant_id boolean;
BEGIN
    FOR cmd IN
        SELECT d.schema_name, split_part(d.object_identity, '.', 2) AS table_name, c.relispartition
        FROM pg_event_trigger_ddl_commands() d
        JOIN pg_class c ON c.oid = d.objid
        WHERE d.object_type = 'table'
          AND d.schema_name IN ('iam', 'org', 'authz', 'resource', 'audit', 'config')
    LOOP
        SELECT EXISTS (
            SELECT 1
            FROM information_schema.columns
            WHERE table_schema = cmd.schema_name
              AND table_name = cmd.table_name
              AND column_name = 'tenant_id'
        ) INTO has_tenant_id;

        IF has_tenant_id THEN
            -- Audit tables are cleaned by the dedicated worker role; the app only inserts/reads.
            IF cmd.schema_name = 'audit' THEN
                EXECUTE format(
                    'GRANT SELECT, INSERT ON %I.%I TO clsc_app;',
                    cmd.schema_name, cmd.table_name
                );
            ELSE
                EXECUTE format(
                    'GRANT SELECT, INSERT, UPDATE, DELETE ON %I.%I TO clsc_app;',
                    cmd.schema_name, cmd.table_name
                );
            END IF;

            IF NOT cmd.relispartition THEN
                EXECUTE format(
                    'ALTER TABLE %I.%I ENABLE ROW LEVEL SECURITY, FORCE ROW LEVEL SECURITY;',
                    cmd.schema_name, cmd.table_name
                );
                EXECUTE format(
                    'DROP POLICY IF EXISTS tenant_isolation ON %I.%I;',
                    cmd.schema_name, cmd.table_name
                );
                EXECUTE format(
                    'CREATE POLICY tenant_isolation ON %I.%I USING (tenant_id = app.current_tenant_id()) WITH CHECK (tenant_id = app.current_tenant_id());',
                    cmd.schema_name, cmd.table_name
                );
            END IF;
        END IF;
    END LOOP;
END;
$$ LANGUAGE plpgsql;

GRANT USAGE ON SCHEMA audit TO clsc_cleanup_worker;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA audit TO clsc_cleanup_worker;
GRANT SELECT, USAGE ON ALL SEQUENCES IN SCHEMA audit TO clsc_cleanup_worker;
ALTER DEFAULT PRIVILEGES IN SCHEMA audit GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO clsc_cleanup_worker;
ALTER DEFAULT PRIVILEGES IN SCHEMA audit GRANT SELECT, USAGE ON SEQUENCES TO clsc_cleanup_worker;

-- Ensure the application role cannot UPDATE/DELETE any existing audit tables.
DO $$
DECLARE
    t record;
BEGIN
    FOR t IN
        SELECT schemaname, tablename
        FROM pg_tables
        WHERE schemaname = 'audit'
    LOOP
        EXECUTE format('REVOKE UPDATE, DELETE ON %I.%I FROM clsc_app', t.schemaname, t.tablename);
    END LOOP;
END $$;

CREATE TABLE IF NOT EXISTS audit.retention_policy (
    target TEXT NOT NULL,
    tenant_id UUID,
    days BIGINT NOT NULL CHECK (days > 0),
    CONSTRAINT retention_policy_target_tenant_unique UNIQUE (target, tenant_id)
);

CREATE TABLE IF NOT EXISTS audit.legal_holds (
    resource_type TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    held_until TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (resource_type, resource_id)
);

ALTER TABLE audit.cleanup_checkpoint
    ADD COLUMN IF NOT EXISTS lease_until TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS worker_id TEXT;

GRANT SELECT, INSERT, UPDATE, DELETE ON audit.retention_policy TO clsc_cleanup_worker;
GRANT SELECT, INSERT, UPDATE, DELETE ON audit.legal_holds TO clsc_cleanup_worker;
GRANT SELECT, INSERT, UPDATE, DELETE ON audit.cleanup_checkpoint TO clsc_cleanup_worker;

-- Bounded, resumable cleanup batch for a single partition. Returns rows deleted.
-- When p_resource_type_col and p_resource_id_col are provided, rows matching an active legal hold are skipped.
CREATE OR REPLACE FUNCTION audit.cleanup_batch(
    p_table TEXT,
    p_partition TEXT,
    p_cutoff TIMESTAMPTZ,
    p_batch_size BIGINT,
    p_timestamp_column TEXT DEFAULT 'created_at',
    p_resource_type_col TEXT DEFAULT NULL,
    p_resource_id_col TEXT DEFAULT NULL
)
RETURNS BIGINT AS $$
DECLARE
    v_deleted BIGINT := 0;
    v_batch_max BIGINT;
    v_last_id BIGINT := 0;
    v_sql TEXT;
BEGIN
    SELECT COALESCE(last_id, 0) INTO v_last_id
    FROM audit.cleanup_checkpoint
    WHERE table_name = p_table AND partition_name = p_partition;

    IF v_last_id IS NULL THEN
        v_last_id := 0;
    END IF;

    IF p_resource_type_col IS NOT NULL AND p_resource_id_col IS NOT NULL THEN
        v_sql := format(
            'WITH candidates AS (
                SELECT id FROM audit.%I
                WHERE id > $1 AND %I < $2
                  AND NOT EXISTS (
                      SELECT 1 FROM audit.legal_holds h
                      WHERE h.resource_type = %I
                        AND h.resource_id = %I::text
                        AND h.held_until > now()
                  )
                ORDER BY id
                LIMIT $3
            ),
            del AS (
                DELETE FROM audit.%I t
                USING candidates c
                WHERE t.id = c.id
                RETURNING t.id
            ) SELECT count(*), max(id) FROM del',
            p_partition, p_timestamp_column, p_resource_type_col, p_resource_id_col, p_partition
        );
    ELSE
        v_sql := format(
            'WITH candidates AS (
                SELECT id FROM audit.%I
                WHERE id > $1 AND %I < $2
                ORDER BY id
                LIMIT $3
            ),
            del AS (
                DELETE FROM audit.%I t
                USING candidates c
                WHERE t.id = c.id
                RETURNING t.id
            ) SELECT count(*), max(id) FROM del',
            p_partition, p_timestamp_column, p_partition
        );
    END IF;

    EXECUTE v_sql INTO v_deleted, v_batch_max USING v_last_id, p_cutoff, p_batch_size;

    IF v_deleted > 0 THEN
        UPDATE audit.cleanup_checkpoint
        SET last_id = v_batch_max, updated_at = now()
        WHERE table_name = p_table AND partition_name = p_partition;
    END IF;

    RETURN COALESCE(v_deleted, 0);
END;
$$ LANGUAGE plpgsql;
