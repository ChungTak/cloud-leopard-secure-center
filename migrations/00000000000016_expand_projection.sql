-- expand: signaling projection tables

CREATE SCHEMA IF NOT EXISTS projection;

GRANT USAGE, CREATE ON SCHEMA projection TO clsc_app;

CREATE TABLE IF NOT EXISTS projection.devices (
    tenant_id UUID NOT NULL,
    external_ref TEXT NOT NULL,
    sequence BIGINT NOT NULL,
    source_event_id TEXT NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL,
    payload TEXT NOT NULL,
    stale BOOLEAN NOT NULL DEFAULT false,
    PRIMARY KEY (tenant_id, external_ref)
);

CREATE TABLE IF NOT EXISTS projection.devices_shadow (
    tenant_id UUID NOT NULL,
    external_ref TEXT NOT NULL,
    sequence BIGINT NOT NULL,
    source_event_id TEXT NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL,
    payload TEXT NOT NULL,
    stale BOOLEAN NOT NULL DEFAULT false,
    PRIMARY KEY (tenant_id, external_ref)
);

CREATE TABLE IF NOT EXISTS projection.channels (
    tenant_id UUID NOT NULL,
    external_ref TEXT NOT NULL,
    sequence BIGINT NOT NULL,
    source_event_id TEXT NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL,
    payload TEXT NOT NULL,
    stale BOOLEAN NOT NULL DEFAULT false,
    PRIMARY KEY (tenant_id, external_ref)
);

CREATE TABLE IF NOT EXISTS projection.channels_shadow (
    tenant_id UUID NOT NULL,
    external_ref TEXT NOT NULL,
    sequence BIGINT NOT NULL,
    source_event_id TEXT NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL,
    payload TEXT NOT NULL,
    stale BOOLEAN NOT NULL DEFAULT false,
    PRIMARY KEY (tenant_id, external_ref)
);

CREATE TABLE IF NOT EXISTS projection.active_view (
    tenant_id UUID PRIMARY KEY,
    device_view TEXT NOT NULL DEFAULT 'devices',
    channel_view TEXT NOT NULL DEFAULT 'channels',
    generation BIGINT NOT NULL DEFAULT 1,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT active_view_device_view_check CHECK (device_view IN ('devices', 'devices_shadow')),
    CONSTRAINT active_view_channel_view_check CHECK (channel_view IN ('channels', 'channels_shadow'))
);

CREATE TABLE IF NOT EXISTS projection.checkpoints (
    worker_id TEXT NOT NULL,
    tenant_id UUID NOT NULL,
    last_event_id TEXT NOT NULL,
    last_observed_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (worker_id, tenant_id)
);

CREATE TABLE IF NOT EXISTS projection.failures (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    source_event_id TEXT NOT NULL,
    external_ref TEXT NOT NULL,
    reason TEXT NOT NULL,
    payload TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at TIMESTAMPTZ
);

GRANT SELECT, INSERT, UPDATE, DELETE ON projection.devices TO clsc_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON projection.devices_shadow TO clsc_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON projection.channels TO clsc_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON projection.channels_shadow TO clsc_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON projection.active_view TO clsc_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON projection.checkpoints TO clsc_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON projection.failures TO clsc_app;
