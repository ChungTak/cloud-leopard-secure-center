-- expand: create iam users and identities pattern.

CREATE TABLE IF NOT EXISTS iam.users (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    username TEXT NOT NULL,
    display_name TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'active', 'locked', 'disabled')),
    session_version BIGINT NOT NULL DEFAULT 1,
    revision BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    actor UUID,
    deleted_at TIMESTAMPTZ,
    CONSTRAINT username_not_empty CHECK (length(trim(username)) > 0),
    CONSTRAINT display_name_not_empty CHECK (length(trim(display_name)) > 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS users_tenant_username_unique
    ON iam.users (tenant_id, username)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS users_tenant_id_idx ON iam.users (tenant_id);

CREATE TABLE IF NOT EXISTS iam.user_identities (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    user_id UUID NOT NULL,
    identity_type TEXT NOT NULL CHECK (identity_type IN ('email', 'phone', 'username')),
    value TEXT NOT NULL,
    verified BOOLEAN NOT NULL DEFAULT false,
    revision BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    actor UUID,
    deleted_at TIMESTAMPTZ,
    CONSTRAINT user_identities_value_not_empty CHECK (length(trim(value)) > 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS user_identities_tenant_type_value_unique
    ON iam.user_identities (tenant_id, identity_type, value)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS user_identities_user_id_idx ON iam.user_identities (user_id);

GRANT SELECT, INSERT, UPDATE, DELETE ON iam.users TO clsc_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON iam.user_identities TO clsc_app;

COMMENT ON TABLE iam.users IS 'User aggregate root. Tenant-scoped, RLS-protected, soft-delete with revision.';
COMMENT ON TABLE iam.user_identities IS 'User external identities (email, phone, username). Each user may have multiple identities.';
