-- expand: credential storage and login-attempt rate-limit indexes.

CREATE TABLE IF NOT EXISTS iam.credentials (
    user_id UUID NOT NULL,
    tenant_id UUID NOT NULL,
    credential_type TEXT NOT NULL CHECK (credential_type IN ('password_hash')),
    value TEXT NOT NULL,
    parameters TEXT NOT NULL,
    revision BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, credential_type),
    FOREIGN KEY (user_id) REFERENCES iam.users(id)
);

CREATE UNIQUE INDEX IF NOT EXISTS credentials_user_type_unique
    ON iam.credentials (user_id, credential_type)
    WHERE credential_type = 'password_hash';

CREATE INDEX IF NOT EXISTS credentials_user_id_idx ON iam.credentials (user_id);

GRANT SELECT, INSERT, UPDATE, DELETE ON iam.credentials TO clsc_app;

-- Indexes to support account and source based rate limiting on login attempts.
CREATE INDEX IF NOT EXISTS login_attempts_account_window_idx
    ON audit.login_attempts (tenant_id, identity, success, created_at DESC);

CREATE INDEX IF NOT EXISTS login_attempts_source_window_idx
    ON audit.login_attempts (tenant_id, ip_address, success, created_at DESC);

COMMENT ON TABLE iam.credentials IS 'User authentication credentials. Each user may have one credential per type. Values are PHC strings.';

-- Grant clsc_app access to existing sequences in tenant schemas.
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA iam, audit, org, authz, resource, config TO clsc_app;

-- Automatically grant clsc_app access to any future sequences.
CREATE OR REPLACE FUNCTION infra.auto_grant_sequence_to_clsc_app()
RETURNS event_trigger AS $$
DECLARE
    seq record;
BEGIN
    FOR seq IN
        SELECT d.schema_name, split_part(d.object_identity, '.', 2) AS sequence_name
        FROM pg_event_trigger_ddl_commands() d
        WHERE d.object_type = 'sequence'
          AND d.schema_name IN ('iam', 'org', 'authz', 'resource', 'audit', 'config')
    LOOP
        EXECUTE format(
            'GRANT USAGE, SELECT ON %I.%I TO clsc_app;',
            seq.schema_name, seq.sequence_name
        );
    END LOOP;
END;
$$ LANGUAGE plpgsql;

DROP EVENT TRIGGER IF EXISTS auto_grant_sequence_to_clsc_app;
CREATE EVENT TRIGGER auto_grant_sequence_to_clsc_app
    ON ddl_command_end
    WHEN TAG IN ('CREATE SEQUENCE')
    EXECUTE FUNCTION infra.auto_grant_sequence_to_clsc_app();
