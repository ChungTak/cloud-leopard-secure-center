//! Spatial aggregate: sites, buildings, floors, and areas with a closure tree.

#![allow(clippy::too_many_arguments)]

use foundation::{
    AreaId, BuildingId, Clock, FloorId, OrganizationId, PlatformError, Revision, SiteId, TenantId,
    UserId, UtcTimestamp,
};

/// A physical site (campus, building group, etc.) within a tenant.
#[derive(Debug, Clone)]
pub struct Site {
    pub id: SiteId,
    pub tenant_id: TenantId,
    pub organization_unit_id: Option<OrganizationId>,
    pub code: String,
    pub name: String,
    pub address: String,
    pub timezone: String,
    pub revision: Revision,
    pub created_at: UtcTimestamp,
    pub updated_at: UtcTimestamp,
    pub actor: Option<UserId>,
}

/// A building located at a site.
#[derive(Debug, Clone)]
pub struct Building {
    pub id: BuildingId,
    pub tenant_id: TenantId,
    pub site_id: SiteId,
    pub code: String,
    pub name: String,
    pub revision: Revision,
    pub created_at: UtcTimestamp,
    pub updated_at: UtcTimestamp,
    pub actor: Option<UserId>,
}

/// A floor within a building.
#[derive(Debug, Clone)]
pub struct Floor {
    pub id: FloorId,
    pub tenant_id: TenantId,
    pub building_id: BuildingId,
    pub code: String,
    pub name: String,
    pub level: i32,
    pub revision: Revision,
    pub created_at: UtcTimestamp,
    pub updated_at: UtcTimestamp,
    pub actor: Option<UserId>,
}

/// A physical or logical area, optionally nested under a floor or another area.
#[derive(Debug, Clone)]
pub struct Area {
    pub id: AreaId,
    pub tenant_id: TenantId,
    pub floor_id: Option<FloorId>,
    pub parent_id: Option<AreaId>,
    pub code: String,
    pub name: String,
    pub coordinate_system: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub altitude: Option<f64>,
    pub revision: Revision,
    pub created_at: UtcTimestamp,
    pub updated_at: UtcTimestamp,
    pub actor: Option<UserId>,
}

impl Site {
    pub fn new(
        id: SiteId,
        tenant_id: TenantId,
        organization_unit_id: Option<OrganizationId>,
        code: impl Into<String>,
        name: impl Into<String>,
        address: impl Into<String>,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<Self, PlatformError> {
        let code = code.into();
        validate_code(&code)?;
        let address = address.into();
        if address.len() > 256 {
            return Err(PlatformError::invalid(
                "site_address",
                "site address must be at most 256 characters",
            ));
        }
        let now = clock.now();
        Ok(Self {
            id,
            tenant_id,
            organization_unit_id,
            code,
            name: name.into(),
            address,
            timezone: "UTC".to_string(),
            revision: Revision::initial(),
            created_at: now,
            updated_at: now,
            actor,
        })
    }

    pub fn from_parts(
        id: SiteId,
        tenant_id: TenantId,
        organization_unit_id: Option<OrganizationId>,
        code: impl Into<String>,
        name: impl Into<String>,
        address: impl Into<String>,
        timezone: impl Into<String>,
        revision: Revision,
        created_at: UtcTimestamp,
        updated_at: UtcTimestamp,
        actor: Option<UserId>,
    ) -> Result<Self, PlatformError> {
        let code = code.into();
        validate_code(&code)?;
        let address = address.into();
        if address.len() > 256 {
            return Err(PlatformError::invalid(
                "site_address",
                "site address must be at most 256 characters",
            ));
        }
        Ok(Self {
            id,
            tenant_id,
            organization_unit_id,
            code,
            name: name.into(),
            address,
            timezone: timezone.into(),
            revision,
            created_at,
            updated_at,
            actor,
        })
    }

    pub fn rename(&mut self, name: impl Into<String>, clock: &dyn Clock, actor: Option<UserId>) {
        self.name = name.into();
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
    }

    pub fn set_address(
        &mut self,
        address: impl Into<String>,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<(), PlatformError> {
        let address = address.into();
        if address.len() > 256 {
            return Err(PlatformError::invalid(
                "site_address",
                "site address must be at most 256 characters",
            ));
        }
        self.address = address;
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
        Ok(())
    }

    pub fn set_timezone(
        &mut self,
        timezone: impl Into<String>,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) {
        self.timezone = timezone.into();
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
    }
}

impl Building {
    pub fn new(
        id: BuildingId,
        tenant_id: TenantId,
        site_id: SiteId,
        code: impl Into<String>,
        name: impl Into<String>,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<Self, PlatformError> {
        let code = code.into();
        validate_code(&code)?;
        let now = clock.now();
        Ok(Self {
            id,
            tenant_id,
            site_id,
            code,
            name: name.into(),
            revision: Revision::initial(),
            created_at: now,
            updated_at: now,
            actor,
        })
    }

    pub fn from_parts(
        id: BuildingId,
        tenant_id: TenantId,
        site_id: SiteId,
        code: impl Into<String>,
        name: impl Into<String>,
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
            site_id,
            code,
            name: name.into(),
            revision,
            created_at,
            updated_at,
            actor,
        })
    }

    pub fn rename(&mut self, name: impl Into<String>, clock: &dyn Clock, actor: Option<UserId>) {
        self.name = name.into();
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
    }
}

impl Floor {
    pub fn new(
        id: FloorId,
        tenant_id: TenantId,
        building_id: BuildingId,
        code: impl Into<String>,
        name: impl Into<String>,
        level: i32,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<Self, PlatformError> {
        let code = code.into();
        validate_code(&code)?;
        if !(-10..=200).contains(&level) {
            return Err(PlatformError::invalid(
                "floor_level",
                "floor level must be between -10 and 200",
            ));
        }
        let now = clock.now();
        Ok(Self {
            id,
            tenant_id,
            building_id,
            code,
            name: name.into(),
            level,
            revision: Revision::initial(),
            created_at: now,
            updated_at: now,
            actor,
        })
    }

    pub fn from_parts(
        id: FloorId,
        tenant_id: TenantId,
        building_id: BuildingId,
        code: impl Into<String>,
        name: impl Into<String>,
        level: i32,
        revision: Revision,
        created_at: UtcTimestamp,
        updated_at: UtcTimestamp,
        actor: Option<UserId>,
    ) -> Result<Self, PlatformError> {
        let code = code.into();
        validate_code(&code)?;
        if !(-10..=200).contains(&level) {
            return Err(PlatformError::invalid(
                "floor_level",
                "floor level must be between -10 and 200",
            ));
        }
        Ok(Self {
            id,
            tenant_id,
            building_id,
            code,
            name: name.into(),
            level,
            revision,
            created_at,
            updated_at,
            actor,
        })
    }

    pub fn rename(&mut self, name: impl Into<String>, clock: &dyn Clock, actor: Option<UserId>) {
        self.name = name.into();
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
    }

    pub fn set_level(
        &mut self,
        level: i32,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<(), PlatformError> {
        if !(-10..=200).contains(&level) {
            return Err(PlatformError::invalid(
                "floor_level",
                "floor level must be between -10 and 200",
            ));
        }
        self.level = level;
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
        Ok(())
    }
}

impl Area {
    pub fn new(
        id: AreaId,
        tenant_id: TenantId,
        floor_id: Option<FloorId>,
        parent_id: Option<AreaId>,
        code: impl Into<String>,
        name: impl Into<String>,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<Self, PlatformError> {
        let code = code.into();
        validate_code(&code)?;
        let now = clock.now();
        Ok(Self {
            id,
            tenant_id,
            floor_id,
            parent_id,
            code,
            name: name.into(),
            coordinate_system: "WGS84".to_string(),
            latitude: None,
            longitude: None,
            altitude: None,
            revision: Revision::initial(),
            created_at: now,
            updated_at: now,
            actor,
        })
    }

    pub fn from_parts(
        id: AreaId,
        tenant_id: TenantId,
        floor_id: Option<FloorId>,
        parent_id: Option<AreaId>,
        code: impl Into<String>,
        name: impl Into<String>,
        coordinate_system: impl Into<String>,
        latitude: Option<f64>,
        longitude: Option<f64>,
        altitude: Option<f64>,
        revision: Revision,
        created_at: UtcTimestamp,
        updated_at: UtcTimestamp,
        actor: Option<UserId>,
    ) -> Result<Self, PlatformError> {
        let code = code.into();
        validate_code(&code)?;
        let coordinate_system = coordinate_system.into();
        validate_coordinates(&coordinate_system, latitude, longitude)?;
        Ok(Self {
            id,
            tenant_id,
            floor_id,
            parent_id,
            code,
            name: name.into(),
            coordinate_system,
            latitude,
            longitude,
            altitude,
            revision,
            created_at,
            updated_at,
            actor,
        })
    }

    pub fn rename(&mut self, name: impl Into<String>, clock: &dyn Clock, actor: Option<UserId>) {
        self.name = name.into();
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
    }

    pub fn set_coordinates(
        &mut self,
        coordinate_system: impl Into<String>,
        latitude: Option<f64>,
        longitude: Option<f64>,
        altitude: Option<f64>,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<(), PlatformError> {
        let coordinate_system = coordinate_system.into();
        validate_coordinates(&coordinate_system, latitude, longitude)?;
        self.coordinate_system = coordinate_system;
        self.latitude = latitude;
        self.longitude = longitude;
        self.altitude = altitude;
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
        Ok(())
    }

    pub fn set_parent(
        &mut self,
        parent_id: Option<AreaId>,
        descendants: &[AreaId],
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<(), PlatformError> {
        if let Some(pid) = parent_id {
            if pid == self.id {
                return Err(PlatformError::invalid(
                    "parent_id",
                    "an area cannot be its own parent",
                ));
            }
            if descendants.contains(&pid) {
                return Err(PlatformError::invalid(
                    "parent_id",
                    "cannot move an area under one of its descendants",
                ));
            }
        }
        self.parent_id = parent_id;
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
        Ok(())
    }
}

fn validate_code(code: &str) -> Result<(), PlatformError> {
    if code.is_empty() {
        return Err(PlatformError::invalid(
            "spatial_code",
            "code must not be empty",
        ));
    }
    if code.len() > 64 {
        return Err(PlatformError::invalid(
            "spatial_code",
            "code must be at most 64 characters",
        ));
    }
    if code.trim() != code || code.contains(' ') {
        return Err(PlatformError::invalid(
            "spatial_code",
            "code must not contain leading, trailing, or internal whitespace",
        ));
    }
    Ok(())
}

fn validate_coordinates(
    coordinate_system: &str,
    latitude: Option<f64>,
    longitude: Option<f64>,
) -> Result<(), PlatformError> {
    if coordinate_system != "WGS84" {
        return Err(PlatformError::invalid(
            "coordinate_system",
            "only WGS84 is supported",
        ));
    }
    if let (Some(lat), Some(lon)) = (latitude, longitude) {
        if !(-90.0..=90.0).contains(&lat) {
            return Err(PlatformError::invalid(
                "latitude",
                "latitude must be between -90 and 90",
            ));
        }
        if !(-180.0..=180.0).contains(&lon) {
            return Err(PlatformError::invalid(
                "longitude",
                "longitude must be between -180 and 180",
            ));
        }
    } else if latitude.is_some() || longitude.is_some() {
        return Err(PlatformError::invalid(
            "coordinates",
            "latitude and longitude must be provided together",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use foundation::FakeClock;

    fn tenant_id() -> TenantId {
        match TenantId::parse_str("018e1234-5678-7abc-8def-0123456789ab") {
            Ok(v) => v,
            Err(e) => panic!("{e}"),
        }
    }

    fn site_id() -> SiteId {
        match SiteId::parse_str("018e1234-5678-7abc-8def-0123456789ac") {
            Ok(v) => v,
            Err(e) => panic!("{e}"),
        }
    }

    #[test]
    fn empty_site_code_is_rejected() {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        let result = Site::new(
            site_id(),
            tenant_id(),
            None,
            "",
            "Name",
            "Addr",
            &clock,
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn site_address_length_is_bounded() {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        let long = "a".repeat(257);
        let result = Site::new(site_id(), tenant_id(), None, "hq", "HQ", long, &clock, None);
        assert!(result.is_err());
    }

    #[test]
    fn floor_level_is_bounded() {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        let building_id = BuildingId::parse_str("018e1234-5678-7abc-8def-0123456789ad")
            .unwrap_or_else(|e| panic!("{e}"));
        let result = Floor::new(
            FloorId::parse_str("018e1234-5678-7abc-8def-0123456789ae")
                .unwrap_or_else(|e| panic!("{e}")),
            tenant_id(),
            building_id,
            "f1",
            "Floor 1",
            500,
            &clock,
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn area_coordinates_are_validated() {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        let mut area = Area::new(
            AreaId::parse_str("018e1234-5678-7abc-8def-0123456789af")
                .unwrap_or_else(|e| panic!("{e}")),
            tenant_id(),
            None,
            None,
            "lobby",
            "Lobby",
            &clock,
            None,
        )
        .unwrap_or_else(|e| panic!("{e}"));

        let result = area.set_coordinates("WGS84", Some(100.0), Some(0.0), None, &clock, None);
        assert!(result.is_err());

        let result = area.set_coordinates("WGS84", Some(0.0), Some(200.0), None, &clock, None);
        assert!(result.is_err());

        let result = area.set_coordinates("OTHER", Some(0.0), Some(0.0), None, &clock, None);
        assert!(result.is_err());
    }

    #[test]
    fn area_parent_rejects_self_and_descendants() -> Result<(), PlatformError> {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        let mut area = Area::new(
            AreaId::parse_str("018e1234-5678-7abc-8def-0123456789af")
                .unwrap_or_else(|e| panic!("{e}")),
            tenant_id(),
            None,
            None,
            "lobby",
            "Lobby",
            &clock,
            None,
        )?;
        let child_id = AreaId::parse_str("018e1234-5678-7abc-8def-0123456789b0")
            .unwrap_or_else(|e| panic!("{e}"));

        let result = area.set_parent(Some(area.id), &[area.id], &clock, None);
        assert!(result.is_err());

        let result = area.set_parent(Some(child_id), &[area.id, child_id], &clock, None);
        assert!(result.is_err());

        Ok(())
    }
}
