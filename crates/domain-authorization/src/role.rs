//! Role aggregate with permission grants and built-in protection.

#![allow(clippy::too_many_arguments)]

use crate::permission::{Permission, PermissionScope};
use foundation::{Clock, PlatformError, Revision, RoleId, TenantId, UserId, UtcTimestamp};

/// A role aggregates a set of permission keys.
#[derive(Debug, Clone)]
pub struct Role {
    pub id: RoleId,
    pub tenant_id: Option<TenantId>,
    pub name: String,
    pub is_builtin: bool,
    pub permissions: Vec<String>,
    pub revision: Revision,
    pub created_at: UtcTimestamp,
    pub updated_at: UtcTimestamp,
    pub actor: Option<UserId>,
}

impl Role {
    /// Create a new role.
    pub fn new(
        id: RoleId,
        tenant_id: Option<TenantId>,
        name: impl Into<String>,
        is_builtin: bool,
        permissions: Vec<Permission>,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<Self, PlatformError> {
        let name = name.into();
        validate_name(&name)?;
        let permission_keys = validate_permissions(tenant_id, permissions)?;
        let now = clock.now();
        Ok(Self {
            id,
            tenant_id,
            name,
            is_builtin,
            permissions: permission_keys,
            revision: Revision::initial(),
            created_at: now,
            updated_at: now,
            actor,
        })
    }

    /// Reconstruct a role from persisted parts.
    pub fn from_parts(
        id: RoleId,
        tenant_id: Option<TenantId>,
        name: impl Into<String>,
        is_builtin: bool,
        permissions: Vec<Permission>,
        revision: Revision,
        created_at: UtcTimestamp,
        updated_at: UtcTimestamp,
        actor: Option<UserId>,
    ) -> Result<Self, PlatformError> {
        let name = name.into();
        validate_name(&name)?;
        let permission_keys = validate_permissions(tenant_id, permissions)?;
        Ok(Self {
            id,
            tenant_id,
            name,
            is_builtin,
            permissions: permission_keys,
            revision,
            created_at,
            updated_at,
            actor,
        })
    }

    /// Rename the role and bump its revision.
    pub fn rename(
        &mut self,
        name: impl Into<String>,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<(), PlatformError> {
        self.require_not_builtin("rename")?;
        let name = name.into();
        validate_name(&name)?;
        self.name = name;
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
        Ok(())
    }

    /// Grant a permission and bump the revision.
    pub fn grant_permission(
        &mut self,
        permission: Permission,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<(), PlatformError> {
        self.require_not_builtin("grant permission")?;
        if self.tenant_id.is_some() && permission.scope == PermissionScope::Platform {
            return Err(PlatformError::invalid(
                "permission",
                "tenant-scoped roles cannot be granted platform permissions",
            ));
        }
        if self.permissions.contains(&permission.key) {
            return Err(PlatformError::invalid(
                "permission",
                format!("permission {} is already granted", permission.key),
            ));
        }
        self.permissions.push(permission.key);
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
        Ok(())
    }

    /// Revoke a permission and bump the revision.
    pub fn revoke_permission(
        &mut self,
        key: &str,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<(), PlatformError> {
        self.require_not_builtin("revoke permission")?;
        let key = key.trim();
        let before = self.permissions.len();
        self.permissions.retain(|p| p != key);
        if self.permissions.len() == before {
            return Err(PlatformError::invalid(
                "permission",
                format!("permission {key} is not granted"),
            ));
        }
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
        Ok(())
    }

    /// Whether this is a tenant-scoped role.
    pub fn is_tenant_role(&self) -> bool {
        self.tenant_id.is_some()
    }

    fn require_not_builtin(&self, action: &str) -> Result<(), PlatformError> {
        if self.is_builtin {
            return Err(PlatformError::invalid(
                "role",
                format!("built-in roles cannot be {action}"),
            ));
        }
        Ok(())
    }
}

fn validate_name(name: &str) -> Result<(), PlatformError> {
    if name.trim().is_empty() {
        return Err(PlatformError::invalid(
            "role_name",
            "role name must not be empty",
        ));
    }
    if name.len() > 128 {
        return Err(PlatformError::invalid(
            "role_name",
            "role name must be at most 128 characters",
        ));
    }
    Ok(())
}

fn validate_permissions(
    tenant_id: Option<TenantId>,
    permissions: Vec<Permission>,
) -> Result<Vec<String>, PlatformError> {
    let mut keys = Vec::with_capacity(permissions.len());
    for p in permissions {
        if tenant_id.is_some() && p.scope == PermissionScope::Platform {
            return Err(PlatformError::invalid(
                "permission",
                "tenant-scoped roles cannot be granted platform permissions",
            ));
        }
        keys.push(p.key);
    }
    keys.sort();
    keys.dedup();
    Ok(keys)
}

#[cfg(test)]
mod tests {
    use super::*;
    use foundation::FakeClock;

    fn role_id() -> RoleId {
        RoleId::parse_str("018e1234-5678-7abc-8def-0123456789ab").unwrap_or_else(|e| panic!("{e}"))
    }

    fn tenant_id() -> TenantId {
        TenantId::parse_str("018e1234-5678-7abc-8def-0123456789ab")
            .unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn tenant_role_cannot_hold_platform_permission() {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        let perms =
            vec![Permission::parse("platform:tenant:read").unwrap_or_else(|e| panic!("{e}"))];
        let result = Role::new(
            role_id(),
            Some(tenant_id()),
            "admin",
            false,
            perms,
            &clock,
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn builtin_role_cannot_be_modified() {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        let mut role = Role::new(
            role_id(),
            Some(tenant_id()),
            "Admin",
            true,
            vec![],
            &clock,
            None,
        )
        .unwrap_or_else(|e| panic!("{e}"));

        assert!(role.rename("Updated", &clock, None).is_err());
        assert!(
            role.grant_permission(
                Permission::parse("tenant:user:read").unwrap_or_else(|e| panic!("{e}")),
                &clock,
                None,
            )
            .is_err()
        );
    }

    #[test]
    fn grant_and_revoke_bump_revision() {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        let mut role = Role::new(
            role_id(),
            Some(tenant_id()),
            "Admin",
            false,
            vec![],
            &clock,
            None,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let before = role.revision;

        role.grant_permission(
            Permission::parse("tenant:user:read").unwrap_or_else(|e| panic!("{e}")),
            &clock,
            None,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(role.revision.value(), before.value() + 1);

        role.revoke_permission("tenant:user:read", &clock, None)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(role.revision.value(), before.value() + 2);
    }
}
