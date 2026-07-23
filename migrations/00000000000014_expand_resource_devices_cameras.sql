-- expand: managed devices and cameras

CREATE SCHEMA IF NOT EXISTS resource;

CREATE TABLE IF NOT EXISTS resource.managed_devices (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    organization_id UUID,
    area_id UUID,
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    serial TEXT,
    lifecycle TEXT NOT NULL CHECK (lifecycle IN ('draft', 'active', 'disabled', 'retired')),
    online_state TEXT NOT NULL CHECK (online_state IN ('unknown', 'online', 'offline')),
    revision BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    actor UUID,
    deleted_at TIMESTAMPTZ,
    CONSTRAINT managed_devices_tenant_fk
        FOREIGN KEY (tenant_id) REFERENCES org.tenants(id),
    CONSTRAINT managed_devices_organization_tenant_fk
        FOREIGN KEY (tenant_id, organization_id) REFERENCES org.organization_units(tenant_id, id),
    CONSTRAINT managed_devices_area_tenant_fk
        FOREIGN KEY (tenant_id, area_id) REFERENCES org.areas(tenant_id, id),
    UNIQUE (tenant_id, code),
    UNIQUE (tenant_id, id)
);

CREATE INDEX IF NOT EXISTS managed_devices_tenant_lifecycle_idx
    ON resource.managed_devices (tenant_id, lifecycle)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS managed_devices_tenant_area_idx
    ON resource.managed_devices (tenant_id, area_id)
    WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS resource.cameras (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    device_id UUID NOT NULL,
    area_id UUID,
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    sensitivity TEXT NOT NULL CHECK (sensitivity IN ('low', 'medium', 'high', 'critical')),
    is_enabled BOOLEAN NOT NULL DEFAULT true,
    revision BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    actor UUID,
    deleted_at TIMESTAMPTZ,
    CONSTRAINT cameras_tenant_fk
        FOREIGN KEY (tenant_id) REFERENCES org.tenants(id),
    CONSTRAINT cameras_device_tenant_fk
        FOREIGN KEY (tenant_id, device_id) REFERENCES resource.managed_devices(tenant_id, id),
    CONSTRAINT cameras_area_tenant_fk
        FOREIGN KEY (tenant_id, area_id) REFERENCES org.areas(tenant_id, id),
    UNIQUE (tenant_id, code)
);

CREATE INDEX IF NOT EXISTS cameras_tenant_device_idx
    ON resource.cameras (tenant_id, device_id)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS cameras_tenant_sensitivity_idx
    ON resource.cameras (tenant_id, sensitivity)
    WHERE deleted_at IS NULL;

GRANT SELECT, INSERT, UPDATE, DELETE ON resource.managed_devices TO clsc_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON resource.cameras TO clsc_app;
