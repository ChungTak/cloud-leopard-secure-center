-- expand: create the first authoritative table as a pattern for all domain tables.

CREATE TABLE IF NOT EXISTS iam.tenants (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('active', 'suspended', 'terminated')),
    revision BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    actor UUID,
    deleted_at TIMESTAMPTZ,
    metadata_schema_version INTEGER NOT NULL DEFAULT 1,
    metadata JSONB,
    CONSTRAINT metadata_is_object CHECK (metadata IS NULL OR jsonb_typeof(metadata) = 'object'),
    CONSTRAINT metadata_size_limit CHECK (metadata IS NULL OR length(metadata::text) < 65536)
);

COMMENT ON TABLE iam.tenants IS 'Authoritative table pattern: UUID PK, status CHECK, revision, UTC timestamps, actor, soft delete, JSONB metadata.';
