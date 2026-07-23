-- expand: configuration definitions and scoped values

CREATE TABLE IF NOT EXISTS config.definitions (
    config_key TEXT PRIMARY KEY,
    value_type TEXT NOT NULL CHECK (value_type IN ('string', 'integer', 'boolean', 'duration', 'secret', 'json')),
    schema JSONB,
    default_value TEXT NOT NULL,
    sensitive BOOLEAN NOT NULL,
    dynamic BOOLEAN NOT NULL
);

CREATE TABLE IF NOT EXISTS config.values (
    config_value_id UUID PRIMARY KEY,
    tenant_id UUID,
    scope_type TEXT NOT NULL CHECK (scope_type IN ('platform', 'tenant', 'module')),
    scope_id TEXT,
    config_key TEXT NOT NULL REFERENCES config.definitions(config_key),
    value JSONB,
    raw_value TEXT NOT NULL,
    secret_ref TEXT,
    revision BIGINT NOT NULL DEFAULT 0,
    CONSTRAINT value_or_secret CHECK (
        (secret_ref IS NULL AND value IS NOT NULL) OR
        (secret_ref IS NOT NULL AND value IS NULL)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS config_values_scope_unique
    ON config.values (tenant_id, scope_type, COALESCE(scope_id, ''), config_key);

-- Definitions are global and readable/writable by the app role.
GRANT SELECT, INSERT, UPDATE, DELETE ON config.definitions TO clsc_app;

-- Values are tenant-scoped but platform rows (tenant_id IS NULL) must be visible to all tenants.
GRANT SELECT, INSERT, UPDATE, DELETE ON config.values TO clsc_app;

ALTER TABLE config.values ENABLE ROW LEVEL SECURITY;
ALTER TABLE config.values FORCE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS tenant_isolation ON config.values;
CREATE POLICY config_value_isolation ON config.values
    USING (tenant_id IS NULL OR tenant_id = app.current_tenant_id())
    WITH CHECK (tenant_id IS NULL OR tenant_id = app.current_tenant_id());
