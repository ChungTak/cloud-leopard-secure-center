//! Camera aggregate with independent sensitivity modeling.

use foundation::{
    AreaId, CameraId, Clock, DeviceId, PlatformError, Revision, TenantId, UserId, UtcTimestamp,
};

/// Sensitivity level for a camera view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sensitivity {
    Low,
    Medium,
    High,
    Critical,
}

impl Sensitivity {
    /// Serialize to the lowercase form stored in the database.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }

    /// Parse the serialized form.
    pub fn parse(input: &str) -> Result<Self, PlatformError> {
        match input {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "critical" => Ok(Self::Critical),
            _ => Err(PlatformError::invalid(
                "sensitivity",
                format!("unknown sensitivity: {input}"),
            )),
        }
    }
}

/// A camera bound to a managed device.
#[derive(Debug, Clone)]
pub struct Camera {
    pub id: CameraId,
    pub tenant_id: TenantId,
    pub device_id: DeviceId,
    pub area_id: Option<AreaId>,
    pub code: String,
    pub name: String,
    pub sensitivity: Sensitivity,
    pub is_enabled: bool,
    pub revision: Revision,
    pub created_at: UtcTimestamp,
    pub updated_at: UtcTimestamp,
    pub actor: Option<UserId>,
}

impl Camera {
    /// Create a new camera in an enabled state.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: CameraId,
        tenant_id: TenantId,
        device_id: DeviceId,
        code: impl Into<String>,
        name: impl Into<String>,
        sensitivity: Sensitivity,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<Self, PlatformError> {
        let code = code.into();
        validate_code(&code)?;
        let now = clock.now();
        Ok(Self {
            id,
            tenant_id,
            device_id,
            area_id: None,
            code,
            name: name.into(),
            sensitivity,
            is_enabled: true,
            revision: Revision::initial(),
            created_at: now,
            updated_at: now,
            actor,
        })
    }

    /// Reconstruct a camera from persisted parts.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        id: CameraId,
        tenant_id: TenantId,
        device_id: DeviceId,
        area_id: Option<AreaId>,
        code: impl Into<String>,
        name: impl Into<String>,
        sensitivity: Sensitivity,
        is_enabled: bool,
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
            device_id,
            area_id,
            code,
            name: name.into(),
            sensitivity,
            is_enabled,
            revision,
            created_at,
            updated_at,
            actor,
        })
    }

    /// Update sensitivity independently of device lifecycle.
    pub fn set_sensitivity(
        &mut self,
        sensitivity: Sensitivity,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) {
        self.sensitivity = sensitivity;
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
    }

    /// Enable or disable the camera view without deleting it.
    pub fn set_enabled(&mut self, enabled: bool, clock: &dyn Clock, actor: Option<UserId>) {
        self.is_enabled = enabled;
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
    }

    /// Assign the camera to an area within the same tenant.
    pub fn set_area(&mut self, area_id: Option<AreaId>, clock: &dyn Clock, actor: Option<UserId>) {
        self.area_id = area_id;
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
    }

    /// Rename the camera.
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
            "camera code must not be empty",
        ));
    }
    if code.len() > 128 {
        return Err(PlatformError::invalid(
            "code",
            "camera code must be at most 128 characters",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use foundation::FakeClock;

    fn id(s: &str) -> CameraId {
        CameraId::parse_str(s).unwrap_or_else(|e| panic!("{e}"))
    }

    fn device() -> DeviceId {
        DeviceId::parse_str("018e0000-0000-0000-0000-000000000001")
            .unwrap_or_else(|e| panic!("{e}"))
    }

    fn tenant() -> TenantId {
        TenantId::parse_str("018e1234-5678-7abc-8def-0123456789ab")
            .unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn sensitivity_and_enabled_are_independent() {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        let mut camera = Camera::new(
            id("018e0000-0000-0000-0000-000000000002"),
            tenant(),
            device(),
            "cam-1",
            "Camera 1",
            Sensitivity::Low,
            &clock,
            None,
        )
        .unwrap_or_else(|e| panic!("{e}"));

        camera.set_sensitivity(Sensitivity::Critical, &clock, None);
        camera.set_enabled(false, &clock, None);

        assert_eq!(camera.sensitivity, Sensitivity::Critical);
        assert!(!camera.is_enabled);
        assert_eq!(camera.revision.value(), 3);
    }
}
