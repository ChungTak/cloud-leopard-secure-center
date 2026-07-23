//! Tag aggregate for resource cataloging.

use foundation::{
    Clock, PlatformError, Revision, TagId, TenantId, UserId, UtcTimestamp, uuid::Uuid,
};

/// Supported resource types in the tag and binding registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    Device,
    Camera,
    OrganizationUnit,
    Area,
    Site,
    Building,
    Floor,
    User,
}

impl ResourceType {
    /// Serialize to the lowercase registry name.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Device => "device",
            Self::Camera => "camera",
            Self::OrganizationUnit => "organization_unit",
            Self::Area => "area",
            Self::Site => "site",
            Self::Building => "building",
            Self::Floor => "floor",
            Self::User => "user",
        }
    }

    /// Parse the serialized registry name.
    pub fn parse(input: &str) -> Result<Self, PlatformError> {
        match input {
            "device" => Ok(Self::Device),
            "camera" => Ok(Self::Camera),
            "organization_unit" => Ok(Self::OrganizationUnit),
            "area" => Ok(Self::Area),
            "site" => Ok(Self::Site),
            "building" => Ok(Self::Building),
            "floor" => Ok(Self::Floor),
            "user" => Ok(Self::User),
            _ => Err(PlatformError::invalid(
                "resource_type",
                format!("unknown resource type: {input}"),
            )),
        }
    }
}

/// Maximum number of tags that can be attached to a single resource.
pub const MAX_TAGS_PER_RESOURCE: usize = 50;

/// A tag attached to a resource in the catalog.
#[derive(Debug, Clone)]
pub struct Tag {
    pub id: TagId,
    pub tenant_id: TenantId,
    pub resource_type: ResourceType,
    pub resource_id: Uuid,
    pub key: String,
    pub value: String,
    pub revision: Revision,
    pub created_at: UtcTimestamp,
    pub updated_at: UtcTimestamp,
    pub actor: Option<UserId>,
}

impl Tag {
    /// Create a new tag with normalized key/value.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: TagId,
        tenant_id: TenantId,
        resource_type: ResourceType,
        resource_id: Uuid,
        key: impl Into<String>,
        value: impl Into<String>,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<Self, PlatformError> {
        let key = key.into();
        let value = value.into();
        validate_key(&key)?;
        validate_value(&value)?;
        let now = clock.now();
        Ok(Self {
            id,
            tenant_id,
            resource_type,
            resource_id,
            key: normalize_key(&key),
            value: value.trim().to_string(),
            revision: Revision::initial(),
            created_at: now,
            updated_at: now,
            actor,
        })
    }

    /// Reconstruct a tag from persisted parts.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        id: TagId,
        tenant_id: TenantId,
        resource_type: ResourceType,
        resource_id: Uuid,
        key: impl Into<String>,
        value: impl Into<String>,
        revision: Revision,
        created_at: UtcTimestamp,
        updated_at: UtcTimestamp,
        actor: Option<UserId>,
    ) -> Result<Self, PlatformError> {
        let key = key.into();
        let value = value.into();
        validate_key(&key)?;
        validate_value(&value)?;
        Ok(Self {
            id,
            tenant_id,
            resource_type,
            resource_id,
            key: normalize_key(&key),
            value: value.trim().to_string(),
            revision,
            created_at,
            updated_at,
            actor,
        })
    }

    /// Update the tag value.
    pub fn set_value(
        &mut self,
        value: impl Into<String>,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<(), PlatformError> {
        let value = value.into();
        validate_value(&value)?;
        self.value = value.trim().to_string();
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
        Ok(())
    }
}

fn normalize_key(key: &str) -> String {
    key.trim().to_lowercase()
}

fn validate_key(key: &str) -> Result<(), PlatformError> {
    let key = key.trim();
    if key.is_empty() {
        return Err(PlatformError::invalid("key", "tag key must not be empty"));
    }
    if key.len() > 64 {
        return Err(PlatformError::invalid(
            "key",
            "tag key must be at most 64 characters",
        ));
    }
    Ok(())
}

fn validate_value(value: &str) -> Result<(), PlatformError> {
    if value.trim().is_empty() {
        return Err(PlatformError::invalid(
            "value",
            "tag value must not be empty",
        ));
    }
    if value.len() > 256 {
        return Err(PlatformError::invalid(
            "value",
            "tag value must be at most 256 characters",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use foundation::FakeClock;

    fn id(s: &str) -> TagId {
        TagId::parse_str(s).unwrap_or_else(|e| panic!("{e}"))
    }

    fn tenant() -> TenantId {
        TenantId::parse_str("018e1234-5678-7abc-8def-0123456789ab")
            .unwrap_or_else(|e| panic!("{e}"))
    }

    fn resource() -> Uuid {
        Uuid::parse_str("018e0000-0000-0000-0000-000000000001").unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn key_is_normalized_and_value_is_trimmed() {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        let tag = Tag::new(
            id("018e0000-0000-0000-0000-000000000002"),
            tenant(),
            ResourceType::Device,
            resource(),
            "  Environment  ",
            "  Production  ",
            &clock,
            None,
        )
        .unwrap_or_else(|e| panic!("{e}"));

        assert_eq!(tag.key, "environment");
        assert_eq!(tag.value, "Production");
    }

    #[test]
    fn empty_key_is_rejected() {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        assert!(
            Tag::new(
                id("018e0000-0000-0000-0000-000000000002"),
                tenant(),
                ResourceType::Device,
                resource(),
                "   ",
                "v",
                &clock,
                None,
            )
            .is_err()
        );
    }
}
