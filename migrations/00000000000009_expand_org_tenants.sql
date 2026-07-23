-- expand: move tenants to org schema and add organization fields

-- Move the existing authoritative tenant table from iam to org.
-- This preserves rows, privileges, and RLS policies already attached to the table.
ALTER TABLE IF EXISTS iam.tenants SET SCHEMA org;

-- Add the immutable tenant code and default locale/timezone.
ALTER TABLE org.tenants
    ADD COLUMN IF NOT EXISTS code TEXT,
    ADD COLUMN IF NOT EXISTS locale TEXT DEFAULT 'en-US' NOT NULL,
    ADD COLUMN IF NOT EXISTS timezone TEXT DEFAULT 'UTC' NOT NULL;

-- Backfill existing rows with stable, unique codes derived from their ids.
UPDATE org.tenants SET code = id::text WHERE code IS NULL;

-- Code is required and globally unique.
ALTER TABLE org.tenants ALTER COLUMN code SET NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS tenants_code_unique ON org.tenants (code);

-- Replace the legacy status check with the Active/Suspended/Closed lifecycle.
DO $$
DECLARE
    con_name text;
BEGIN
    SELECT conname INTO con_name
    FROM pg_constraint
    WHERE conrelid = 'org.tenants'::regclass
      AND contype = 'c'
      AND pg_get_constraintdef(oid) LIKE '%status%';

    IF con_name IS NOT NULL THEN
        EXECUTE format('ALTER TABLE org.tenants DROP CONSTRAINT %I', con_name);
    END IF;
END $$;

UPDATE org.tenants SET status = 'closed' WHERE status = 'terminated';

ALTER TABLE org.tenants
    ADD CONSTRAINT tenants_status_check CHECK (status IN ('active', 'suspended', 'closed'));

-- Ensure the app role can manage the table after the schema move.
GRANT SELECT, INSERT, UPDATE, DELETE ON org.tenants TO clsc_app;

-- Refresh the tenant-isolation policy for the new schema name.
DROP POLICY IF EXISTS tenant_isolation ON org.tenants;
CREATE POLICY tenant_isolation ON org.tenants
    USING (id = app.current_tenant_id())
    WITH CHECK (id = app.current_tenant_id());

ALTER TABLE org.tenants ENABLE ROW LEVEL SECURITY, FORCE ROW LEVEL SECURITY;
