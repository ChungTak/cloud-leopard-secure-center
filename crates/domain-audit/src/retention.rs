//! Retention policy, legal hold, and cleanup job domain types.

use foundation::{PlatformError, TenantId, UtcTimestamp};

/// Category of data subject to retention cleanup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RetentionTarget {
    AuditRecords,
    AuditEvents,
    LoginAttempts,
    Outbox,
    Inbox,
}

impl RetentionTarget {
    /// Human-readable target identifier used for storage and configuration.
    pub fn as_str(&self) -> &'static str {
        match self {
            RetentionTarget::AuditRecords => "audit.records",
            RetentionTarget::AuditEvents => "audit.events",
            RetentionTarget::LoginAttempts => "audit.login_attempts",
            RetentionTarget::Outbox => "infra.outbox_messages",
            RetentionTarget::Inbox => "infra.inbox_messages",
        }
    }

    /// Parse a target from its storage identifier.
    pub fn parse(value: &str) -> Result<Self, PlatformError> {
        match value {
            "audit.records" => Ok(RetentionTarget::AuditRecords),
            "audit.events" => Ok(RetentionTarget::AuditEvents),
            "audit.login_attempts" => Ok(RetentionTarget::LoginAttempts),
            "infra.outbox_messages" => Ok(RetentionTarget::Outbox),
            "infra.inbox_messages" => Ok(RetentionTarget::Inbox),
            _ => Err(PlatformError::invalid("target", "unknown retention target")),
        }
    }
}

/// Default retention policy for a target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionPolicy {
    pub target: RetentionTarget,
    pub days: u32,
    pub max_batch_size: u64,
}

impl RetentionPolicy {
    /// Create a policy after validating bounds.
    pub fn new(
        target: RetentionTarget,
        days: u32,
        max_batch_size: u64,
    ) -> Result<Self, PlatformError> {
        if days == 0 {
            return Err(PlatformError::invalid(
                "days",
                "retention period must be at least one day",
            ));
        }
        if max_batch_size == 0 {
            return Err(PlatformError::invalid(
                "max_batch_size",
                "cleanup batch size must be greater than zero",
            ));
        }
        Ok(Self {
            target,
            days,
            max_batch_size,
        })
    }

    /// Compute the cutoff timestamp for data older than this policy.
    pub fn cutoff(&self, now: UtcTimestamp) -> UtcTimestamp {
        let dt: chrono::DateTime<chrono::Utc> = now.into();
        (dt - chrono::Duration::days(i64::from(self.days))).into()
    }
}

/// Tenant-specific override of a retention policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TenantRetentionOverride {
    pub tenant_id: TenantId,
    pub target: RetentionTarget,
    pub days: u32,
}

impl TenantRetentionOverride {
    pub fn new(
        tenant_id: TenantId,
        target: RetentionTarget,
        days: u32,
    ) -> Result<Self, PlatformError> {
        if days == 0 {
            return Err(PlatformError::invalid(
                "days",
                "tenant retention override must be at least one day",
            ));
        }
        Ok(Self {
            tenant_id,
            target,
            days,
        })
    }
}

/// A legal hold placed on a resource that prevents cleanup of associated records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalHold {
    pub resource_type: String,
    pub resource_id: String,
    pub held_until: UtcTimestamp,
}

impl LegalHold {
    pub fn new(
        resource_type: impl Into<String>,
        resource_id: impl Into<String>,
        held_until: UtcTimestamp,
    ) -> Self {
        Self {
            resource_type: resource_type.into(),
            resource_id: resource_id.into(),
            held_until,
        }
    }

    pub fn is_active(&self, now: UtcTimestamp) -> bool {
        let now_dt: chrono::DateTime<chrono::Utc> = now.into();
        let held_dt: chrono::DateTime<chrono::Utc> = self.held_until.into();
        now_dt < held_dt
    }
}

/// Identity of a cleanup worker and the scope it has locked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupLease {
    pub target: RetentionTarget,
    pub partition: String,
    pub worker_id: String,
    pub lease_until: UtcTimestamp,
}

/// Summary returned after a cleanup batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CleanupBatchResult {
    pub rows_deleted: u64,
    pub finished: bool,
}
