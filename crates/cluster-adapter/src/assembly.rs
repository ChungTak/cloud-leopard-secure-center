//! Cluster assembly: role-aware startup, readiness, and graceful shutdown.
//!
//! Phase 1 supports a single-process `All` role only as a skeleton. Live cluster
//! assembly with separate binaries, readiness probes, and rolling drain requires
//! NATS KV, JetStream, and the application service wiring; those return
//! `Unsupported`/`Unavailable` until implemented.

use std::collections::HashSet;

use foundation::NodeId;

use crate::{ClusterAdapterConfig, ClusterError, ClusterErrorKind, Role};

/// A named phase in the node lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum LifecyclePhase {
    Config,
    Secret,
    SchemaCheck,
    MessageBus,
    Repositories,
    Workers,
    Listeners,
    Ready,
    Drain,
}

/// Fixed startup and shutdown phase ordering for role-aware binaries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lifecycle {
    startup: Vec<LifecyclePhase>,
    shutdown: Vec<LifecyclePhase>,
}

impl Lifecycle {
    /// The canonical startup order: config/secret -> schema check -> bus ->
    /// repositories -> workers -> listeners -> ready.
    pub fn startup() -> &'static [LifecyclePhase] {
        &[
            LifecyclePhase::Config,
            LifecyclePhase::Secret,
            LifecyclePhase::SchemaCheck,
            LifecyclePhase::MessageBus,
            LifecyclePhase::Repositories,
            LifecyclePhase::Workers,
            LifecyclePhase::Listeners,
            LifecyclePhase::Ready,
        ]
    }

    /// The canonical shutdown order: ready -> listeners -> workers -> repositories
    /// -> bus -> schema check -> secret -> config, then drain.
    pub fn shutdown() -> &'static [LifecyclePhase] {
        &[
            LifecyclePhase::Ready,
            LifecyclePhase::Listeners,
            LifecyclePhase::Workers,
            LifecyclePhase::Repositories,
            LifecyclePhase::MessageBus,
            LifecyclePhase::SchemaCheck,
            LifecyclePhase::Secret,
            LifecyclePhase::Config,
            LifecyclePhase::Drain,
        ]
    }

    /// Validate that `phases` contains all startup phases in order without
    /// duplicates.
    pub fn validate_startup(phases: &[LifecyclePhase]) -> Result<(), ClusterError> {
        let required: HashSet<_> = Self::startup().iter().copied().collect();
        let present: HashSet<_> = phases.iter().copied().collect();
        let missing: Vec<_> = required.difference(&present).copied().collect();
        if !missing.is_empty() {
            return Err(ClusterError::new(
                ClusterErrorKind::Invalid,
                format!("missing startup phases: {missing:?}"),
            ));
        }
        let mut last = None;
        for phase in phases {
            if Some(*phase) == last {
                return Err(ClusterError::new(
                    ClusterErrorKind::Invalid,
                    format!("duplicate startup phase: {phase:?}"),
                ));
            }
            last = Some(*phase);
        }
        Ok(())
    }

    /// Validate that `phases` contains all shutdown phases in reverse order
    /// and ends with a bounded drain.
    pub fn validate_shutdown(phases: &[LifecyclePhase]) -> Result<(), ClusterError> {
        if !matches!(phases.last(), Some(LifecyclePhase::Drain)) {
            return Err(ClusterError::new(
                ClusterErrorKind::Invalid,
                "shutdown sequence must end with Drain",
            ));
        }
        let mut seen = HashSet::new();
        for phase in phases {
            if !seen.insert(*phase) {
                return Err(ClusterError::new(
                    ClusterErrorKind::Invalid,
                    format!("duplicate shutdown phase: {phase:?}"),
                ));
            }
        }
        Ok(())
    }
}

/// Cluster assembler that builds and tears down a role-specific runtime.
#[derive(Debug, Clone, Default)]
pub struct ClusterAssembler {
    config: ClusterAdapterConfig,
}

impl ClusterAssembler {
    /// Create an assembler from configuration.
    pub fn new(config: ClusterAdapterConfig) -> Self {
        Self { config }
    }

    fn check(&self, action: &str) -> Result<(), ClusterError> {
        if self.config.nats_servers.is_some() {
            Err(ClusterError::new(
                ClusterErrorKind::Unsupported,
                format!("{action} is not implemented in this build"),
            ))
        } else {
            Err(ClusterError::new(
                ClusterErrorKind::Unavailable,
                "cluster assembly is not configured",
            ))
        }
    }

    /// Start the runtime for a single role or `Role::All`.
    pub async fn run(&self, _role: Role) -> Result<(), ClusterError> {
        self.check("run")?;
        unreachable!("error always returned above")
    }

    /// Validate the node lifecycle ordering before starting a role.
    pub fn validate_lifecycle(
        &self,
        startup: &[LifecyclePhase],
        shutdown: &[LifecyclePhase],
    ) -> Result<(), ClusterError> {
        Lifecycle::validate_startup(startup)?;
        Lifecycle::validate_shutdown(shutdown)?;
        Ok(())
    }

    /// Role-aware readiness probe. Each role becomes ready when its dependencies
    /// are wired.
    pub async fn ready(&self, _role: Role) -> Result<bool, ClusterError> {
        self.check("ready")?;
        unreachable!("error always returned above")
    }

    /// Gracefully shut down a role: drain, stop consumers, stop listeners.
    pub async fn shutdown(&self, _node_id: NodeId) -> Result<(), ClusterError> {
        self.check("shutdown")?;
        unreachable!("error always returned above")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_or_panic<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
        match result {
            Ok(v) => v,
            Err(e) => panic!("{e}"),
        }
    }

    #[test]
    fn role_all_expands_to_all_supported_roles() {
        let roles = Role::All.expand();
        let expected = vec![
            Role::Api,
            Role::Workflow,
            Role::Projection,
            Role::Scheduler,
            Role::PluginHost,
        ];
        assert_eq!(roles, expected);
    }

    #[test]
    fn single_role_expands_to_itself() {
        assert_eq!(Role::Api.expand(), vec![Role::Api]);
    }

    #[test]
    fn lifecycle_startup_order_is_valid() {
        ok_or_panic(Lifecycle::validate_startup(Lifecycle::startup()));
    }

    #[test]
    fn lifecycle_shutdown_ends_with_drain() {
        ok_or_panic(Lifecycle::validate_shutdown(Lifecycle::shutdown()));
    }

    #[test]
    fn lifecycle_shutdown_missing_drain_fails() {
        let phases = vec![LifecyclePhase::Ready, LifecyclePhase::Listeners];
        let err = match Lifecycle::validate_shutdown(&phases) {
            Ok(_) => panic!("expected invalid shutdown"),
            Err(e) => e,
        };
        assert_eq!(err.kind, ClusterErrorKind::Invalid);
    }

    #[tokio::test]
    async fn unconfigured_assembler_returns_unavailable() {
        let assembler = ClusterAssembler::new(ClusterAdapterConfig::default());
        match assembler.run(Role::Api).await {
            Ok(_) => panic!("expected unavailable"),
            Err(e) => assert_eq!(e.kind, ClusterErrorKind::Unavailable),
        }
    }

    #[tokio::test]
    async fn configured_assembler_returns_unsupported() {
        let config = ClusterAdapterConfig {
            nats_servers: Some("nats://localhost:4222".to_string()),
        };
        let assembler = ClusterAssembler::new(config);
        match assembler.ready(Role::All).await {
            Ok(_) => panic!("expected unsupported"),
            Err(e) => assert_eq!(e.kind, ClusterErrorKind::Unsupported),
        }
    }
}
