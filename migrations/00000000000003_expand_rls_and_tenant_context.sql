-- expand: tenant context helpers and row-level security

CREATE OR REPLACE FUNCTION app.set_tenant_context(p_tenant_id UUID)
RETURNS void AS $$
BEGIN
    PERFORM set_config('app.tenant_id', p_tenant_id::text, true);
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION app.current_tenant_id()
RETURNS UUID AS $$
    SELECT CASE
        WHEN current_setting('app.tenant_id', true) IS NULL
            OR current_setting('app.tenant_id', true) = '' THEN NULL
        ELSE current_setting('app.tenant_id', true)::UUID
    END;
$$ LANGUAGE sql STABLE;

-- Enable RLS on the authoritative tenant table. A tenant can only see itself.
ALTER TABLE iam.tenants ENABLE ROW LEVEL SECURITY;
ALTER TABLE iam.tenants FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation ON iam.tenants;
CREATE POLICY tenant_isolation ON iam.tenants
    USING (id = app.current_tenant_id())
    WITH CHECK (id = app.current_tenant_id());

-- Event trigger that automatically enables RLS for future tenant-scoped tables
-- (tables that contain a `tenant_id` column) in the domain schemas.
CREATE OR REPLACE FUNCTION infra.auto_enable_tenant_rls()
RETURNS event_trigger AS $$
DECLARE
    cmd record;
    has_tenant_id boolean;
BEGIN
    FOR cmd IN
        SELECT schema_name, split_part(object_identity, '.', 2) AS table_name
        FROM pg_event_trigger_ddl_commands()
        WHERE object_type = 'table'
          AND schema_name IN ('iam', 'org', 'authz', 'resource', 'audit', 'config')
    LOOP
        SELECT EXISTS (
            SELECT 1
            FROM information_schema.columns
            WHERE table_schema = cmd.schema_name
              AND table_name = cmd.table_name
              AND column_name = 'tenant_id'
        ) INTO has_tenant_id;

        IF has_tenant_id THEN
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
            EXECUTE format(
                'GRANT SELECT, INSERT, UPDATE, DELETE ON %I.%I TO clsc_app;',
                cmd.schema_name, cmd.table_name
            );
        END IF;
    END LOOP;
END;
$$ LANGUAGE plpgsql;

DROP EVENT TRIGGER IF EXISTS auto_enable_tenant_rls;
CREATE EVENT TRIGGER auto_enable_tenant_rls
    ON ddl_command_end
    WHEN TAG IN ('CREATE TABLE')
    EXECUTE FUNCTION infra.auto_enable_tenant_rls();
