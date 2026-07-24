//! Alarm linkage workflow: rules, conditions, actions, and loop detection.
//!
//! Phase 1 freezes the linkage rule shape and the workflow port. Execution of
//! actions, cooling, max derivation depth, and replay without side effects are
//! deferred to a follow-up phase.

use std::collections::HashSet;

use foundation::TenantId;

use crate::{Alarm, AlarmError, AlarmErrorKind};

const MAX_RULE_ID_LEN: usize = 256;
const MAX_EXCLUSION_ID_LEN: usize = 256;
const MAX_EXCLUSIONS: usize = 256;
const MAX_ACTIONS: usize = 64;
const MAX_ACTION_STRING_LEN: usize = 1024;
const MAX_LINKAGE_CONDITION_DEPTH: usize = 32;

/// Conditions that trigger a linkage rule.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LinkageCondition {
    StateIs(crate::AlarmState),
    SeverityAtLeast(crate::Severity),
    And(Vec<LinkageCondition>),
    Or(Vec<LinkageCondition>),
}

/// Actions a linkage rule can perform.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LinkageAction {
    /// Notify a recipient via a channel.
    Notify {
        channel: String,
        recipient: String,
        template: String,
    },
    /// Call a plugin action by name.
    Plugin { plugin: String, action: String },
    /// Create a derived alarm.
    DeriveAlarm { title: String },
}

impl LinkageAction {
    /// Validate the action string fields before execution.
    pub fn validate(&self) -> Result<(), AlarmError> {
        fn check(value: &str, field: &str) -> Result<(), AlarmError> {
            if value.trim().is_empty() || value.len() > MAX_ACTION_STRING_LEN {
                return Err(AlarmError::new(
                    AlarmErrorKind::Invalid,
                    format!("{field} is empty or exceeds maximum length"),
                ));
            }
            Ok(())
        }

        match self {
            LinkageAction::Notify {
                channel,
                recipient,
                template,
            } => {
                check(channel, "notify.channel")?;
                check(recipient, "notify.recipient")?;
                check(template, "notify.template")?;
            }
            LinkageAction::Plugin { plugin, action } => {
                check(plugin, "plugin.name")?;
                check(action, "plugin.action")?;
            }
            LinkageAction::DeriveAlarm { title } => {
                check(title, "derive_alarm.title")?;
            }
        }
        Ok(())
    }
}

impl LinkageCondition {
    /// Validate the condition tree depth to prevent stack overflow from a
    /// maliciously nested upstream payload.
    pub fn validate(&self, depth: usize) -> Result<(), AlarmError> {
        if depth > MAX_LINKAGE_CONDITION_DEPTH {
            return Err(AlarmError::new(
                AlarmErrorKind::Invalid,
                "linkage condition exceeds maximum nesting depth",
            ));
        }
        match self {
            LinkageCondition::And(conditions) | LinkageCondition::Or(conditions) => {
                for condition in conditions {
                    condition.validate(depth + 1)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// Linkage rule with cooling and depth limits.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AlarmLinkageRule {
    pub tenant_id: TenantId,
    pub rule_id: String,
    pub condition: LinkageCondition,
    pub actions: Vec<LinkageAction>,
    /// Seconds before the same alarm can trigger this rule again.
    pub cooldown_seconds: u64,
    /// Maximum number of derived alarms/actions before the chain stops.
    pub max_depth: u32,
    /// IDs of rules that must not be triggered in the same chain (loop guard).
    pub exclusions: HashSet<String>,
}

impl AlarmLinkageRule {
    /// Validate the rule shape, bounds, and action strings.
    pub fn validate(&self) -> Result<(), AlarmError> {
        if self.rule_id.trim().is_empty() || self.rule_id.len() > MAX_RULE_ID_LEN {
            return Err(AlarmError::new(
                AlarmErrorKind::Invalid,
                "rule_id is empty or exceeds maximum length",
            ));
        }
        if self.exclusions.len() > MAX_EXCLUSIONS {
            return Err(AlarmError::new(
                AlarmErrorKind::Invalid,
                "too many exclusion rule ids",
            ));
        }
        for id in &self.exclusions {
            if id.trim().is_empty() || id.len() > MAX_EXCLUSION_ID_LEN {
                return Err(AlarmError::new(
                    AlarmErrorKind::Invalid,
                    "exclusion id is empty or exceeds maximum length",
                ));
            }
        }
        if self.actions.len() > MAX_ACTIONS {
            return Err(AlarmError::new(
                AlarmErrorKind::Invalid,
                "too many linkage actions",
            ));
        }
        for action in &self.actions {
            action.validate()?;
        }
        self.condition.validate(0)?;
        Ok(())
    }
}

/// Result of a linkage workflow run.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LinkageOutcome {
    /// All actions executed and their results are known.
    Success,
    /// Some action results cannot be confirmed.
    UnknownOutcome,
}

/// Port for executing linkage workflows.
#[async_trait::async_trait]
pub trait LinkageWorkflow: Send + Sync {
    /// Evaluate rules for the given alarm and execute the first matching rule.
    async fn run(
        &self,
        tenant_id: TenantId,
        alarm: &Alarm,
        rules: &[AlarmLinkageRule],
    ) -> Result<LinkageOutcome, AlarmError>;
}

/// Placeholder linkage workflow.
#[derive(Debug, Clone, Default)]
pub struct UnsupportedLinkageWorkflow {
    enabled: bool,
}

impl UnsupportedLinkageWorkflow {
    /// Create the port. When `enabled` is true it reports `Unsupported`;
    /// otherwise `Unavailable`.
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

#[async_trait::async_trait]
impl LinkageWorkflow for UnsupportedLinkageWorkflow {
    async fn run(
        &self,
        _tenant_id: TenantId,
        _alarm: &Alarm,
        rules: &[AlarmLinkageRule],
    ) -> Result<LinkageOutcome, AlarmError> {
        for rule in rules {
            rule.validate()?;
        }
        if self.enabled {
            Err(AlarmError::new(
                AlarmErrorKind::Unsupported,
                "linkage workflow execution is not implemented in this build",
            ))
        } else {
            Err(AlarmError::new(
                AlarmErrorKind::Unavailable,
                "linkage workflow is not configured",
            ))
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use foundation::{AlarmId, SystemClock, SystemIdGenerator, SystemRandom, UtcTimestamp};

    use super::*;

    fn make_rule() -> AlarmLinkageRule {
        let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
        AlarmLinkageRule {
            tenant_id: TenantId::generate(&generator).expect("generate tenant id"),
            rule_id: "rule-1".to_string(),
            condition: LinkageCondition::SeverityAtLeast(crate::Severity::High),
            actions: vec![],
            cooldown_seconds: 60,
            max_depth: 3,
            exclusions: HashSet::new(),
        }
    }

    fn err_or_panic<T, E: std::fmt::Debug>(result: Result<T, E>) -> E {
        match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        }
    }

    #[test]
    fn disabled_workflow_returns_unavailable() {
        futures::executor::block_on(async {
            let workflow = UnsupportedLinkageWorkflow::new(false);
            let e = err_or_panic(
                workflow
                    .run(make_rule().tenant_id, &make_sample_alarm(), &[make_rule()])
                    .await,
            );
            assert_eq!(e.kind, AlarmErrorKind::Unavailable);
        });
    }

    #[test]
    fn enabled_workflow_returns_unsupported() {
        futures::executor::block_on(async {
            let workflow = UnsupportedLinkageWorkflow::new(true);
            let e = err_or_panic(
                workflow
                    .run(make_rule().tenant_id, &make_sample_alarm(), &[make_rule()])
                    .await,
            );
            assert_eq!(e.kind, AlarmErrorKind::Unsupported);
        });
    }

    fn make_sample_alarm() -> Alarm {
        let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
        Alarm {
            id: AlarmId::generate(&generator).expect("generate alarm id"),
            tenant_id: TenantId::generate(&generator).expect("generate tenant id"),
            state: crate::AlarmState::New,
            severity: crate::Severity::High,
            title: "motion".to_string(),
            payload: serde_json::json!({}),
            dedup: None,
            evidence: vec![],
            assigned_to: None,
            revision: 1,
            created_at: UtcTimestamp::now(),
            updated_at: UtcTimestamp::now(),
        }
    }
}
