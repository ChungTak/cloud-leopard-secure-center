-- expand: authorization roles and permissions

CREATE TABLE IF NOT EXISTS authz.permissions (
    key TEXT PRIMARY KEY,
    scope TEXT NOT NULL CHECK (scope IN ('platform', 'tenant'))
);

INSERT INTO authz.permissions (key, scope) VALUES
    ('platform:tenant:read', 'platform'),
    ('platform:tenant:write', 'platform'),
    ('tenant:user:read', 'tenant'),
    ('tenant:user:write', 'tenant'),
    ('tenant:role:read', 'tenant'),
    ('tenant:role:write', 'tenant'),
    ('tenant:organization:read', 'tenant'),
    ('tenant:organization:write', 'tenant'),
    ('tenant:site:read', 'tenant'),
    ('tenant:site:write', 'tenant'),
    ('tenant:area:read', 'tenant'),
    ('tenant:area:write', 'tenant')
ON CONFLICT (key) DO NOTHING;

CREATE TABLE IF NOT EXISTS authz.roles (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    name TEXT NOT NULL,
    is_builtin BOOLEAN NOT NULL DEFAULT false,
    revision BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    actor UUID,
    deleted_at TIMESTAMPTZ,
    CONSTRAINT roles_tenant_fk
        FOREIGN KEY (tenant_id) REFERENCES org.tenants(id),
    UNIQUE (tenant_id, name)
);

CREATE INDEX IF NOT EXISTS roles_tenant_name_idx
    ON authz.roles (tenant_id, name)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS authz.role_permissions (
    tenant_id UUID NOT NULL,
    role_id UUID NOT NULL,
    permission_key TEXT NOT NULL,
    PRIMARY KEY (tenant_id, role_id, permission_key),
    CONSTRAINT role_permissions_tenant_fk
        FOREIGN KEY (tenant_id) REFERENCES org.tenants(id),
    CONSTRAINT role_permissions_role_fk
        FOREIGN KEY (role_id) REFERENCES authz.roles(id),
    CONSTRAINT role_permissions_permission_fk
        FOREIGN KEY (permission_key) REFERENCES authz.permissions(key)
);

CREATE INDEX IF NOT EXISTS role_permissions_role_idx
    ON authz.role_permissions (role_id, permission_key);

GRANT SELECT, INSERT, UPDATE, DELETE ON authz.roles TO clsc_app;
GRANT SELECT ON authz.permissions TO clsc_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON authz.role_permissions TO clsc_app;
