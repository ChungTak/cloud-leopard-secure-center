-- expand: create base schemas and roles

CREATE SCHEMA IF NOT EXISTS iam;
CREATE SCHEMA IF NOT EXISTS org;
CREATE SCHEMA IF NOT EXISTS authz;
CREATE SCHEMA IF NOT EXISTS resource;
CREATE SCHEMA IF NOT EXISTS audit;
CREATE SCHEMA IF NOT EXISTS config;
CREATE SCHEMA IF NOT EXISTS infra;
CREATE SCHEMA IF NOT EXISTS app;

DO $$
BEGIN
    CREATE ROLE clsc_migrator WITH LOGIN;
EXCEPTION
    WHEN unique_violation OR duplicate_object THEN
        NULL;
END
$$;

DO $$
BEGIN
    CREATE ROLE clsc_app NOLOGIN;
EXCEPTION
    WHEN unique_violation OR duplicate_object THEN
        NULL;
END
$$;

GRANT USAGE ON SCHEMA iam, org, authz, resource, audit, config, infra TO clsc_app;

CREATE TABLE IF NOT EXISTS infra.schema_metadata (
    version BIGINT PRIMARY KEY,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    checksum TEXT NOT NULL,
    description TEXT NOT NULL
);
