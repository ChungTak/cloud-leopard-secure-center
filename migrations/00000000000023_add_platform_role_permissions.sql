-- Add platform-scoped role management permissions.
--
-- The application layer uses `platform:role:read` and `platform:role:write`
-- for platform-level role operations; these were missing from the registry.

INSERT INTO authz.permissions (key, scope) VALUES
    ('platform:role:read', 'platform'),
    ('platform:role:write', 'platform')
ON CONFLICT (key) DO NOTHING;
