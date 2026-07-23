//! Managed device aggregate.

use foundation::{
    AreaId, Clock, DeviceId, OrganizationId, PlatformError, Revision, TenantId, UserId,
    UtcTimestamp,
};

/// Business lifecycle state of a managed device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceLifecycle {
    Draft,
    Active,
    Disabled,
    Retired,
}

impl DeviceLifecycle {
    /// Whether the device is available for operational use.
    pub fn is_operational(&self) -> bool {
        matches!(self, Self::Active)
    }

    /// Whether the device can be changed to another lifecycle state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Retired)
    }

    /// Serialize to the lowercase form stored in the database.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Disabled => "disabled",
            Self::Retired => "retired",
        }
    }

    /// Parse the serialized form.
    pub fn parse(input: &str) -> Result<Self, PlatformError> {
        match input {
            "draft" => Ok(Self::Draft),
            "active" => Ok(Self::Active),
            "disabled" => Ok(Self::Disabled),
            "retired" => Ok(Self::Retired),
            _ => Err(PlatformError::invalid(
                "lifecycle",
                format!("unknown device lifecycle: {input}"),
            )),
        }
    }
}

/// Observed online state reported by an external integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnlineState {
    Unknown,
    Online,
    Offline,
}

impl OnlineState {
    /// Serialize to the lowercase form stored in the database.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Online => "online",
            Self::Offline => "offline",
        }
    }

    /// Parse the serialized form.
    pub fn parse(input: &str) -> Result<Self, PlatformError> {
        match input {
            "unknown" => Ok(Self::Unknown),
            "online" => Ok(Self::Online),
            "offline" => Ok(Self::Offline),
            _ => Err(PlatformError::invalid(
                "online_state",
                format!("unknown online state: {input}"),
            )),
        }
    }
}

/// A managed device in the resource catalog.
#[derive(Debug, Clone)]
pub struct ManagedDevice {
    pub id: DeviceId,
    pub tenant_id: TenantId,
    pub organization_id: Option<OrganizationId>,
    pub area_id: Option<AreaId>,
    pub code: String,
    pub name: String,
    pub serial: Option<String>,
    pub lifecycle: DeviceLifecycle,
    pub online_state: OnlineState,
    pub revision: Revision,
    pub created_at: UtcTimestamp,
    pub updated_at: UtcTimestamp,
    pub actor: Option<UserId>,
}

impl ManagedDevice {
    /// Create a new managed device in the Draft lifecycle.
    pub fn new(
        id: DeviceId,
        tenant_id: TenantId,
        code: impl Into<String>,
        name: impl Into<String>,
        serial: Option<String>,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<Self, PlatformError> {
        let code = code.into();
        validate_code(&code)?;
        let now = clock.now();
        Ok(Self {
            id,
            tenant_id,
            organization_id: None,
            area_id: None,
            code,
            name: name.into(),
            serial,
            lifecycle: DeviceLifecycle::Draft,
            online_state: OnlineState::Unknown,
            revision: Revision::initial(),
            created_at: now,
            updated_at: now,
            actor,
        })
    }

    /// Reconstruct a device from persisted parts.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        id: DeviceId,
        tenant_id: TenantId,
        organization_id: Option<OrganizationId>,
        area_id: Option<AreaId>,
        code: impl Into<String>,
        name: impl Into<String>,
        serial: Option<String>,
        lifecycle: DeviceLifecycle,
        online_state: OnlineState,
        revision: Revision,
        created_at: UtcTimestamp,
        updated_at: UtcTimestamp,
        actor: Option<UserId>,
    ) -> Result<Self, PlatformError> {
        let code = code.into();
        validate_code(&code)?;
        Ok(Self {
            id,
            tenant_id,
            organization_id,
            area_id,
            code,
            name: name.into(),
            serial,
            lifecycle,
            online_state,
            revision,
            created_at,
            updated_at,
            actor,
        })
    }

    /// Move from Draft to Active.
    pub fn activate(
        &mut self,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<(), PlatformError> {
        self.transition_to(DeviceLifecycle::Active, clock, actor)
    }

    /// Disable an Active device.
    pub fn disable(
        &mut self,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<(), PlatformError> {
        self.transition_to(DeviceLifecycle::Disabled, clock, actor)
    }

    /// Retire a device; retired devices are terminal and do not cascade deletes.
    pub fn retire(
        &mut self,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<(), PlatformError> {
        self.transition_to(DeviceLifecycle::Retired, clock, actor)
    }

    fn transition_to(
        &mut self,
        target: DeviceLifecycle,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<(), PlatformError> {
        if self.lifecycle.is_terminal() {
            return Err(PlatformError::invalid(
                "lifecycle",
                "retired device cannot transition",
            ));
        }
        let valid = match (self.lifecycle, target) {
            (DeviceLifecycle::Draft, DeviceLifecycle::Active) => true,
            (DeviceLifecycle::Active, DeviceLifecycle::Disabled) => true,
            (DeviceLifecycle::Disabled, DeviceLifecycle::Active) => true,
            (DeviceLifecycle::Active, DeviceLifecycle::Retired) => true,
            (DeviceLifecycle::Disabled, DeviceLifecycle::Retired) => true,
            (a, b) if a == b => true,
            _ => false,
        };
        if !valid {
            return Err(PlatformError::invalid(
                "lifecycle",
                format!(
                    "cannot transition from {:?} to {:?}",
                    self.lifecycle, target
                ),
            ));
        }
        self.lifecycle = target;
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
        Ok(())
    }

    /// Update the observed online state without affecting business lifecycle.
    pub fn set_online_state(
        &mut self,
        state: OnlineState,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) {
        self.online_state = state;
        self.updated_at = clock.now();
        self.actor = actor;
    }

    /// Assign the device to an organization and area within the same tenant.
    pub fn set_location(
        &mut self,
        organization_id: Option<OrganizationId>,
        area_id: Option<AreaId>,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) {
        self.organization_id = organization_id;
        self.area_id = area_id;
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
    }

    /// Rename the device.
    pub fn rename(&mut self, name: impl Into<String>, clock: &dyn Clock, actor: Option<UserId>) {
        self.name = name.into();
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
    }
}

fn validate_code(code: &str) -> Result<(), PlatformError> {
    if code.trim().is_empty() {
        return Err(PlatformError::invalid(
            "code",
            "device code must not be empty",
        ));
    }
    if code.len() > 128 {
        return Err(PlatformError::invalid(
            "code",
            "device code must be at most 128 characters",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use foundation::FakeClock;

    fn id(s: &str) -> DeviceId {
        DeviceId::parse_str(s).unwrap_or_else(|e| panic!("{e}"))
    }

    fn tenant() -> TenantId {
        TenantId::parse_str("018e1234-5678-7abc-8def-0123456789ab")
            .unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn lifecycle_transitions_and_terminal_state() {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        let mut device = ManagedDevice::new(
            id("018e0000-0000-0000-0000-000000000001"),
            tenant(),
            "cam-01",
            "Camera 1",
            Some("SN123".to_string()),
            &clock,
            None,
        )
        .unwrap_or_else(|e| panic!("{e}"));

        device
            .activate(&clock, None)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(device.lifecycle, DeviceLifecycle::Active);

        device
            .disable(&clock, None)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(device.lifecycle, DeviceLifecycle::Disabled);

        device
            .activate(&clock, None)
            .unwrap_or_else(|e| panic!("{e}"));
        device
            .retire(&clock, None)
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(device.lifecycle.is_terminal());

        assert!(device.retire(&clock, None).is_err());
    }

    #[test]
    fn draft_cannot_be_disabled() {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        let mut device = ManagedDevice::new(
            id("018e0000-0000-0000-0000-000000000001"),
            tenant(),
            "cam-01",
            "Camera 1",
            None,
            &clock,
            None,
        )
        .unwrap_or_else(|e| panic!("{e}"));

        assert!(device.disable(&clock, None).is_err());
    }

    #[test]
    fn empty_code_is_rejected() {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        assert!(
            ManagedDevice::new(
                id("018e0000-0000-0000-0000-000000000001"),
                tenant(),
                "   ",
                "Camera 1",
                None,
                &clock,
                None,
            )
            .is_err()
        );
    }
}
