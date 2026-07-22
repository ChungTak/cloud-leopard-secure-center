//! Tenant aggregate.

use foundation::{Clock, PlatformError, Revision, TenantId, UserId, UtcTimestamp};

/// Lifecycle status of a tenant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TenantStatus {
    /// Active and operational.
    Active,
    /// Suspended; no new operations allowed.
    Suspended,
    /// Terminated and pending cleanup.
    Terminated,
}

impl TenantStatus {
    /// Parse a status string into the typed enum.
    pub fn parse(input: &str) -> Result<Self, PlatformError> {
        match input {
            "active" => Ok(Self::Active),
            "suspended" => Ok(Self::Suspended),
            "terminated" => Ok(Self::Terminated),
            _ => Err(PlatformError::invalid(
                "tenant_status",
                format!("unsupported status: {input}"),
            )),
        }
    }

    /// Return the canonical database representation.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Suspended => "suspended",
            Self::Terminated => "terminated",
        }
    }
}

/// A tenant (customer organization container).
#[derive(Debug, Clone)]
pub struct Tenant {
    /// Unique tenant identifier.
    pub id: TenantId,
    /// Display name.
    pub name: String,
    /// Current status.
    pub status: TenantStatus,
    /// Optimistic lock version.
    pub revision: Revision,
    /// Creation timestamp.
    pub created_at: UtcTimestamp,
    /// Last update timestamp.
    pub updated_at: UtcTimestamp,
    /// Actor that performed the last change.
    pub actor: Option<UserId>,
}

impl Tenant {
    /// Create a new active tenant.
    pub fn new(
        id: TenantId,
        name: impl Into<String>,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Self {
        let now = clock.now();
        Self {
            id,
            name: name.into(),
            status: TenantStatus::Active,
            revision: Revision::initial(),
            created_at: now,
            updated_at: now,
            actor,
        }
    }

    /// Rename the tenant and bump the revision.
    pub fn rename(&mut self, name: impl Into<String>, clock: &dyn Clock, actor: Option<UserId>) {
        self.name = name.into();
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
    }

    /// Suspend the tenant.
    pub fn suspend(&mut self, clock: &dyn Clock, actor: Option<UserId>) {
        self.status = TenantStatus::Suspended;
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
    }

    /// Terminate the tenant.
    pub fn terminate(&mut self, clock: &dyn Clock, actor: Option<UserId>) {
        self.status = TenantStatus::Terminated;
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
    }
}
