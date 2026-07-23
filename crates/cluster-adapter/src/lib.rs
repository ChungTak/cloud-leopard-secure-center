//! Cluster runtime adapter: node leases and role scheduling.
//!
//! Phase 1 does not include a live cluster runtime (NATS KV, workflow/scheduler,
//! DB revision guards). The port and descriptor types are frozen here so that
//! `security-platform` can assemble role-aware binaries; unimplemented paths
//! return `Unsupported` or `Unavailable` depending on configuration.

use foundation::{NodeId, TenantId, UtcTimestamp};

pub mod assembly;

/// Role a binary can run as. `All` starts all supported roles in a single process.
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

impl Role {
    /// Expand a role into the concrete roles that must be started.
    /// `Role::All` expands to all supported roles except `All` itself.
    pub fn expand(&self) -> Vec<Role> {
        match self {
            Role::All => vec![
                Role::Api,
                Role::Workflow,
                Role::Projection,
                Role::Scheduler,
                Role::PluginHost,
            ],
            other => vec![*other],
        }
    }
}

/// Capabilities advertised by a node.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NodeCapabilities {
    pub zone: Option<String>,
    pub build: String,
    pub max_tasks: u32,
    pub contracts: Vec<String>,
}

/// Node descriptor used for lease and scheduling decisions.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NodeDescriptor {
    pub node_id: NodeId,
    pub roles: Vec<Role>,
    pub capabilities: NodeCapabilities,
    pub started_at: UtcTimestamp,
    pub expires_at: UtcTimestamp,
}

/// Node lease record with CAS/epoch fencing.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NodeLease {
    pub node_id: NodeId,
    pub epoch: u64,
    pub tenant_id: Option<TenantId>,
    pub role: Role,
    pub acquired_at: UtcTimestamp,
    pub expires_at: UtcTimestamp,
}

/// Configuration for the cluster adapter.
#[derive(Debug, Clone, Default)]
pub struct ClusterAdapterConfig {
    /// NATS servers used for KV-backed node leases. When `None`, lease operations
    /// return `Unavailable`.
    pub nats_servers: Option<String>,
}

/// Errors returned by cluster runtime operations.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error("{kind:?}: {message}")]
pub struct ClusterError {
    pub kind: ClusterErrorKind,
    pub message: String,
}

impl ClusterError {
    /// Create an error of the given kind with a human-readable message.
    pub fn new(kind: ClusterErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

/// Categorization of cluster runtime failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ClusterErrorKind {
    Unsupported,
    Unavailable,
    LeaseConflict,
    Expired,
    Invalid,
    Unauthorized,
}

/// Port for node lease and role scheduling operations.
#[async_trait::async_trait]
pub trait RoleScheduler: Send + Sync {
    /// Claim a lease for the given role on this node.
    async fn claim_lease(
        &self,
        descriptor: &NodeDescriptor,
        role: Role,
    ) -> Result<NodeLease, ClusterError>;

    /// Release a previously claimed lease.
    async fn release_lease(&self, lease: &NodeLease) -> Result<(), ClusterError>;

    /// Mark the node as draining; it should finish in-flight work and refuse new
    /// scheduling.
    async fn drain(&self, node_id: NodeId) -> Result<(), ClusterError>;

    /// Schedule a single task for a tenant, protected by a DB revision and lease.
    async fn schedule_task(
        &self,
        tenant_id: TenantId,
        role: Role,
        task_id: &str,
        expected_revision: u64,
    ) -> Result<u64, ClusterError>;
}

/// Cluster runtime adapter that fulfills the `RoleScheduler` port.
#[derive(Debug, Clone, Default)]
pub struct ClusterRuntime {
    config: ClusterAdapterConfig,
}

impl ClusterRuntime {
    /// Create a cluster runtime from configuration.
    pub fn new(config: ClusterAdapterConfig) -> Self {
        Self { config }
    }

    fn unsupported(action: &str) -> ClusterError {
        ClusterError::new(
            ClusterErrorKind::Unsupported,
            format!("{action} is not implemented in this build"),
        )
    }

    fn unavailable() -> ClusterError {
        ClusterError::new(
            ClusterErrorKind::Unavailable,
            "cluster runtime is not configured",
        )
    }

    fn check(&self, action: &str) -> Result<(), ClusterError> {
        if self.config.nats_servers.is_some() {
            Err(Self::unsupported(action))
        } else {
            Err(Self::unavailable())
        }
    }
}

#[async_trait::async_trait]
impl RoleScheduler for ClusterRuntime {
    async fn claim_lease(
        &self,
        _descriptor: &NodeDescriptor,
        _role: Role,
    ) -> Result<NodeLease, ClusterError> {
        self.check("claim_lease")?;
        unreachable!("error always returned above")
    }

    async fn release_lease(&self, _lease: &NodeLease) -> Result<(), ClusterError> {
        self.check("release_lease")?;
        unreachable!("error always returned above")
    }

    async fn drain(&self, _node_id: NodeId) -> Result<(), ClusterError> {
        self.check("drain")?;
        unreachable!("error always returned above")
    }

    async fn schedule_task(
        &self,
        _tenant_id: TenantId,
        _role: Role,
        _task_id: &str,
        _expected_revision: u64,
    ) -> Result<u64, ClusterError> {
        self.check("schedule_task")?;
        unreachable!("error always returned above")
    }
}

#[cfg(test)]
mod tests {
    use foundation::{SystemClock, SystemIdGenerator, SystemRandom};

    use super::*;

    fn sample_descriptor() -> NodeDescriptor {
        let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
        let now = UtcTimestamp::now();
        NodeDescriptor {
            node_id: NodeId::generate(&generator),
            roles: vec![Role::Scheduler, Role::Api],
            capabilities: NodeCapabilities {
                zone: Some("zone-a".to_string()),
                build: env!("CARGO_PKG_VERSION").to_string(),
                max_tasks: 64,
                contracts: vec!["media".to_string()],
            },
            started_at: now,
            expires_at: now,
        }
    }

    #[tokio::test]
    async fn unconfigured_cluster_returns_unavailable() {
        let runtime = ClusterRuntime::new(ClusterAdapterConfig::default());
        match runtime
            .claim_lease(&sample_descriptor(), Role::Scheduler)
            .await
        {
            Ok(_) => panic!("expected unavailable"),
            Err(e) => assert_eq!(e.kind, ClusterErrorKind::Unavailable),
        }
    }

    #[tokio::test]
    async fn configured_cluster_returns_unsupported() {
        let config = ClusterAdapterConfig {
            nats_servers: Some("nats://localhost:4222".to_string()),
        };
        let runtime = ClusterRuntime::new(config);
        match runtime
            .claim_lease(&sample_descriptor(), Role::Scheduler)
            .await
        {
            Ok(_) => panic!("expected unsupported"),
            Err(e) => assert_eq!(e.kind, ClusterErrorKind::Unsupported),
        }
    }
}
