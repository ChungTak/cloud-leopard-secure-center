//! Tenant aggregate.

use foundation::{Clock, PlatformError, Revision, TenantId, UserId, UtcTimestamp};

/// Lifecycle status of a tenant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TenantStatus {
    /// Active and operational.
    Active,
    /// Suspended; no new sessions allowed.
    Suspended,
    /// Closed and pending cleanup; terminal state.
    Closed,
}

impl TenantStatus {
    /// Parse a status string into the typed enum.
    pub fn parse(input: &str) -> Result<Self, PlatformError> {
        match input {
            "active" => Ok(Self::Active),
            "suspended" => Ok(Self::Suspended),
            "closed" | "terminated" => Ok(Self::Closed),
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
            Self::Closed => "closed",
        }
    }
}

/// A tenant (customer organization container).
#[derive(Debug, Clone)]
pub struct Tenant {
    /// Unique tenant identifier.
    pub id: TenantId,
    /// Immutable human-readable tenant code.
    pub code: String,
    /// Display name.
    pub name: String,
    /// Default locale, e.g. "en-US".
    pub locale: String,
    /// Default timezone, e.g. "UTC".
    pub timezone: String,
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
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: TenantId,
        code: impl Into<String>,
        name: impl Into<String>,
        locale: Option<impl Into<String>>,
        timezone: Option<impl Into<String>>,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<Self, PlatformError> {
        let code = code.into();
        validate_code(&code)?;
        let now = clock.now();
        Ok(Self {
            id,
            code,
            name: name.into(),
            locale: locale.map_or_else(|| "en-US".to_string(), Into::into),
            timezone: timezone.map_or_else(|| "UTC".to_string(), Into::into),
            status: TenantStatus::Active,
            revision: Revision::initial(),
            created_at: now,
            updated_at: now,
            actor,
        })
    }

    /// Reconstruct a tenant from persisted parts.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        id: TenantId,
        code: impl Into<String>,
        name: impl Into<String>,
        locale: impl Into<String>,
        timezone: impl Into<String>,
        status: TenantStatus,
        revision: Revision,
        created_at: UtcTimestamp,
        updated_at: UtcTimestamp,
        actor: Option<UserId>,
    ) -> Result<Self, PlatformError> {
        let code = code.into();
        validate_code(&code)?;
        Ok(Self {
            id,
            code,
            name: name.into(),
            locale: locale.into(),
            timezone: timezone.into(),
            status,
            revision,
            created_at,
            updated_at,
            actor,
        })
    }

    /// Rename the tenant and bump the revision.
    pub fn rename(
        &mut self,
        name: impl Into<String>,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<(), PlatformError> {
        self.ensure_not_closed("rename")?;
        self.name = name.into();
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
        Ok(())
    }

    /// Update the default locale.
    pub fn set_locale(
        &mut self,
        locale: impl Into<String>,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<(), PlatformError> {
        self.ensure_not_closed("set_locale")?;
        self.locale = locale.into();
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
        Ok(())
    }

    /// Update the default timezone.
    pub fn set_timezone(
        &mut self,
        timezone: impl Into<String>,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<(), PlatformError> {
        self.ensure_not_closed("set_timezone")?;
        self.timezone = timezone.into();
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
        Ok(())
    }

    /// Suspend the tenant.
    pub fn suspend(
        &mut self,
        clock: &dyn Clock,
        actor: Option<UserId>,
    ) -> Result<(), PlatformError> {
        self.ensure_not_closed("suspend")?;
        self.status = TenantStatus::Suspended;
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
        Ok(())
    }

    /// Close the tenant. This is a terminal state.
    pub fn close(&mut self, clock: &dyn Clock, actor: Option<UserId>) {
        self.status = TenantStatus::Closed;
        self.updated_at = clock.now();
        self.actor = actor;
        self.revision = self.revision.next();
    }

    /// Whether the tenant is active and may start new sessions.
    pub fn allows_new_sessions(&self) -> bool {
        self.status == TenantStatus::Active
    }

    /// Whether the tenant has been closed.
    pub fn is_closed(&self) -> bool {
        self.status == TenantStatus::Closed
    }

    fn ensure_not_closed(&self, operation: &str) -> Result<(), PlatformError> {
        if self.is_closed() {
            return Err(PlatformError::invalid(
                "tenant_status",
                format!("cannot {operation} a closed tenant"),
            ));
        }
        Ok(())
    }
}

fn validate_code(code: &str) -> Result<(), PlatformError> {
    if code.is_empty() {
        return Err(PlatformError::invalid(
            "tenant_code",
            "tenant code must not be empty",
        ));
    }
    if code.len() > 64 {
        return Err(PlatformError::invalid(
            "tenant_code",
            "tenant code must be at most 64 characters",
        ));
    }
    if code.trim() != code || code.contains(' ') {
        return Err(PlatformError::invalid(
            "tenant_code",
            "tenant code must not contain leading, trailing, or internal whitespace",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use foundation::FakeClock;

    fn tenant_id() -> Result<TenantId, PlatformError> {
        TenantId::parse_str("018e1234-5678-7abc-8def-0123456789ab")
    }

    fn tenant() -> Result<Tenant, PlatformError> {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        Tenant::new(
            tenant_id()?,
            "acme",
            "Acme Corp",
            Option::<&str>::None,
            Option::<&str>::None,
            &clock,
            None,
        )
    }

    #[test]
    fn tenant_has_defaults() -> Result<(), PlatformError> {
        let tenant = tenant()?;
        assert_eq!(tenant.locale, "en-US");
        assert_eq!(tenant.timezone, "UTC");
        assert!(tenant.allows_new_sessions());
        Ok(())
    }

    #[test]
    fn empty_code_is_rejected() -> Result<(), PlatformError> {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        let result = Tenant::new(
            tenant_id()?,
            "",
            "Name",
            Option::<&str>::None,
            Option::<&str>::None,
            &clock,
            None,
        );
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn whitespace_in_code_is_rejected() -> Result<(), PlatformError> {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        let result = Tenant::new(
            tenant_id()?,
            "ac me",
            "Name",
            Option::<&str>::None,
            Option::<&str>::None,
            &clock,
            None,
        );
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn suspended_tenant_disallows_new_sessions() -> Result<(), PlatformError> {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        let mut tenant = tenant()?;
        tenant.suspend(&clock, None)?;
        assert!(!tenant.allows_new_sessions());
        Ok(())
    }

    #[test]
    fn closed_tenant_is_terminal_and_rejects_modifications() -> Result<(), PlatformError> {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        let mut tenant = tenant()?;
        tenant.close(&clock, None);
        assert!(tenant.is_closed());
        assert!(tenant.rename("New", &clock, None).is_err());
        assert!(tenant.suspend(&clock, None).is_err());
        assert!(tenant.set_locale("fr-FR", &clock, None).is_err());
        Ok(())
    }

    #[test]
    fn rename_bumps_revision() -> Result<(), PlatformError> {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        let mut tenant = tenant()?;
        let before = tenant.revision;
        tenant.rename("Acme Updated", &clock, None)?;
        assert_eq!(tenant.revision.value(), before.value() + 1);
        Ok(())
    }
}
