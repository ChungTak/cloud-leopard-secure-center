-- expand: spatial tree (sites, buildings, floors, areas, area closure)

CREATE TABLE IF NOT EXISTS org.sites (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    organization_unit_id UUID,
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    address TEXT NOT NULL DEFAULT '',
    timezone TEXT NOT NULL DEFAULT 'UTC',
    revision BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    actor UUID,
    deleted_at TIMESTAMPTZ,
    CONSTRAINT sites_tenant_fk
        FOREIGN KEY (tenant_id) REFERENCES org.tenants(id),
    CONSTRAINT sites_organization_unit_tenant_fk
        FOREIGN KEY (tenant_id, organization_unit_id) REFERENCES org.organization_units(tenant_id, id)
);

ALTER TABLE org.sites
    ADD CONSTRAINT sites_tenant_id_unique UNIQUE (tenant_id, id);

CREATE UNIQUE INDEX IF NOT EXISTS sites_tenant_code_unique
    ON org.sites (tenant_id, code)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS sites_tenant_organization_unit_idx
    ON org.sites (tenant_id, organization_unit_id);

CREATE TABLE IF NOT EXISTS org.buildings (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    site_id UUID NOT NULL,
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    revision BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    actor UUID,
    deleted_at TIMESTAMPTZ,
    CONSTRAINT buildings_tenant_fk
        FOREIGN KEY (tenant_id) REFERENCES org.tenants(id),
    CONSTRAINT buildings_site_tenant_fk
        FOREIGN KEY (tenant_id, site_id) REFERENCES org.sites(tenant_id, id)
);

ALTER TABLE org.buildings
    ADD CONSTRAINT buildings_tenant_id_unique UNIQUE (tenant_id, id);

CREATE UNIQUE INDEX IF NOT EXISTS buildings_tenant_code_unique
    ON org.buildings (tenant_id, code)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS buildings_tenant_site_idx
    ON org.buildings (tenant_id, site_id);

CREATE TABLE IF NOT EXISTS org.floors (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    building_id UUID NOT NULL,
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    level INT NOT NULL DEFAULT 0,
    revision BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    actor UUID,
    deleted_at TIMESTAMPTZ,
    CONSTRAINT floors_tenant_fk
        FOREIGN KEY (tenant_id) REFERENCES org.tenants(id),
    CONSTRAINT floors_building_tenant_fk
        FOREIGN KEY (tenant_id, building_id) REFERENCES org.buildings(tenant_id, id)
);

ALTER TABLE org.floors
    ADD CONSTRAINT floors_tenant_id_unique UNIQUE (tenant_id, id);

CREATE UNIQUE INDEX IF NOT EXISTS floors_tenant_code_unique
    ON org.floors (tenant_id, code)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS floors_tenant_building_idx
    ON org.floors (tenant_id, building_id);

CREATE TABLE IF NOT EXISTS org.areas (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    floor_id UUID,
    parent_id UUID,
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    coordinate_system TEXT NOT NULL DEFAULT 'WGS84',
    latitude DOUBLE PRECISION,
    longitude DOUBLE PRECISION,
    altitude DOUBLE PRECISION,
    revision BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    actor UUID,
    deleted_at TIMESTAMPTZ,
    CONSTRAINT areas_tenant_fk
        FOREIGN KEY (tenant_id) REFERENCES org.tenants(id),
    CONSTRAINT areas_floor_tenant_fk
        FOREIGN KEY (tenant_id, floor_id) REFERENCES org.floors(tenant_id, id)
);

ALTER TABLE org.areas
    ADD CONSTRAINT areas_tenant_id_unique UNIQUE (tenant_id, id);

ALTER TABLE org.areas
    ADD CONSTRAINT areas_parent_tenant_fk
        FOREIGN KEY (tenant_id, parent_id) REFERENCES org.areas(tenant_id, id);

CREATE UNIQUE INDEX IF NOT EXISTS areas_tenant_code_unique
    ON org.areas (tenant_id, code)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS areas_tenant_floor_idx
    ON org.areas (tenant_id, floor_id);

CREATE INDEX IF NOT EXISTS areas_coordinates_idx
    ON org.areas (tenant_id, latitude, longitude)
    WHERE latitude IS NOT NULL AND longitude IS NOT NULL;

CREATE TABLE IF NOT EXISTS org.area_closure (
    tenant_id UUID NOT NULL,
    ancestor_id UUID NOT NULL,
    descendant_id UUID NOT NULL,
    depth INT NOT NULL,
    PRIMARY KEY (tenant_id, ancestor_id, descendant_id),
    CONSTRAINT area_closure_tenant_fk
        FOREIGN KEY (tenant_id) REFERENCES org.tenants(id),
    CONSTRAINT area_closure_ancestor_tenant_fk
        FOREIGN KEY (tenant_id, ancestor_id) REFERENCES org.areas(tenant_id, id),
    CONSTRAINT area_closure_descendant_tenant_fk
        FOREIGN KEY (tenant_id, descendant_id) REFERENCES org.areas(tenant_id, id)
);

CREATE INDEX IF NOT EXISTS area_closure_ancestor_idx
    ON org.area_closure (tenant_id, ancestor_id, depth);

CREATE INDEX IF NOT EXISTS area_closure_descendant_idx
    ON org.area_closure (tenant_id, descendant_id);

GRANT SELECT, INSERT, UPDATE, DELETE ON org.sites TO clsc_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON org.buildings TO clsc_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON org.floors TO clsc_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON org.areas TO clsc_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON org.area_closure TO clsc_app;
