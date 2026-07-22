-- expand: API keys and MFA factors.

CREATE TABLE IF NOT EXISTS iam.api_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    owner_id UUID NOT NULL,
    name TEXT NOT NULL,
    scopes TEXT[] NOT NULL DEFAULT '{}',
    allowed_sources TEXT[] NOT NULL DEFAULT '{}',
    token_hash TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used_at TIMESTAMPTZ,
    FOREIGN KEY (owner_id) REFERENCES iam.users(id)
);

CREATE UNIQUE INDEX IF NOT EXISTS api_keys_tenant_hash_idx
    ON iam.api_keys (tenant_id, token_hash);

CREATE INDEX IF NOT EXISTS api_keys_owner_idx
    ON iam.api_keys (tenant_id, owner_id);

CREATE TABLE IF NOT EXISTS iam.mfa_factors (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    user_id UUID NOT NULL,
    factor_type TEXT NOT NULL,
    secret_ref TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT false,
    verified_at TIMESTAMPTZ,
    recovery_code_hashes TEXT[] NOT NULL DEFAULT '{}',
    recovery_code_used BOOLEAN[] NOT NULL DEFAULT '{}',
    last_used_step BIGINT,
    last_used_code TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (user_id) REFERENCES iam.users(id)
);

CREATE UNIQUE INDEX IF NOT EXISTS mfa_factors_active_user_idx
    ON iam.mfa_factors (tenant_id, user_id) WHERE enabled = true;

GRANT SELECT, INSERT, UPDATE, DELETE ON iam.api_keys TO clsc_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON iam.mfa_factors TO clsc_app;

COMMENT ON TABLE iam.api_keys IS 'API keys for service accounts and integrations. Only token hashes are stored.';
COMMENT ON TABLE iam.mfa_factors IS 'MFA factors. Secrets are referenced, not stored; recovery code hashes are stored.';
