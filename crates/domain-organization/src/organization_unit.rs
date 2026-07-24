//! Organization unit aggregate and closure-tree primitives.

use foundation::{Clock, OrganizationId, PlatformError, Revision, TenantId, UserId, UtcTimestamp};

/// A node in a tenant's organization hierarchy.
#[derive(Debug, Clone)]
pub struct OrganizationUnit {
    /// Unique organization unit identifier.
    pub id: OrganizationId,
    /// Owning tenant.
    pub tenant_id: TenantId,
    /// Parent unit, if any.
    pub parent_id: Option<OrganizationId>,
    /// Immutable human-readable code.
    pub code: String,
    /// Display name.
    pub name: String,
    /// Optimistic lock version.
    pub revision: Revision,
    /// Creation timestamp.
    pub created_at: UtcTimestamp,
    /// Last update timestamp.
    pub updated_at: UtcTimestamp,
    /// Actor that performed the last change.
    pub actor: Option<UserId>,
}

impl OrganizationUnit {
    /// Create a new organization unit.
    pub fn new(
        id: OrganizationId,
        tenant_id: TenantId,
        parent_id: Option<OrganizationId>,
        code: impl AsRef<str>,
        name: impl AsRef<str>,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<Self, PlatformError> {
        let code = code.as_ref();
        validate_code(code)?;
        let name = name.as_ref();
        validate_name(name)?;
        let now = clock.now();
        Ok(Self {
            id,
            tenant_id,
            parent_id,
            code: code.to_string(),
            name: name.to_string(),
            revision: Revision::initial(),
            created_at: now,
            updated_at: now,
            actor,
        })
    }

    /// Reconstruct a unit from persisted parts.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        id: OrganizationId,
        tenant_id: TenantId,
        parent_id: Option<OrganizationId>,
        code: impl AsRef<str>,
        name: impl AsRef<str>,
        revision: Revision,
        created_at: UtcTimestamp,
        updated_at: UtcTimestamp,
        actor: Option<UserId>,
    ) -> Result<Self, PlatformError> {
        let code = code.as_ref();
        validate_code(code)?;
        let name = name.as_ref();
        validate_name(name)?;
        Ok(Self {
            id,
            tenant_id,
            parent_id,
            code: code.to_string(),
            name: name.to_string(),
            revision,
            created_at,
            updated_at,
            actor,
        })
    }

    /// Rename the unit and bump the revision.
    pub fn rename(
        &mut self,
        name: impl AsRef<str>,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<(), PlatformError> {
        let name = name.as_ref();
        validate_name(name)?;
        let name = name.to_string();
        if name == self.name {
            return Ok(());
        }
        self.name = name;
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
        Ok(())
    }

    /// Set a new parent after validating that it is not this unit or one of its descendants.
    pub fn set_parent(
        &mut self,
        parent_id: Option<OrganizationId>,
        descendants: &[OrganizationId],
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<(), PlatformError> {
        if let Some(pid) = parent_id {
            if pid == self.id {
                return Err(PlatformError::invalid(
                    "parent_id",
                    "an organization unit cannot be its own parent",
                ));
            }
            if descendants.contains(&pid) {
                return Err(PlatformError::invalid(
                    "parent_id",
                    "cannot move an organization unit under one of its descendants",
                ));
            }
        }
        self.parent_id = parent_id;
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
        Ok(())
    }

    /// Whether this unit is a root node.
    pub fn is_root(&self) -> bool {
        self.parent_id.is_none()
    }
}

fn validate_code(code: &str) -> Result<(), PlatformError> {
    if code.is_empty() {
        return Err(PlatformError::invalid(
            "organization_unit_code",
            "organization unit code must not be empty",
        ));
    }
    if code.len() > 64 {
        return Err(PlatformError::invalid(
            "organization_unit_code",
            "organization unit code must be at most 64 characters",
        ));
    }
    if code.trim() != code || code.contains(' ') {
        return Err(PlatformError::invalid(
            "organization_unit_code",
            "organization unit code must not contain leading, trailing, or internal whitespace",
        ));
    }
    Ok(())
}

fn validate_name(name: &str) -> Result<(), PlatformError> {
    if name.trim().is_empty() {
        return Err(PlatformError::invalid(
            "organization_unit_name",
            "organization unit name must not be empty",
        ));
    }
    if name.len() > 128 {
        return Err(PlatformError::invalid(
            "organization_unit_name",
            "organization unit name must be at most 128 characters",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use foundation::FakeClock;

    fn unit_id() -> OrganizationId {
        match OrganizationId::parse_str("018e1234-5678-7abc-8def-0123456789ab") {
            Ok(id) => id,
            Err(e) => panic!("{e}"),
        }
    }

    fn tenant_id() -> TenantId {
        match TenantId::parse_str("018e1234-5678-7abc-8def-0123456789ab") {
            Ok(id) => id,
            Err(e) => panic!("{e}"),
        }
    }

    fn make_unit() -> Result<OrganizationUnit, PlatformError> {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        OrganizationUnit::new(unit_id(), tenant_id(), None, "acme", "Acme", &clock, None)
    }

    #[test]
    fn empty_code_is_rejected() {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        let result = OrganizationUnit::new(unit_id(), tenant_id(), None, "", "Name", &clock, None);
        assert!(result.is_err());
    }

    #[test]
    fn set_parent_rejects_self_and_descendants() -> Result<(), PlatformError> {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        let mut unit = make_unit()?;
        let child_id = OrganizationId::parse_str("018e1234-5678-7abc-8def-0123456789ac")?;

        let result = unit.set_parent(Some(unit.id), &[unit.id], &clock, None);
        assert!(result.is_err());

        let result = unit.set_parent(Some(child_id), &[unit.id, child_id], &clock, None);
        assert!(result.is_err());

        unit.set_parent(None, &[unit.id, child_id], &clock, None)?;
        assert!(unit.is_root());
        Ok(())
    }

    #[test]
    fn rename_bumps_revision() -> Result<(), PlatformError> {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        let mut unit = make_unit()?;
        let before = unit.revision;
        unit.rename("Updated", &clock, None)?;
        assert_eq!(unit.revision.value(), before.value() + 1);
        Ok(())
    }
}
