//! External binding aggregate that maps upstream identifiers to catalog resources.

use crate::tag::ResourceType;
use foundation::{
    Clock, ErrorCode, ExternalBindingId, PlatformError, Revision, TenantId, UserId, UtcTimestamp,
    uuid::Uuid,
};

/// Lifecycle state of an external binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalBindingState {
    Pending,
    Active,
    Stale,
    Conflict,
    Disabled,
}

impl ExternalBindingState {
    /// Serialize to the lowercase form stored in the database.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Stale => "stale",
            Self::Conflict => "conflict",
            Self::Disabled => "disabled",
        }
    }

    /// Parse the serialized form.
    pub fn parse(input: &str) -> Result<Self, PlatformError> {
        match input {
            "pending" => Ok(Self::Pending),
            "active" => Ok(Self::Active),
            "stale" => Ok(Self::Stale),
            "conflict" => Ok(Self::Conflict),
            "disabled" => Ok(Self::Disabled),
            _ => Err(PlatformError::invalid(
                "state",
                format!("unknown external binding state: {input}"),
            )),
        }
    }

    /// Whether this binding is considered effective for authorization or queries.
    pub fn is_effective(&self) -> bool {
        matches!(self, Self::Active | Self::Pending)
    }
}

/// A binding between an upstream external identifier and an internal catalog resource.
#[derive(Debug, Clone)]
pub struct ExternalBinding {
    pub id: ExternalBindingId,
    pub tenant_id: TenantId,
    pub resource_type: ResourceType,
    pub resource_id: Uuid,
    pub external_ref: String,
    pub external_kind: String,
    pub state: ExternalBindingState,
    pub activated_at: Option<UtcTimestamp>,
    pub revision: Revision,
    pub created_at: UtcTimestamp,
    pub updated_at: UtcTimestamp,
    pub actor: Option<UserId>,
}

impl ExternalBinding {
    /// Create a new binding in the Pending state from an auto-match.
    pub fn auto_match(
        id: ExternalBindingId,
        tenant_id: TenantId,
        resource_type: ResourceType,
        resource_id: Uuid,
        external_ref: impl Into<String>,
        external_kind: impl Into<String>,
        clock: &dyn Clock,
    ) -> Result<Self, PlatformError> {
        let external_ref = external_ref.into();
        let external_kind = external_kind.into();
        validate_ref(&external_ref)?;
        validate_kind(&external_kind)?;
        let now = clock.now();
        Ok(Self {
            id,
            tenant_id,
            resource_type,
            resource_id,
            external_ref,
            external_kind,
            state: ExternalBindingState::Pending,
            activated_at: None,
            revision: Revision::initial(),
            created_at: now,
            updated_at: now,
            actor: None,
        })
    }

    /// Reconstruct a binding from persisted parts.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        id: ExternalBindingId,
        tenant_id: TenantId,
        resource_type: ResourceType,
        resource_id: Uuid,
        external_ref: impl Into<String>,
        external_kind: impl Into<String>,
        state: ExternalBindingState,
        activated_at: Option<UtcTimestamp>,
        revision: Revision,
        created_at: UtcTimestamp,
        updated_at: UtcTimestamp,
        actor: Option<UserId>,
    ) -> Result<Self, PlatformError> {
        let external_ref = external_ref.into();
        let external_kind = external_kind.into();
        validate_ref(&external_ref)?;
        validate_kind(&external_kind)?;
        Ok(Self {
            id,
            tenant_id,
            resource_type,
            resource_id,
            external_ref,
            external_kind,
            state,
            activated_at,
            revision,
            created_at,
            updated_at,
            actor,
        })
    }

    /// Activate a Pending binding; repository enforces global uniqueness.
    pub fn activate(
        &mut self,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<(), PlatformError> {
        if self.state != ExternalBindingState::Pending {
            return Err(PlatformError::new(
                ErrorCode::Conflict,
                "only pending bindings can be activated".to_string(),
            ));
        }
        self.state = ExternalBindingState::Active;
        self.activated_at = Some(clock.now());
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
        Ok(())
    }

    /// Mark the binding as conflicting with another active binding.
    pub fn mark_conflict(&mut self, clock: &dyn Clock, actor: Option<UserId>) {
        self.state = ExternalBindingState::Conflict;
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
    }

    /// Mark the binding as stale when the upstream source reports it missing.
    pub fn mark_stale(&mut self, clock: &dyn Clock, actor: Option<UserId>) {
        self.state = ExternalBindingState::Stale;
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
    }

    /// Disable the binding without deleting it, preserving history.
    pub fn disable(&mut self, clock: &dyn Clock, actor: Option<UserId>) {
        self.state = ExternalBindingState::Disabled;
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
    }
}

fn validate_ref(external_ref: &str) -> Result<(), PlatformError> {
    if external_ref.trim().is_empty() {
        return Err(PlatformError::invalid(
            "external_ref",
            "external reference must not be empty",
        ));
    }
    if external_ref.len() > 256 {
        return Err(PlatformError::invalid(
            "external_ref",
            "external reference must be at most 256 characters",
        ));
    }
    Ok(())
}

fn validate_kind(external_kind: &str) -> Result<(), PlatformError> {
    if external_kind.trim().is_empty() {
        return Err(PlatformError::invalid(
            "external_kind",
            "external kind must not be empty",
        ));
    }
    if external_kind.len() > 64 {
        return Err(PlatformError::invalid(
            "external_kind",
            "external kind must be at most 64 characters",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use foundation::FakeClock;

    fn id(s: &str) -> ExternalBindingId {
        ExternalBindingId::parse_str(s).unwrap_or_else(|e| panic!("{e}"))
    }

    fn tenant() -> TenantId {
        TenantId::parse_str("018e1234-5678-7abc-8def-0123456789ab")
            .unwrap_or_else(|e| panic!("{e}"))
    }

    fn resource() -> Uuid {
        Uuid::parse_str("018e0000-0000-0000-0000-000000000001").unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn auto_match_creates_pending() {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        let binding = ExternalBinding::auto_match(
            id("018e0000-0000-0000-0000-000000000002"),
            tenant(),
            ResourceType::Device,
            resource(),
            "upstream-123",
            "serial",
            &clock,
        )
        .unwrap_or_else(|e| panic!("{e}"));

        assert_eq!(binding.state, ExternalBindingState::Pending);
    }

    #[test]
    fn activation_requires_pending() {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        let mut binding = ExternalBinding::auto_match(
            id("018e0000-0000-0000-0000-000000000002"),
            tenant(),
            ResourceType::Device,
            resource(),
            "upstream-123",
            "serial",
            &clock,
        )
        .unwrap_or_else(|e| panic!("{e}"));

        binding
            .activate(&clock, None)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(binding.state, ExternalBindingState::Active);

        assert!(binding.activate(&clock, None).is_err());
    }

    #[test]
    fn disable_preserves_record_in_state() {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        let mut binding = ExternalBinding::auto_match(
            id("018e0000-0000-0000-0000-000000000002"),
            tenant(),
            ResourceType::Device,
            resource(),
            "upstream-123",
            "serial",
            &clock,
        )
        .unwrap_or_else(|e| panic!("{e}"));

        binding.disable(&clock, None);
        assert_eq!(binding.state, ExternalBindingState::Disabled);
    }
}
