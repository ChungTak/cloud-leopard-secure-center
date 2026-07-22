-- expand: refresh token families and session revocation.

CREATE TABLE IF NOT EXISTS iam.refresh_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL,
    user_id UUID NOT NULL,
    family_id UUID NOT NULL,
    token_hash TEXT NOT NULL,
    session_version BIGINT NOT NULL,
    used BOOLEAN NOT NULL DEFAULT false,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY (user_id) REFERENCES iam.users(id)
);

CREATE UNIQUE INDEX IF NOT EXISTS refresh_tokens_tenant_hash_idx
    ON iam.refresh_tokens (tenant_id, token_hash);

CREATE INDEX IF NOT EXISTS refresh_tokens_family_idx
    ON iam.refresh_tokens (tenant_id, family_id);

CREATE INDEX IF NOT EXISTS refresh_tokens_user_idx
    ON iam.refresh_tokens (tenant_id, user_id);

GRANT SELECT, INSERT, UPDATE, DELETE ON iam.refresh_tokens TO clsc_app;

COMMENT ON TABLE iam.refresh_tokens IS 'Refresh token hashes grouped by family. Used to detect refresh token replay and support session revocation.';
