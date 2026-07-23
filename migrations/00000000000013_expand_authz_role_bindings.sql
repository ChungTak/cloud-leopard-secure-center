-- expand: role bindings and resource set scopes

CREATE TABLE IF NOT EXISTS authz.role_bindings (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    principal_id UUID NOT NULL,
    role_id UUID NOT NULL,
    scope_type TEXT NOT NULL,
    scope_ref UUID,
    valid_from TIMESTAMPTZ NOT NULL,
    valid_until TIMESTAMPTZ,
    revision BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    actor UUID,
    deleted_at TIMESTAMPTZ,
    CONSTRAINT role_bindings_tenant_fk
        FOREIGN KEY (tenant_id) REFERENCES org.tenants(id),
    CONSTRAINT role_bindings_role_fk
        FOREIGN KEY (role_id) REFERENCES authz.roles(id),
    CONSTRAINT role_bindings_scope_ref_when_needed
        CHECK ((scope_type IN ('organization_subtree', 'area_subtree')) = (scope_ref IS NOT NULL))
);

CREATE INDEX IF NOT EXISTS role_bindings_principal_idx
    ON authz.role_bindings (tenant_id, principal_id)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS role_bindings_role_idx
    ON authz.role_bindings (tenant_id, role_id)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS authz.role_binding_resources (
    tenant_id UUID NOT NULL,
    role_binding_id UUID NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id UUID NOT NULL,
    PRIMARY KEY (tenant_id, role_binding_id, resource_type, resource_id),
    CONSTRAINT role_binding_resources_binding_fk
        FOREIGN KEY (role_binding_id) REFERENCES authz.role_bindings(id),
    CONSTRAINT role_binding_resources_tenant_fk
        FOREIGN KEY (tenant_id) REFERENCES org.tenants(id)
);

CREATE INDEX IF NOT EXISTS role_binding_resources_resource_idx
    ON authz.role_binding_resources (resource_type, resource_id);

GRANT SELECT, INSERT, UPDATE, DELETE ON authz.role_bindings TO clsc_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON authz.role_binding_resources TO clsc_app;
