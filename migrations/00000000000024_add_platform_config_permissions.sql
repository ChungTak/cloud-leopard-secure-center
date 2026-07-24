-- Add platform-scoped configuration definition permissions.
--
-- Configuration definitions are global (platform) resources, but the existing
-- permission registry only had tenant-scoped `tenant:config:*` keys. The
-- application layer was incorrectly reusing `platform:tenant:write`/`read` for
-- definition management.

INSERT INTO authz.permissions (key, scope) VALUES
    ('platform:config:read', 'platform'),
    ('platform:config:write', 'platform')
ON CONFLICT (key) DO NOTHING;
