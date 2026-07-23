//! Audit record aggregate.

use foundation::{Clock, PlatformError, TenantId, UtcTimestamp};

/// Risk level of an audited action, used to decide whether a write failure
/// should reject the operation or only alert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionRisk {
    Normal,
    High,
    Critical,
}

impl ActionRisk {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }

    pub fn parse(input: &str) -> Result<Self, PlatformError> {
        match input {
            "normal" => Ok(Self::Normal),
            "high" => Ok(Self::High),
            "critical" => Ok(Self::Critical),
            _ => Err(PlatformError::invalid(
                "action_risk",
                format!("unknown action risk: {input}"),
            )),
        }
    }

    /// Whether a failure to write this audit record should be silent or not.
    pub const fn must_not_be_silent(&self) -> bool {
        matches!(self, Self::High | Self::Critical)
    }
}

/// Outcome of an audited operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditResult {
    Success,
    Denied,
    Failure,
}

impl AuditResult {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Denied => "denied",
            Self::Failure => "failure",
        }
    }

    pub fn parse(input: &str) -> Result<Self, PlatformError> {
        match input {
            "success" => Ok(Self::Success),
            "denied" => Ok(Self::Denied),
            "failure" => Ok(Self::Failure),
            _ => Err(PlatformError::invalid(
                "audit_result",
                format!("unknown audit result: {input}"),
            )),
        }
    }
}

/// Details attached to an audit record.
#[derive(Debug, Clone)]
pub struct AuditDetails {
    pub schema: String,
    pub value: String,
}

impl AuditDetails {
    /// Maximum size of the JSON details payload in bytes.
    pub const MAX_SIZE: usize = 65536;

    /// Create details with a named schema and a JSON object value.
    pub fn new(schema: impl Into<String>, value: impl Into<String>) -> Result<Self, PlatformError> {
        let schema = schema.into();
        let value = value.into();
        if schema.trim().is_empty() {
            return Err(PlatformError::invalid(
                "details_schema",
                "details schema must not be empty",
            ));
        }
        if value.len() > Self::MAX_SIZE {
            return Err(PlatformError::invalid(
                "details",
                format!("details must not exceed {} bytes", Self::MAX_SIZE),
            ));
        }
        match serde_json::from_str::<serde_json::Value>(&value) {
            Ok(serde_json::Value::Object(_)) => {}
            Ok(_) => {
                return Err(PlatformError::invalid(
                    "details",
                    "details must be a JSON object",
                ));
            }
            Err(e) => {
                return Err(PlatformError::invalid(
                    "details",
                    format!("details must be valid JSON: {e}"),
                ));
            }
        }
        Ok(Self { schema, value })
    }
}

/// Strongly typed identifier for an audit record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuditRecordId(pub i64);

impl AuditRecordId {
    pub const fn new(id: i64) -> Self {
        Self(id)
    }

    pub const fn value(&self) -> i64 {
        self.0
    }
}

/// An immutable audit record describing a security-relevant action.
#[derive(Debug, Clone)]
pub struct AuditRecord {
    pub id: Option<AuditRecordId>,
    pub tenant_id: TenantId,
    pub actor_type: String,
    pub actor_id: String,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub result: AuditResult,
    pub risk: ActionRisk,
    pub request_id: Option<String>,
    pub trace_id: Option<String>,
    pub source_ip: Option<String>,
    pub before_digest: Option<String>,
    pub after_digest: Option<String>,
    pub occurred_at: UtcTimestamp,
    pub details: AuditDetails,
}

impl AuditRecord {
    /// Create a new audit record with the required fields.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tenant_id: TenantId,
        actor_type: impl Into<String>,
        actor_id: impl Into<String>,
        action: impl Into<String>,
        target_type: impl Into<String>,
        target_id: impl Into<String>,
        result: AuditResult,
        risk: ActionRisk,
        details: AuditDetails,
        clock: &dyn Clock,
    ) -> Result<Self, PlatformError> {
        let actor_type = actor_type.into();
        let actor_id = actor_id.into();
        let action = action.into();
        let target_type = target_type.into();
        let target_id = target_id.into();
        if action.trim().is_empty() {
            return Err(PlatformError::invalid(
                "action",
                "audit action must not be empty",
            ));
        }
        if actor_type.trim().is_empty() || actor_id.trim().is_empty() {
            return Err(PlatformError::invalid(
                "actor",
                "audit actor type and id must not be empty",
            ));
        }
        if target_type.trim().is_empty() || target_id.trim().is_empty() {
            return Err(PlatformError::invalid(
                "target",
                "audit target type and id must not be empty",
            ));
        }
        Ok(Self {
            id: None,
            tenant_id,
            actor_type,
            actor_id,
            action,
            target_type,
            target_id,
            result,
            risk,
            request_id: None,
            trace_id: None,
            source_ip: None,
            before_digest: None,
            after_digest: None,
            occurred_at: clock.now(),
            details,
        })
    }

    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }

    pub fn with_source_ip(mut self, source_ip: impl Into<String>) -> Self {
        self.source_ip = Some(source_ip.into());
        self
    }

    pub fn with_digests(
        mut self,
        before_digest: impl Into<String>,
        after_digest: impl Into<String>,
    ) -> Self {
        self.before_digest = Some(before_digest.into());
        self.after_digest = Some(after_digest.into());
        self
    }

    pub fn with_id(mut self, id: AuditRecordId) -> Self {
        self.id = Some(id);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use foundation::FakeClock;

    fn tenant() -> TenantId {
        TenantId::parse_str("018e1234-5678-7abc-8def-0123456789ab")
            .unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn valid_record_is_created() {
        let clock = FakeClock::from_millis(1_000_000_000_000);
        let details = AuditDetails::new("user.update", "{\"field\":\"name\"}")
            .unwrap_or_else(|e| panic!("{e}"));
        let record = AuditRecord::new(
            tenant(),
            "user",
            "018e0000-0000-0000-0000-000000000001",
            "tenant.user.update",
            "user",
            "018e0000-0000-0000-0000-000000000001",
            AuditResult::Success,
            ActionRisk::High,
            details,
            &clock,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(record.result, AuditResult::Success);
        assert!(record.risk.must_not_be_silent());
    }

    #[test]
    fn details_must_be_json_object() {
        assert!(AuditDetails::new("x", "\"string\"").is_err());
        assert!(AuditDetails::new("x", "{}").is_ok());
    }

    #[test]
    fn oversized_details_is_rejected() {
        let big = "{\"x\":\"".to_string() + &"a".repeat(65536) + "\"}";
        assert!(AuditDetails::new("x", big).is_err());
    }
}
