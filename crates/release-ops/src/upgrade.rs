//! Rolling upgrade and rollback primitives.
//!
//! Phase 1 freezes the `expand -> backfill -> switch -> contract` plan shape.
//! Real orchestration, dual-write NATS migrations, and binary coexistence tests
//! are deferred.

/// Kind of upgrade step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum UpgradeStepKind {
    Expand,
    Backfill,
    Switch,
    Contract,
    HealthCheck,
}

/// A single upgrade step.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct UpgradeStep {
    pub kind: UpgradeStepKind,
    pub target: String,
    pub pre_condition: String,
    pub post_verification: String,
    pub can_rollback_before: bool,
}

/// Upgrade plan for a new release.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct UpgradePlan {
    pub from_version: String,
    pub to_version: String,
    pub steps: Vec<UpgradeStep>,
    pub max_parallel_nodes: u32,
    pub rollback_deadline_seconds: u32,
}

impl UpgradePlan {
    /// Validate that the plan follows the expand->backfill->switch->contract order.
    pub fn validate(&self) -> Result<(), UpgradeError> {
        let mut saw_expand = false;
        let mut saw_backfill = false;
        let mut saw_switch = false;
        let mut saw_contract = false;
        for step in &self.steps {
            match step.kind {
                UpgradeStepKind::Expand => {
                    if saw_expand || saw_backfill || saw_switch || saw_contract {
                        return Err(UpgradeError::new(
                            UpgradeErrorKind::Invalid,
                            "expand must come first",
                        ));
                    }
                    saw_expand = true;
                }
                UpgradeStepKind::Backfill => {
                    if !saw_expand || saw_switch || saw_contract {
                        return Err(UpgradeError::new(
                            UpgradeErrorKind::Invalid,
                            "backfill must follow expand and precede switch/contract",
                        ));
                    }
                    saw_backfill = true;
                }
                UpgradeStepKind::Switch => {
                    if !saw_expand || !saw_backfill || saw_contract {
                        return Err(UpgradeError::new(
                            UpgradeErrorKind::Invalid,
                            "switch must follow expand+backfill and precede contract",
                        ));
                    }
                    saw_switch = true;
                }
                UpgradeStepKind::Contract => {
                    if !saw_expand || !saw_backfill || !saw_switch {
                        return Err(UpgradeError::new(
                            UpgradeErrorKind::Invalid,
                            "contract must follow expand+backfill+switch",
                        ));
                    }
                    saw_contract = true;
                }
                UpgradeStepKind::HealthCheck => {}
            }
        }
        if !saw_expand || !saw_backfill || !saw_switch || !saw_contract {
            return Err(UpgradeError::new(
                UpgradeErrorKind::Invalid,
                "upgrade plan is missing required phases",
            ));
        }
        Ok(())
    }
}

/// Rollback step.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RollbackStep {
    pub reverse_of: UpgradeStepKind,
    pub action: String,
    pub verification: String,
}

/// Rollback plan derived from an upgrade plan.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RollbackPlan {
    pub target_version: String,
    pub steps: Vec<RollbackStep>,
}

/// Upgrade error.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error("{kind:?}: {message}")]
pub struct UpgradeError {
    pub kind: UpgradeErrorKind,
    pub message: String,
}

impl UpgradeError {
    pub fn new(kind: UpgradeErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

/// Kinds of upgrade failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum UpgradeErrorKind {
    Invalid,
    Unsupported,
    Unavailable,
    HealthCheckFailed,
    RollbackRequired,
}

/// Port for executing upgrades and rollbacks.
#[async_trait::async_trait]
pub trait UpgradeEngine: Send + Sync {
    async fn execute(&self, plan: &UpgradePlan) -> Result<(), UpgradeError>;
    async fn rollback(&self, plan: &RollbackPlan) -> Result<(), UpgradeError>;
}

/// Placeholder upgrade engine.
#[derive(Debug, Clone, Default)]
pub struct UnsupportedUpgradeEngine {
    enabled: bool,
}

impl UnsupportedUpgradeEngine {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

#[async_trait::async_trait]
impl UpgradeEngine for UnsupportedUpgradeEngine {
    async fn execute(&self, _plan: &UpgradePlan) -> Result<(), UpgradeError> {
        if self.enabled {
            Err(UpgradeError::new(
                UpgradeErrorKind::Unsupported,
                "upgrade execution is not implemented in this build",
            ))
        } else {
            Err(UpgradeError::new(
                UpgradeErrorKind::Unavailable,
                "upgrade engine is not configured",
            ))
        }
    }

    async fn rollback(&self, _plan: &RollbackPlan) -> Result<(), UpgradeError> {
        if self.enabled {
            Err(UpgradeError::new(
                UpgradeErrorKind::Unsupported,
                "rollback execution is not implemented in this build",
            ))
        } else {
            Err(UpgradeError::new(
                UpgradeErrorKind::Unavailable,
                "rollback engine is not configured",
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

    fn sample_plan() -> UpgradePlan {
        UpgradePlan {
            from_version: "0.1.0".to_string(),
            to_version: "0.2.0".to_string(),
            steps: vec![
                UpgradeStep {
                    kind: UpgradeStepKind::Expand,
                    target: "nats subjects".to_string(),
                    pre_condition: "cluster healthy".to_string(),
                    post_verification: "subjects expanded".to_string(),
                    can_rollback_before: true,
                },
                UpgradeStep {
                    kind: UpgradeStepKind::Backfill,
                    target: "kv data".to_string(),
                    pre_condition: "subjects expanded".to_string(),
                    post_verification: "backfill complete".to_string(),
                    can_rollback_before: true,
                },
                UpgradeStep {
                    kind: UpgradeStepKind::Switch,
                    target: "active code".to_string(),
                    pre_condition: "backfill complete".to_string(),
                    post_verification: "traffic on new code".to_string(),
                    can_rollback_before: false,
                },
                UpgradeStep {
                    kind: UpgradeStepKind::Contract,
                    target: "old subjects".to_string(),
                    pre_condition: "switch verified".to_string(),
                    post_verification: "old subjects removed".to_string(),
                    can_rollback_before: false,
                },
            ],
            max_parallel_nodes: 1,
            rollback_deadline_seconds: 300,
        }
    }

    fn ok_or_panic<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
        match result {
            Ok(v) => v,
            Err(e) => panic!("{e}"),
        }
    }

    #[test]
    fn valid_plan_passes() {
        let plan = sample_plan();
        ok_or_panic(plan.validate());
    }

    #[test]
    fn contract_before_switch_fails() {
        let mut plan = sample_plan();
        plan.steps[2].kind = UpgradeStepKind::Contract;
        plan.steps[3].kind = UpgradeStepKind::Switch;
        let err = err_or_panic(plan.validate());
        assert_eq!(err.kind, UpgradeErrorKind::Invalid);
    }

    #[tokio::test]
    async fn disabled_engine_returns_unavailable() {
        let engine = UnsupportedUpgradeEngine::new(false);
        let err = err_or_panic(engine.execute(&sample_plan()).await);
        assert_eq!(err.kind, UpgradeErrorKind::Unavailable);
    }

    #[tokio::test]
    async fn enabled_rollback_returns_unsupported() {
        let engine = UnsupportedUpgradeEngine::new(true);
        let plan = RollbackPlan {
            target_version: "0.1.0".to_string(),
            steps: vec![],
        };
        let err = err_or_panic(engine.rollback(&plan).await);
        assert_eq!(err.kind, UpgradeErrorKind::Unsupported);
    }
}
