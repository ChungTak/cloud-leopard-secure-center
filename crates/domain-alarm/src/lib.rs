//! Alarm domain: lifecycle, severity, evidence, and repository port.
//!
//! Phase 1 freezes the aggregate shapes and repository port. Live persistence,
//! notification delivery, and linkage workflow are in follow-up tasks.

use foundation::{AlarmId, MessageId, TenantId, UtcTimestamp};

pub mod linkage;
pub mod notification;

const MAX_TITLE_LEN: usize = 256;
const MAX_DEDUP_VALUE_LEN: usize = 256;
const MAX_EVIDENCE_OBJECT_KEY_LEN: usize = 1024;
const MAX_EVIDENCE_ALGORITHM_LEN: usize = 64;
const MAX_EVIDENCE_CHECKSUM_LEN: usize = 1024;
const MAX_EVIDENCE_REFS: usize = 64;
const MAX_ACTION_STRING_LEN: usize = 1024;
const MAX_ASSIGNED_TO_LEN: usize = 256;

/// Alarm severity with explicit upper bounds per tenant policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

/// Alarm lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum AlarmState {
    New,
    Acknowledged,
    Processing,
    Resolved,
    Closed,
    Suppressed,
    Merged,
    Reopened,
}

/// Action taken on an alarm, recorded as evidence.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AlarmAction {
    Acknowledge { by: String, note: Option<String> },
    Assign { to: String },
    Resolve { reason: String },
    Close,
    Reopen { reason: String },
    Suppress { until: Option<UtcTimestamp> },
    Merge { target: AlarmId },
}

/// External evidence reference with checksum.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EvidenceRef {
    pub object_key: String,
    pub algorithm: String,
    pub checksum: String,
}

/// A deduplication key plus aggregation window.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DedupKey {
    pub value: String,
    pub window_seconds: u32,
}

/// Immutable alarm event that feeds the aggregate.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AlarmEvent {
    pub id: MessageId,
    pub tenant_id: TenantId,
    pub alarm_id: AlarmId,
    pub dedup: Option<DedupKey>,
    pub severity: Severity,
    pub title: String,
    pub payload: serde_json::Value,
    pub evidence: Vec<EvidenceRef>,
    pub occurred_at: UtcTimestamp,
}

/// Alarm aggregate.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Alarm {
    pub id: AlarmId,
    pub tenant_id: TenantId,
    pub state: AlarmState,
    pub severity: Severity,
    pub title: String,
    pub payload: serde_json::Value,
    pub dedup: Option<DedupKey>,
    pub evidence: Vec<EvidenceRef>,
    pub assigned_to: Option<String>,
    pub revision: u64,
    pub created_at: UtcTimestamp,
    pub updated_at: UtcTimestamp,
}

impl Alarm {
    /// Create a new alarm in the `New` state.
    pub fn new(
        id: AlarmId,
        tenant_id: TenantId,
        event: &AlarmEvent,
        now: UtcTimestamp,
    ) -> Result<Self, AlarmError> {
        if event.severity == Severity::Info && event.dedup.is_none() {
            return Err(AlarmError::new(
                AlarmErrorKind::Invalid,
                "info alarms require a dedup key",
            ));
        }
        validate_alarm_string(&event.title, "title", MAX_TITLE_LEN)?;
        if let Some(dedup) = &event.dedup {
            validate_alarm_string(&dedup.value, "dedup.value", MAX_DEDUP_VALUE_LEN)?;
        }
        if event.evidence.len() > MAX_EVIDENCE_REFS {
            return Err(AlarmError::new(
                AlarmErrorKind::Invalid,
                "too many evidence references",
            ));
        }
        for (i, evidence) in event.evidence.iter().enumerate() {
            validate_alarm_string(
                &evidence.object_key,
                "evidence.object_key",
                MAX_EVIDENCE_OBJECT_KEY_LEN,
            )?;
            validate_alarm_string(
                &evidence.algorithm,
                "evidence.algorithm",
                MAX_EVIDENCE_ALGORITHM_LEN,
            )?;
            validate_alarm_string(
                &evidence.checksum,
                "evidence.checksum",
                MAX_EVIDENCE_CHECKSUM_LEN,
            )?;
            if evidence.object_key.trim().is_empty() {
                return Err(AlarmError::new(
                    AlarmErrorKind::Invalid,
                    format!("evidence[{i}] object_key is empty"),
                ));
            }
        }
        Ok(Self {
            id,
            tenant_id,
            state: AlarmState::New,
            severity: event.severity,
            title: event.title.clone(),
            payload: event.payload.clone(),
            dedup: event.dedup.clone(),
            evidence: event.evidence.clone(),
            assigned_to: None,
            revision: 1,
            created_at: now,
            updated_at: now,
        })
    }

    /// Apply an action, validating the state transition.
    pub fn apply(&mut self, action: AlarmAction, now: UtcTimestamp) -> Result<(), AlarmError> {
        let next = match (self.state, action) {
            (AlarmState::New, AlarmAction::Acknowledge { by, note }) => {
                validate_alarm_string(&by, "by", MAX_ASSIGNED_TO_LEN)?;
                if let Some(note) = &note {
                    validate_alarm_string(note, "note", MAX_ACTION_STRING_LEN)?;
                }
                AlarmState::Acknowledged
            }
            (AlarmState::New, AlarmAction::Assign { to }) => {
                validate_alarm_string(&to, "to", MAX_ASSIGNED_TO_LEN)?;
                self.assigned_to = Some(to);
                AlarmState::Acknowledged
            }
            (AlarmState::Acknowledged, AlarmAction::Assign { to }) => {
                validate_alarm_string(&to, "to", MAX_ASSIGNED_TO_LEN)?;
                self.assigned_to = Some(to);
                self.state
            }
            (AlarmState::Acknowledged, AlarmAction::Resolve { reason }) => {
                validate_alarm_string(&reason, "reason", MAX_ACTION_STRING_LEN)?;
                AlarmState::Resolved
            }
            (AlarmState::Processing, AlarmAction::Resolve { reason }) => {
                validate_alarm_string(&reason, "reason", MAX_ACTION_STRING_LEN)?;
                AlarmState::Resolved
            }
            (AlarmState::Resolved, AlarmAction::Close) => AlarmState::Closed,
            (AlarmState::Resolved, AlarmAction::Reopen { reason }) => {
                validate_alarm_string(&reason, "reason", MAX_ACTION_STRING_LEN)?;
                AlarmState::Reopened
            }
            (AlarmState::Closed, AlarmAction::Reopen { reason }) => {
                validate_alarm_string(&reason, "reason", MAX_ACTION_STRING_LEN)?;
                AlarmState::Reopened
            }
            (_, AlarmAction::Suppress { .. }) => AlarmState::Suppressed,
            (_, AlarmAction::Merge { .. }) => AlarmState::Merged,
            _ => {
                return Err(AlarmError::new(
                    AlarmErrorKind::Invalid,
                    "illegal alarm state transition",
                ));
            }
        };
        self.state = next;
        self.revision += 1;
        self.updated_at = now;
        Ok(())
    }
}

fn validate_alarm_string(value: &str, field: &str, max: usize) -> Result<(), AlarmError> {
    if value.trim().is_empty() || value.len() > max {
        return Err(AlarmError::new(
            AlarmErrorKind::Invalid,
            format!("{field} is empty or exceeds maximum length"),
        ));
    }
    Ok(())
}

/// Alarm domain error.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error("{kind:?}: {message}")]
pub struct AlarmError {
    pub kind: AlarmErrorKind,
    pub message: String,
}

impl AlarmError {
    pub fn new(kind: AlarmErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

/// Kinds of alarm domain failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AlarmErrorKind {
    Invalid,
    NotFound,
    Duplicate,
    Concurrent,
    Unauthorized,
    Unsupported,
    Unavailable,
}

/// Port for alarm persistence.
#[async_trait::async_trait]
pub trait AlarmRepository: Send + Sync {
    async fn save(&self, alarm: &Alarm) -> Result<(), AlarmError>;
    async fn by_id(&self, tenant_id: TenantId, id: AlarmId) -> Result<Option<Alarm>, AlarmError>;
    async fn by_dedup_key(
        &self,
        tenant_id: TenantId,
        key: &str,
    ) -> Result<Option<Alarm>, AlarmError>;
}

/// Placeholder repository for builds without a persistence backend.
#[derive(Debug, Clone, Default)]
pub struct UnsupportedAlarmRepository;

#[async_trait::async_trait]
impl AlarmRepository for UnsupportedAlarmRepository {
    async fn save(&self, _alarm: &Alarm) -> Result<(), AlarmError> {
        Err(AlarmError::new(
            AlarmErrorKind::Unsupported,
            "alarm repository is not enabled in this build",
        ))
    }

    async fn by_id(&self, _tenant_id: TenantId, _id: AlarmId) -> Result<Option<Alarm>, AlarmError> {
        Err(AlarmError::new(
            AlarmErrorKind::Unsupported,
            "alarm repository is not enabled in this build",
        ))
    }

    async fn by_dedup_key(
        &self,
        _tenant_id: TenantId,
        _key: &str,
    ) -> Result<Option<Alarm>, AlarmError> {
        Err(AlarmError::new(
            AlarmErrorKind::Unsupported,
            "alarm repository is not enabled in this build",
        ))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use foundation::{DeviceId, SystemClock, SystemIdGenerator, SystemRandom};

    use super::*;

    fn ok_or_panic<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
        match result {
            Ok(v) => v,
            Err(e) => panic!("{e}"),
        }
    }

    fn err_or_panic<T, E: std::fmt::Debug>(result: Result<T, E>) -> E {
        match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        }
    }

    fn make_event() -> AlarmEvent {
        let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
        AlarmEvent {
            id: MessageId::generate(&generator).expect("generate message id"),
            tenant_id: TenantId::generate(&generator).expect("generate tenant id"),
            alarm_id: AlarmId::generate(&generator).expect("generate alarm id"),
            dedup: Some(DedupKey {
                value: "cam-1.motion".to_string(),
                window_seconds: 60,
            }),
            severity: Severity::High,
            title: "motion detected".to_string(),
            payload: serde_json::json!({"device_id": DeviceId::generate(&generator)
                .expect("generate device id")
                .to_string()}),
            evidence: vec![EvidenceRef {
                object_key: "evidence/1.jpg".to_string(),
                algorithm: "sha256".to_string(),
                checksum: "abc".to_string(),
            }],
            occurred_at: UtcTimestamp::now(),
        }
    }

    #[test]
    fn alarm_starts_in_new_state() {
        let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
        let event = make_event();
        let alarm = ok_or_panic(Alarm::new(
            AlarmId::generate(&generator).expect("generate alarm id"),
            TenantId::generate(&generator).expect("generate tenant id"),
            &event,
            UtcTimestamp::now(),
        ));
        assert_eq!(alarm.state, AlarmState::New);
        assert_eq!(alarm.severity, Severity::High);
    }

    #[test]
    fn acknowledge_moves_to_acknowledged() {
        let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
        let mut alarm = ok_or_panic(Alarm::new(
            AlarmId::generate(&generator).expect("generate alarm id"),
            TenantId::generate(&generator).expect("generate tenant id"),
            &make_event(),
            UtcTimestamp::now(),
        ));
        ok_or_panic(alarm.apply(
            AlarmAction::Acknowledge {
                by: "operator".to_string(),
                note: None,
            },
            UtcTimestamp::now(),
        ));
        assert_eq!(alarm.state, AlarmState::Acknowledged);
    }

    #[test]
    fn close_from_new_is_illegal() {
        let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
        let mut alarm = ok_or_panic(Alarm::new(
            AlarmId::generate(&generator).expect("generate alarm id"),
            TenantId::generate(&generator).expect("generate tenant id"),
            &make_event(),
            UtcTimestamp::now(),
        ));
        let result = err_or_panic(alarm.apply(AlarmAction::Close, UtcTimestamp::now()));
        assert_eq!(result.kind, AlarmErrorKind::Invalid);
    }

    #[test]
    fn unsupported_repository_returns_unsupported() {
        let mut runtime = futures::executor::LocalPool::new();
        let repo = UnsupportedAlarmRepository;
        let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
        let alarm = ok_or_panic(Alarm::new(
            AlarmId::generate(&generator).expect("generate alarm id"),
            TenantId::generate(&generator).expect("generate tenant id"),
            &make_event(),
            UtcTimestamp::now(),
        ));
        match runtime.run_until(repo.save(&alarm)) {
            Ok(_) => panic!("expected unsupported"),
            Err(e) => assert_eq!(e.kind, AlarmErrorKind::Unsupported),
        }
    }
}
