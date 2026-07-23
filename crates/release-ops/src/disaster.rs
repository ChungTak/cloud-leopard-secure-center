//! Disaster recovery primitives.
//!
//! Phase 1 freezes the recovery plan shape and engine port. Real PostgreSQL
//! PITR, object-store restore, configuration/secret metadata replay, NATS
//! non-authoritative replay, projection rebuild, and job/outbox reconciliation
//! are deferred.

/// Scope of a recovery operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RecoveryTarget {
    SingleNode,
    Zone,
    FullSite,
}

/// Recovery step.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RecoveryStep {
    pub order: u32,
    pub action: String,
    pub verification: String,
    pub side_effect_safe: bool,
}

/// Disaster recovery plan.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DisasterRecoveryPlan {
    pub target: RecoveryTarget,
    pub rpo_seconds: u32,
    pub rto_seconds: u32,
    pub steps: Vec<RecoveryStep>,
    pub pitr_timestamp: Option<String>,
    pub object_store_path: String,
    pub config_metadata_path: String,
    pub nats_replay_order: Vec<String>,
    pub expected_data_digest: String,
}

impl DisasterRecoveryPlan {
    /// Validate the recovery plan ordering and safety invariants.
    pub fn validate(&self) -> Result<(), RecoveryError> {
        if self.steps.is_empty() {
            return Err(RecoveryError::new(
                RecoveryErrorKind::Invalid,
                "recovery plan has no steps",
            ));
        }
        for step in &self.steps {
            if !step.side_effect_safe {
                return Err(RecoveryError::new(
                    RecoveryErrorKind::Invalid,
                    format!(
                        "recovery step '{}' is not marked side-effect safe",
                        step.action
                    ),
                ));
            }
        }
        if self.rto_seconds == 0 || self.rpo_seconds == 0 {
            return Err(RecoveryError::new(
                RecoveryErrorKind::Invalid,
                "RTO and RPO must be greater than zero",
            ));
        }
        Ok(())
    }
}

/// Recovery engine port.
#[async_trait::async_trait]
pub trait RecoveryEngine: Send + Sync {
    async fn recover(&self, plan: &DisasterRecoveryPlan) -> Result<RecoveryReport, RecoveryError>;
}

/// Result of a disaster recovery run.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RecoveryReport {
    pub target: RecoveryTarget,
    pub completed_steps: Vec<String>,
    pub projection_rebuilt: bool,
    pub pending_jobs_reconciled: bool,
    pub outbox_reconciled: bool,
    pub actual_data_digest: String,
    pub rpo_seconds: u32,
    pub rto_seconds: u32,
}

/// Recovery error.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error("{kind:?}: {message}")]
pub struct RecoveryError {
    pub kind: RecoveryErrorKind,
    pub message: String,
}

impl RecoveryError {
    pub fn new(kind: RecoveryErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

/// Kinds of recovery failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecoveryErrorKind {
    Invalid,
    Unsupported,
    Unavailable,
    DigestMismatch,
    SideEffectDetected,
    Timeout,
}

/// Placeholder recovery engine.
#[derive(Debug, Clone, Default)]
pub struct UnsupportedRecoveryEngine {
    enabled: bool,
}

impl UnsupportedRecoveryEngine {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

#[async_trait::async_trait]
impl RecoveryEngine for UnsupportedRecoveryEngine {
    async fn recover(&self, _plan: &DisasterRecoveryPlan) -> Result<RecoveryReport, RecoveryError> {
        if self.enabled {
            Err(RecoveryError::new(
                RecoveryErrorKind::Unsupported,
                "disaster recovery is not implemented in this build",
            ))
        } else {
            Err(RecoveryError::new(
                RecoveryErrorKind::Unavailable,
                "recovery engine is not configured",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err_or_panic<T, E: std::fmt::Debug>(result: Result<T, E>) -> E {
        match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        }
    }

    fn ok_or_panic<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
        match result {
            Ok(v) => v,
            Err(e) => panic!("{e}"),
        }
    }

    fn sample_plan() -> DisasterRecoveryPlan {
        DisasterRecoveryPlan {
            target: RecoveryTarget::Zone,
            rpo_seconds: 60,
            rto_seconds: 300,
            steps: vec![
                RecoveryStep {
                    order: 1,
                    action: "restore postgres".to_string(),
                    verification: "SELECT 1".to_string(),
                    side_effect_safe: true,
                },
                RecoveryStep {
                    order: 2,
                    action: "replay config metadata".to_string(),
                    verification: "config digest match".to_string(),
                    side_effect_safe: true,
                },
                RecoveryStep {
                    order: 3,
                    action: "rebuild projections".to_string(),
                    verification: "projection view ready".to_string(),
                    side_effect_safe: true,
                },
            ],
            pitr_timestamp: None,
            object_store_path: "s3://backup/objects".to_string(),
            config_metadata_path: "s3://backup/config".to_string(),
            nats_replay_order: vec!["events".to_string()],
            expected_data_digest: "sha256:final".to_string(),
        }
    }

    #[test]
    fn valid_plan_passes() {
        ok_or_panic(sample_plan().validate());
    }

    #[test]
    fn unsafe_step_fails() {
        let mut plan = sample_plan();
        plan.steps[0].side_effect_safe = false;
        let err = err_or_panic(plan.validate());
        assert_eq!(err.kind, RecoveryErrorKind::Invalid);
    }

    #[test]
    fn zero_rto_fails() {
        let mut plan = sample_plan();
        plan.rto_seconds = 0;
        let err = err_or_panic(plan.validate());
        assert_eq!(err.kind, RecoveryErrorKind::Invalid);
    }

    #[tokio::test]
    async fn disabled_engine_returns_unavailable() {
        let engine = UnsupportedRecoveryEngine::new(false);
        let err = err_or_panic(engine.recover(&sample_plan()).await);
        assert_eq!(err.kind, RecoveryErrorKind::Unavailable);
    }

    #[tokio::test]
    async fn enabled_engine_returns_unsupported() {
        let engine = UnsupportedRecoveryEngine::new(true);
        let err = err_or_panic(engine.recover(&sample_plan()).await);
        assert_eq!(err.kind, RecoveryErrorKind::Unsupported);
    }
}
