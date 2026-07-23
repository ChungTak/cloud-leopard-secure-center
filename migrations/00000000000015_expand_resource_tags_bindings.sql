-- expand: tags and external bindings

CREATE TABLE IF NOT EXISTS resource.tags (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id UUID NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    revision BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    actor UUID,
    deleted_at TIMESTAMPTZ,
    CONSTRAINT tags_tenant_fk
        FOREIGN KEY (tenant_id) REFERENCES org.tenants(id),
    UNIQUE (tenant_id, resource_type, resource_id, key)
);

CREATE INDEX IF NOT EXISTS tags_resource_idx
    ON resource.tags (resource_type, resource_id)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS tags_tenant_key_idx
    ON resource.tags (tenant_id, key)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS resource.external_bindings (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id UUID NOT NULL,
    external_ref TEXT NOT NULL,
    external_kind TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('pending', 'active', 'stale', 'conflict', 'disabled')),
    activated_at TIMESTAMPTZ,
    revision BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    actor UUID,
    deleted_at TIMESTAMPTZ,
    CONSTRAINT external_bindings_tenant_fk
        FOREIGN KEY (tenant_id) REFERENCES org.tenants(id)
);

CREATE INDEX IF NOT EXISTS external_bindings_resource_idx
    ON resource.external_bindings (resource_type, resource_id)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS external_bindings_external_ref_idx
    ON resource.external_bindings (external_kind, external_ref)
    WHERE deleted_at IS NULL;

GRANT SELECT, INSERT, UPDATE, DELETE ON resource.tags TO clsc_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON resource.external_bindings TO clsc_app;
