//! Permission key registry and scope classification.

use foundation::PlatformError;

/// The scope at which a permission is granted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionScope {
    /// Platform-wide permission, cannot be granted to tenant-scoped roles.
    Platform,
    /// Tenant-scoped permission.
    Tenant,
}

impl PermissionScope {
    /// Serialize to the string stored in the database.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Platform => "platform",
            Self::Tenant => "tenant",
        }
    }

    /// Parse the serialized form.
    pub fn parse(input: &str) -> Result<Self, PlatformError> {
        match input {
            "platform" => Ok(Self::Platform),
            "tenant" => Ok(Self::Tenant),
            _ => Err(PlatformError::invalid(
                "permission_scope",
                format!("unsupported permission scope: {input}"),
            )),
        }
    }
}

/// A known permission key and its scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Permission {
    /// Permission key, e.g. `user:read`.
    pub key: String,
    /// Scope at which this permission may be granted.
    pub scope: PermissionScope,
}

impl Permission {
    /// Look up a permission by key in the static registry.
    pub fn parse(key: &str) -> Result<Self, PlatformError> {
        let key_trimmed = key.trim();
        for (k, scope) in KNOWN_PERMISSIONS {
            if *k == key_trimmed {
                return Ok(Self {
                    key: key_trimmed.to_string(),
                    scope: *scope,
                });
            }
        }
        Err(PlatformError::invalid(
            "permission_key",
            format!("unknown permission key: {key_trimmed}"),
        ))
    }

    /// All known permissions, useful for seeding and validation.
    pub fn all() -> Vec<Permission> {
        KNOWN_PERMISSIONS
            .iter()
            .map(|(k, scope)| Permission {
                key: k.to_string(),
                scope: *scope,
            })
            .collect()
    }
}

const KNOWN_PERMISSIONS: &[(&str, PermissionScope)] = &[
    ("platform:tenant:read", PermissionScope::Platform),
    ("platform:tenant:write", PermissionScope::Platform),
    ("platform:role:read", PermissionScope::Platform),
    ("platform:role:write", PermissionScope::Platform),
    ("tenant:user:read", PermissionScope::Tenant),
    ("tenant:user:write", PermissionScope::Tenant),
    ("tenant:role:read", PermissionScope::Tenant),
    ("tenant:role:write", PermissionScope::Tenant),
    ("tenant:organization:read", PermissionScope::Tenant),
    ("tenant:organization:write", PermissionScope::Tenant),
    ("tenant:site:read", PermissionScope::Tenant),
    ("tenant:site:write", PermissionScope::Tenant),
    ("tenant:area:read", PermissionScope::Tenant),
    ("tenant:area:write", PermissionScope::Tenant),
    ("tenant:device:read", PermissionScope::Tenant),
    ("tenant:device:write", PermissionScope::Tenant),
    ("tenant:config:read", PermissionScope::Tenant),
    ("tenant:config:write", PermissionScope::Tenant),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_key_is_rejected() {
        assert!(Permission::parse("tenant:unknown").is_err());
    }

    #[test]
    fn known_key_has_correct_scope() {
        let p = Permission::parse("tenant:user:read").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(p.scope, PermissionScope::Tenant);
    }

    #[test]
    fn platform_key_is_recognized() {
        let p = Permission::parse("platform:tenant:read").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(p.scope, PermissionScope::Platform);
    }
}
