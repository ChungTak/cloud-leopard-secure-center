-- expand: organization unit tree and closure table

CREATE TABLE IF NOT EXISTS org.organization_units (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    parent_id UUID,
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    revision BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    actor UUID,
    deleted_at TIMESTAMPTZ,
    CONSTRAINT organization_units_tenant_fk
        FOREIGN KEY (tenant_id) REFERENCES org.tenants(id)
);

-- Composite unique key lets foreign keys enforce tenant-scoped parent references.
ALTER TABLE org.organization_units
    DROP CONSTRAINT IF EXISTS organization_units_tenant_id_unique;
ALTER TABLE org.organization_units
    ADD CONSTRAINT organization_units_tenant_id_unique UNIQUE (tenant_id, id);

ALTER TABLE org.organization_units
    DROP CONSTRAINT IF EXISTS organization_units_parent_tenant_fk;
ALTER TABLE org.organization_units
    ADD CONSTRAINT organization_units_parent_tenant_fk
        FOREIGN KEY (tenant_id, parent_id) REFERENCES org.organization_units(tenant_id, id);

CREATE UNIQUE INDEX IF NOT EXISTS organization_units_tenant_code_unique
    ON org.organization_units (tenant_id, code)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS organization_units_tenant_parent_idx
    ON org.organization_units (tenant_id, parent_id);

CREATE TABLE IF NOT EXISTS org.organization_unit_closure (
    tenant_id UUID NOT NULL,
    ancestor_id UUID NOT NULL,
    descendant_id UUID NOT NULL,
    depth INT NOT NULL,
    PRIMARY KEY (tenant_id, ancestor_id, descendant_id),
    CONSTRAINT organization_unit_closure_tenant_fk
        FOREIGN KEY (tenant_id) REFERENCES org.tenants(id),
    CONSTRAINT organization_unit_closure_ancestor_tenant_fk
        FOREIGN KEY (tenant_id, ancestor_id) REFERENCES org.organization_units(tenant_id, id),
    CONSTRAINT organization_unit_closure_descendant_tenant_fk
        FOREIGN KEY (tenant_id, descendant_id) REFERENCES org.organization_units(tenant_id, id)
);

CREATE INDEX IF NOT EXISTS organization_unit_closure_ancestor_idx
    ON org.organization_unit_closure (tenant_id, ancestor_id, depth);

CREATE INDEX IF NOT EXISTS organization_unit_closure_descendant_idx
    ON org.organization_unit_closure (tenant_id, descendant_id);

GRANT SELECT, INSERT, UPDATE, DELETE ON org.organization_units TO clsc_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON org.organization_unit_closure TO clsc_app;
