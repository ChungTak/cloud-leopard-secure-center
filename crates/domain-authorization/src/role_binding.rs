//! Role binding aggregate: assigns a role to a principal with a scope.

use foundation::{
    AreaId, BindingId, BuildingId, CameraId, Clock, DeviceId, FloorId, OrganizationId,
    PlatformError, Revision, RoleId, SiteId, TenantId, UserId, UtcTimestamp, uuid::Uuid,
};

/// A reference to a concrete resource that can appear in a `ResourceSet` scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceRef {
    User(UserId),
    Organization(OrganizationId),
    Site(SiteId),
    Building(BuildingId),
    Floor(FloorId),
    Area(AreaId),
    Device(DeviceId),
    Camera(CameraId),
}

impl ResourceRef {
    /// Stable string type tag used for persistence.
    pub fn resource_type(&self) -> &'static str {
        match self {
            Self::User(_) => "user",
            Self::Organization(_) => "organization",
            Self::Site(_) => "site",
            Self::Building(_) => "building",
            Self::Floor(_) => "floor",
            Self::Area(_) => "area",
            Self::Device(_) => "device",
            Self::Camera(_) => "camera",
        }
    }

    /// The UUID identifying the referenced resource.
    pub fn as_uuid(&self) -> Uuid {
        match self {
            Self::User(id) => *id.as_uuid(),
            Self::Organization(id) => *id.as_uuid(),
            Self::Site(id) => *id.as_uuid(),
            Self::Building(id) => *id.as_uuid(),
            Self::Floor(id) => *id.as_uuid(),
            Self::Area(id) => *id.as_uuid(),
            Self::Device(id) => *id.as_uuid(),
            Self::Camera(id) => *id.as_uuid(),
        }
    }
}

/// Scope over which a role binding applies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// Entire tenant.
    Tenant,
    /// An organization unit and all of its descendants.
    OrganizationSubtree(OrganizationId),
    /// An area and all of its descendants.
    AreaSubtree(AreaId),
    /// An explicit set of resources.
    ResourceSet(Vec<ResourceRef>),
}

impl Scope {
    /// Number of resources covered; used for batch-size limits.
    pub fn resource_count(&self) -> usize {
        match self {
            Self::Tenant => 1,
            Self::OrganizationSubtree(_) | Self::AreaSubtree(_) => 1,
            Self::ResourceSet(resources) => resources.len(),
        }
    }
}

const MAX_RESOURCE_SET_SIZE: usize = 1000;

/// A binding between a principal, a role, and an authorization scope.
#[derive(Debug, Clone)]
pub struct RoleBinding {
    pub id: BindingId,
    pub tenant_id: TenantId,
    pub principal_id: UserId,
    pub role_id: RoleId,
    pub scope: Scope,
    pub valid_from: UtcTimestamp,
    pub valid_until: Option<UtcTimestamp>,
    pub revision: Revision,
    pub created_at: UtcTimestamp,
    pub updated_at: UtcTimestamp,
    pub actor: Option<UserId>,
}

impl RoleBinding {
    /// Create a new role binding.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: BindingId,
        tenant_id: TenantId,
        principal_id: UserId,
        role_id: RoleId,
        scope: Scope,
        valid_from: UtcTimestamp,
        valid_until: Option<UtcTimestamp>,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<Self, PlatformError> {
        validate_scope(&scope)?;
        validate_validity(valid_from, valid_until)?;
        let now = clock.now();
        Ok(Self {
            id,
            tenant_id,
            principal_id,
            role_id,
            scope,
            valid_from,
            valid_until,
            revision: Revision::initial(),
            created_at: now,
            updated_at: now,
            actor,
        })
    }

    /// Reconstruct a binding from persisted parts.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        id: BindingId,
        tenant_id: TenantId,
        principal_id: UserId,
        role_id: RoleId,
        scope: Scope,
        valid_from: UtcTimestamp,
        valid_until: Option<UtcTimestamp>,
        revision: Revision,
        created_at: UtcTimestamp,
        updated_at: UtcTimestamp,
        actor: Option<UserId>,
    ) -> Result<Self, PlatformError> {
        // Scope is intentionally not re-validated here: persisted resource sets
        // are filled by the repository after construction.
        validate_validity(valid_from, valid_until)?;
        Ok(Self {
            id,
            tenant_id,
            principal_id,
            role_id,
            scope,
            valid_from,
            valid_until,
            revision,
            created_at,
            updated_at,
            actor,
        })
    }

    /// Change the scope and bump the revision.
    pub fn set_scope(
        &mut self,
        scope: Scope,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<(), PlatformError> {
        validate_scope(&scope)?;
        self.scope = scope;
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
        Ok(())
    }

    /// Change the validity window and bump the revision.
    pub fn set_validity(
        &mut self,
        valid_from: UtcTimestamp,
        valid_until: Option<UtcTimestamp>,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<(), PlatformError> {
        validate_validity(valid_from, valid_until)?;
        self.valid_from = valid_from;
        self.valid_until = valid_until;
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
        Ok(())
    }

    /// Whether the binding is currently valid relative to `now`.
    pub fn is_valid_at(&self, now: UtcTimestamp) -> bool {
        self.valid_from <= now && self.valid_until.is_none_or(|until| now < until)
    }
}

fn validate_scope(scope: &Scope) -> Result<(), PlatformError> {
    if let Scope::ResourceSet(resources) = scope {
        if resources.is_empty() {
            return Err(PlatformError::invalid(
                "scope",
                "resource set scope must not be empty",
            ));
        }
        if resources.len() > MAX_RESOURCE_SET_SIZE {
            return Err(PlatformError::invalid(
                "scope",
                format!("resource set may contain at most {MAX_RESOURCE_SET_SIZE} resources"),
            ));
        }
    }
    Ok(())
}

fn validate_validity(
    valid_from: UtcTimestamp,
    valid_until: Option<UtcTimestamp>,
) -> Result<(), PlatformError> {
    if let Some(until) = valid_until
        && until <= valid_from
    {
        return Err(PlatformError::invalid(
            "valid_until",
            "valid_until must be after valid_from",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use foundation::FakeClock;

    fn id(s: &str) -> BindingId {
        BindingId::parse_str(s).unwrap_or_else(|e| panic!("{e}"))
    }

    fn tenant_id() -> TenantId {
        TenantId::parse_str("018e1234-5678-7abc-8def-0123456789ab")
            .unwrap_or_else(|e| panic!("{e}"))
    }

    fn user_id() -> UserId {
        UserId::parse_str("018e1234-5678-7abc-8def-0123456789ac").unwrap_or_else(|e| panic!("{e}"))
    }

    fn role_id() -> RoleId {
        RoleId::parse_str("018e1234-5678-7abc-8def-0123456789ad").unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn empty_resource_set_is_rejected() {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        let result = RoleBinding::new(
            id("018e0000-0000-0000-0000-000000000001"),
            tenant_id(),
            user_id(),
            role_id(),
            Scope::ResourceSet(vec![]),
            clock.now(),
            None,
            &clock,
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn resource_set_size_is_bounded() {
        let refs: Vec<ResourceRef> = (0..1001).map(|_| ResourceRef::User(user_id())).collect();
        let result = validate_scope(&Scope::ResourceSet(refs));
        assert!(result.is_err());
    }

    #[test]
    fn valid_until_must_be_after_valid_from() {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        let from = clock.now();
        let until = Some(
            UtcTimestamp::parse_rfc3339("2000-01-01T00:00:00Z").unwrap_or_else(|e| panic!("{e}")),
        );
        let result = RoleBinding::new(
            id("018e0000-0000-0000-0000-000000000001"),
            tenant_id(),
            user_id(),
            role_id(),
            Scope::Tenant,
            from,
            until,
            &clock,
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn expiry_is_respected() {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        let from = clock.now();
        let until =
            UtcTimestamp::parse_rfc3339("2099-01-01T00:00:00Z").unwrap_or_else(|e| panic!("{e}"));
        let binding = RoleBinding::new(
            id("018e0000-0000-0000-0000-000000000001"),
            tenant_id(),
            user_id(),
            role_id(),
            Scope::Tenant,
            from,
            Some(until),
            &clock,
            None,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        assert!(binding.is_valid_at(from));
        assert!(!binding.is_valid_at(until));
    }
}
