//! Cluster assembly: role-aware startup, readiness, and graceful shutdown.
//!
//! Phase 1 supports a single-process `All` role only as a skeleton. Live cluster
//! assembly with separate binaries, readiness probes, and rolling drain requires
//! NATS KV, JetStream, and the application service wiring; those return
//! `Unsupported`/`Unavailable` until implemented.

use foundation::NodeId;

use crate::{ClusterAdapterConfig, ClusterError, ClusterErrorKind, Role};

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
