//! Audit record aggregate.

use foundation::{Clock, PlatformError, TenantId, UtcTimestamp};

const MAX_AUDIT_FIELD_LEN: usize = 256;
const MAX_SOURCE_IP_LEN: usize = 64;
const MAX_DIGEST_LEN: usize = 1024;

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
    pub fn new(schema: impl AsRef<str>, value: impl AsRef<str>) -> Result<Self, PlatformError> {
        let schema = schema.as_ref();
        let value = value.as_ref();
        if schema.trim().is_empty() {
            return Err(PlatformError::invalid(
                "details_schema",
                "details schema must not be empty",
            ));
        }
        if schema.len() > MAX_AUDIT_FIELD_LEN {
            return Err(PlatformError::invalid(
                "details_schema",
                "details schema exceeds maximum length",
            ));
        }
        if value.len() > Self::MAX_SIZE {
            return Err(PlatformError::invalid(
                "details",
                format!("details must not exceed {} bytes", Self::MAX_SIZE),
            ));
        }
        match serde_json::from_str::<serde_json::Value>(value) {
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
        Ok(Self {
            schema: schema.to_string(),
            value: value.to_string(),
        })
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
        actor_type: impl AsRef<str>,
        actor_id: impl AsRef<str>,
        action: impl AsRef<str>,
        target_type: impl AsRef<str>,
        target_id: impl AsRef<str>,
        result: AuditResult,
        risk: ActionRisk,
        details: AuditDetails,
        clock: &dyn Clock,
    ) -> Result<Self, PlatformError> {
        let actor_type = actor_type.as_ref();
        let actor_id = actor_id.as_ref();
        let action = action.as_ref();
        let target_type = target_type.as_ref();
        let target_id = target_id.as_ref();
        validate_audit_field(action, "action")?;
        validate_audit_field(actor_type, "actor_type")?;
        validate_audit_field(actor_id, "actor_id")?;
        validate_audit_field(target_type, "target_type")?;
        validate_audit_field(target_id, "target_id")?;
        Ok(Self {
            id: None,
            tenant_id,
            actor_type: actor_type.to_string(),
            actor_id: actor_id.to_string(),
            action: action.to_string(),
            target_type: target_type.to_string(),
            target_id: target_id.to_string(),
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

    pub fn with_request_id(mut self, request_id: impl AsRef<str>) -> Result<Self, PlatformError> {
        let request_id = request_id.as_ref();
        validate_audit_field(request_id, "request_id")?;
        self.request_id = Some(request_id.to_string());
        Ok(self)
    }

    pub fn with_trace_id(mut self, trace_id: impl AsRef<str>) -> Result<Self, PlatformError> {
        let trace_id = trace_id.as_ref();
        validate_audit_field(trace_id, "trace_id")?;
        self.trace_id = Some(trace_id.to_string());
        Ok(self)
    }

    pub fn with_source_ip(mut self, source_ip: impl AsRef<str>) -> Result<Self, PlatformError> {
        let source_ip = source_ip.as_ref();
        if source_ip.trim().is_empty() || source_ip.len() > MAX_SOURCE_IP_LEN {
            return Err(PlatformError::invalid(
                "source_ip",
                "source ip is empty or too long",
            ));
        }
        self.source_ip = Some(source_ip.to_string());
        Ok(self)
    }

    pub fn with_digests(
        mut self,
        before_digest: impl AsRef<str>,
        after_digest: impl AsRef<str>,
    ) -> Result<Self, PlatformError> {
        let before_digest = before_digest.as_ref();
        let after_digest = after_digest.as_ref();
        if before_digest.trim().is_empty() || before_digest.len() > MAX_DIGEST_LEN {
            return Err(PlatformError::invalid(
                "before_digest",
                "before digest is empty or too long",
            ));
        }
        if after_digest.trim().is_empty() || after_digest.len() > MAX_DIGEST_LEN {
            return Err(PlatformError::invalid(
                "after_digest",
                "after digest is empty or too long",
            ));
        }
        self.before_digest = Some(before_digest.to_string());
        self.after_digest = Some(after_digest.to_string());
        Ok(self)
    }

    pub fn with_id(mut self, id: AuditRecordId) -> Self {
        self.id = Some(id);
        self
    }
}

fn validate_audit_field(value: &str, field: &str) -> Result<(), PlatformError> {
    if value.trim().is_empty() || value.len() > MAX_AUDIT_FIELD_LEN {
        return Err(PlatformError::invalid(
            field,
            format!("{field} is empty or exceeds maximum length"),
        ));
    }
    Ok(())
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
