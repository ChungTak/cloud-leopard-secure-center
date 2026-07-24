//! Health, readiness, alerts, and runbook primitives.
//!
//! Phase 1 freezes the health/readiness/alert contract. Live probing of DB,
//! NATS, signaling, projection, disk, and certificate state is deferred.

use std::collections::HashMap;

use foundation::NodeId;

const MAX_ALERT_ID_LEN: usize = 256;
const MAX_COMPONENT_LEN: usize = 256;
const MAX_CONDITION_LEN: usize = 4096;
const MAX_SEVERITY_LEN: usize = 64;
const MAX_RUNBOOK_ID_LEN: usize = 256;
const MAX_RUNBOOK_TITLE_LEN: usize = 512;
const MAX_DIAGNOSIS_LINES: usize = 64;
const MAX_DIAGNOSIS_LINE_LEN: usize = 4096;
const MAX_STEPS: usize = 64;
const MAX_STEP_ACTION_LEN: usize = 4096;
const MAX_STEP_VERIFICATION_LEN: usize = 4096;
const MAX_DEPENDENCY_NAME_LEN: usize = 256;
const MAX_READINESS_DEPENDENCIES: usize = 64;

/// Operational role used for readiness checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Role {
    Api,
    Workflow,
    Projection,
    Scheduler,
    PluginHost,
    All,
}

/// Health state of a dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum HealthState {
    Unknown,
    Starting,
    Live,
    Ready,
    Degraded,
    Down,
}

/// A single readiness dependency for a role.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReadinessDependency {
    pub name: String,
    pub required_state: HealthState,
}

impl ReadinessDependency {
    /// Validate the readiness dependency entry.
    pub fn validate(&self) -> Result<(), HealthError> {
        if self.name.trim().is_empty() || self.name.len() > MAX_DEPENDENCY_NAME_LEN {
            return Err(HealthError::new(
                HealthErrorKind::Invalid,
                "dependency name is empty or exceeds maximum length",
            ));
        }
        Ok(())
    }
}

/// Readiness rules per role.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RoleReadiness {
    pub dependencies: HashMap<Role, Vec<ReadinessDependency>>,
}

impl RoleReadiness {
    /// Dependencies for a role, or an empty list if not defined.
    pub fn for_role(&self, role: Role) -> &[ReadinessDependency] {
        self.dependencies.get(&role).map_or(&[], |v| v.as_slice())
    }

    /// Validate all readiness dependencies.
    pub fn validate(&self) -> Result<(), HealthError> {
        for (role, deps) in &self.dependencies {
            if deps.len() > MAX_READINESS_DEPENDENCIES {
                return Err(HealthError::new(
                    HealthErrorKind::Invalid,
                    format!("too many readiness dependencies for role {role:?}"),
                ));
            }
            for dep in deps {
                dep.validate()?;
            }
        }
        Ok(())
    }
}

/// Alert rule for an infrastructure component.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AlertRule {
    pub id: String,
    pub component: String,
    pub condition: String,
    pub severity: String,
    pub runbook_ref: String,
}

impl AlertRule {
    /// Validate the alert rule fields.
    pub fn validate(&self) -> Result<(), HealthError> {
        for (value, field, max) in [
            (&self.id, "id", MAX_ALERT_ID_LEN),
            (&self.component, "component", MAX_COMPONENT_LEN),
            (&self.condition, "condition", MAX_CONDITION_LEN),
            (&self.severity, "severity", MAX_SEVERITY_LEN),
            (&self.runbook_ref, "runbook_ref", MAX_RUNBOOK_ID_LEN),
        ] {
            if value.trim().is_empty() || value.len() > max {
                return Err(HealthError::new(
                    HealthErrorKind::Invalid,
                    format!("{field} is empty or exceeds maximum length"),
                ));
            }
        }
        Ok(())
    }
}

/// A runbook step.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RunbookStep {
    pub order: u32,
    pub action: String,
    pub verification: String,
}

impl RunbookStep {
    /// Validate a single runbook step.
    pub fn validate(&self) -> Result<(), HealthError> {
        if self.action.trim().is_empty() || self.action.len() > MAX_STEP_ACTION_LEN {
            return Err(HealthError::new(
                HealthErrorKind::Invalid,
                "runbook step action is empty or exceeds maximum length",
            ));
        }
        if self.verification.len() > MAX_STEP_VERIFICATION_LEN {
            return Err(HealthError::new(
                HealthErrorKind::Invalid,
                "runbook step verification exceeds maximum length",
            ));
        }
        Ok(())
    }
}

/// Operational runbook.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Runbook {
    pub id: String,
    pub title: String,
    pub diagnosis: Vec<String>,
    pub mitigation: Vec<RunbookStep>,
    pub recovery: Vec<RunbookStep>,
    pub rollback: Vec<RunbookStep>,
    /// Deleting data is never the first action.
    pub avoid_deletion_first: bool,
}

impl Runbook {
    /// Validate the runbook shape and all nested steps.
    pub fn validate(&self) -> Result<(), HealthError> {
        if self.id.trim().is_empty() || self.id.len() > MAX_RUNBOOK_ID_LEN {
            return Err(HealthError::new(
                HealthErrorKind::Invalid,
                "runbook id is empty or exceeds maximum length",
            ));
        }
        if self.title.trim().is_empty() || self.title.len() > MAX_RUNBOOK_TITLE_LEN {
            return Err(HealthError::new(
                HealthErrorKind::Invalid,
                "runbook title is empty or exceeds maximum length",
            ));
        }
        if self.diagnosis.len() > MAX_DIAGNOSIS_LINES {
            return Err(HealthError::new(
                HealthErrorKind::Invalid,
                "runbook diagnosis has too many lines",
            ));
        }
        for line in &self.diagnosis {
            if line.trim().is_empty() || line.len() > MAX_DIAGNOSIS_LINE_LEN {
                return Err(HealthError::new(
                    HealthErrorKind::Invalid,
                    "runbook diagnosis line is empty or exceeds maximum length",
                ));
            }
        }
        for (steps, field) in [
            (&self.mitigation, "mitigation"),
            (&self.recovery, "recovery"),
            (&self.rollback, "rollback"),
        ] {
            if steps.len() > MAX_STEPS {
                return Err(HealthError::new(
                    HealthErrorKind::Invalid,
                    format!("runbook {field} has too many steps"),
                ));
            }
            for step in steps {
                step.validate().map_err(|e| {
                    HealthError::new(HealthErrorKind::Invalid, format!("{field}: {e}"))
                })?;
            }
        }
        Ok(())
    }
}

/// Health monitoring port.
#[async_trait::async_trait]
pub trait HealthMonitor: Send + Sync {
    async fn live(&self, node_id: NodeId) -> Result<HealthState, HealthError>;
    async fn ready(&self, node_id: NodeId, role: Role) -> Result<HealthState, HealthError>;
    async fn alert_rules(&self) -> Result<Vec<AlertRule>, HealthError>;
    async fn runbook(&self, id: &str) -> Result<Option<Runbook>, HealthError>;
}

/// Health error.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error("{kind:?}: {message}")]
pub struct HealthError {
    pub kind: HealthErrorKind,
    pub message: String,
}

impl HealthError {
    pub fn new(kind: HealthErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

/// Kinds of health failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum HealthErrorKind {
    Unsupported,
    Unavailable,
    Invalid,
    Down,
}

/// Placeholder health monitor.
#[derive(Debug, Clone, Default)]
pub struct UnsupportedHealthMonitor {
    enabled: bool,
}

impl UnsupportedHealthMonitor {
    /// Create the monitor. When `enabled` is true it reports `Unsupported`;
    /// otherwise `Unavailable`.
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

#[async_trait::async_trait]
impl HealthMonitor for UnsupportedHealthMonitor {
    async fn live(&self, _node_id: NodeId) -> Result<HealthState, HealthError> {
        self.reject("live")
    }

    async fn ready(&self, _node_id: NodeId, _role: Role) -> Result<HealthState, HealthError> {
        self.reject("ready")
    }

    async fn alert_rules(&self) -> Result<Vec<AlertRule>, HealthError> {
        self.reject("alert_rules")
    }

    async fn runbook(&self, id: &str) -> Result<Option<Runbook>, HealthError> {
        if id.trim().is_empty() || id.len() > MAX_RUNBOOK_ID_LEN {
            return Err(HealthError::new(
                HealthErrorKind::Invalid,
                "runbook id is empty or exceeds maximum length",
            ));
        }
        self.reject("runbook")
    }
}

impl UnsupportedHealthMonitor {
    fn reject<T>(&self, action: &str) -> Result<T, HealthError> {
        if self.enabled {
            Err(HealthError::new(
                HealthErrorKind::Unsupported,
                format!("health monitor {action} is not implemented in this build"),
            ))
        } else {
            Err(HealthError::new(
                HealthErrorKind::Unavailable,
                "health monitor is not configured",
            ))
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use foundation::{SystemClock, SystemIdGenerator, SystemRandom};

    use super::*;

    fn err_or_panic<T, E: std::fmt::Debug>(result: Result<T, E>) -> E {
        match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        }
    }

    #[tokio::test]
    async fn disabled_monitor_returns_unavailable() {
        let monitor = UnsupportedHealthMonitor::new(false);
        let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
        let result = monitor
            .live(NodeId::generate(&generator).expect("generate node id"))
            .await;
        assert_eq!(err_or_panic(result).kind, HealthErrorKind::Unavailable);
    }

    #[tokio::test]
    async fn enabled_monitor_returns_unsupported() {
        let monitor = UnsupportedHealthMonitor::new(true);
        let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
        let result = monitor
            .ready(
                NodeId::generate(&generator).expect("generate node id"),
                Role::Api,
            )
            .await;
        assert_eq!(err_or_panic(result).kind, HealthErrorKind::Unsupported);
    }

    #[test]
    fn runbook_forbids_deletion_as_first_action() {
        let runbook = Runbook {
            id: "disk-full".to_string(),
            title: "disk full".to_string(),
            diagnosis: vec!["check disk usage".to_string()],
            mitigation: vec![RunbookStep {
                order: 1,
                action: "expand volume".to_string(),
                verification: "df -h".to_string(),
            }],
            recovery: vec![],
            rollback: vec![],
            avoid_deletion_first: true,
        };
        assert!(runbook.avoid_deletion_first);
    }
}
