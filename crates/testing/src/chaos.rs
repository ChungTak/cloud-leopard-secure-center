//! Fault injection and long-running stability primitives.
//!
//! Phase 1 freezes the chaos scenario and fault injector contract. Real
//! PostgreSQL failover, NATS partitions, signaling/media/plugin crashes, disk
//! exhaustion, clock skew injection, and 72h soak runs are deferred to the
//! resilience test harness.

/// Fault scenario that can be injected into a running platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ChaosScenario {
    PostgresFailover,
    NatsPartition,
    SignalingCrash,
    MediaCrash,
    PluginCrash,
    DiskFull,
    ClockSkew,
    NetworkLatency,
}

/// Scope of a fault injection run.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChaosRun {
    pub scenarios: Vec<ChaosScenario>,
    pub duration_seconds: u32,
    pub tenants: u32,
    pub max_concurrent_faults: u32,
}

/// Result of a chaos/soak run.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChaosReport {
    pub run: ChaosRun,
    pub tenant_isolation_violations: Vec<String>,
    pub duplicate_side_effects: Vec<String>,
    pub rejected_old_epoch_events: u64,
    pub backlog_recovered: bool,
    pub memory_trend_kb: Vec<u64>,
    pub connection_trend: Vec<u64>,
    pub lag_trend_ms: Vec<u64>,
    pub recovery_time_ms: u64,
    pub soak_hours: u32,
}

/// Port for injecting faults and running soak tests.
#[async_trait::async_trait]
pub trait FaultInjector: Send + Sync {
    async fn inject(&self, scenario: ChaosScenario) -> Result<(), ChaosError>;
    async fn run_soak(&self, run: &ChaosRun) -> Result<ChaosReport, ChaosError>;
}

/// Chaos/soak error.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error("{kind:?}: {message}")]
pub struct ChaosError {
    pub kind: ChaosErrorKind,
    pub message: String,
}

impl ChaosError {
    pub fn new(kind: ChaosErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

/// Kinds of chaos/soak failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ChaosErrorKind {
    Unsupported,
    Unavailable,
    Invalid,
    TenantIsolationViolation,
    DuplicateSideEffect,
    OldEpochRejected,
    BacklogNotRecovered,
}

/// Placeholder fault injector.
#[derive(Debug, Clone, Default)]
pub struct UnsupportedFaultInjector {
    enabled: bool,
}

impl UnsupportedFaultInjector {
    /// Create the injector. When `enabled` is true it reports `Unsupported`;
    /// otherwise `Unavailable`.
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

#[async_trait::async_trait]
impl FaultInjector for UnsupportedFaultInjector {
    async fn inject(&self, _scenario: ChaosScenario) -> Result<(), ChaosError> {
        if self.enabled {
            Err(ChaosError::new(
                ChaosErrorKind::Unsupported,
                "fault injection is not implemented in this build",
            ))
        } else {
            Err(ChaosError::new(
                ChaosErrorKind::Unavailable,
                "fault injector is not configured",
            ))
        }
    }

    async fn run_soak(&self, _run: &ChaosRun) -> Result<ChaosReport, ChaosError> {
        if self.enabled {
            Err(ChaosError::new(
                ChaosErrorKind::Unsupported,
                "soak testing is not implemented in this build",
            ))
        } else {
            Err(ChaosError::new(
                ChaosErrorKind::Unavailable,
                "soak runner is not configured",
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

    #[tokio::test]
    async fn disabled_injector_returns_unavailable() {
        let injector = UnsupportedFaultInjector::new(false);
        let err = err_or_panic(injector.inject(ChaosScenario::DiskFull).await);
        assert_eq!(err.kind, ChaosErrorKind::Unavailable);
    }

    #[tokio::test]
    async fn enabled_soak_returns_unsupported() {
        let injector = UnsupportedFaultInjector::new(true);
        let run = ChaosRun {
            scenarios: vec![ChaosScenario::PostgresFailover],
            duration_seconds: 1,
            tenants: 1,
            max_concurrent_faults: 1,
        };
        let err = err_or_panic(injector.run_soak(&run).await);
        assert_eq!(err.kind, ChaosErrorKind::Unsupported);
    }

    #[test]
    fn report_tracks_isolation_and_recovery() {
        let run = ChaosRun {
            scenarios: vec![],
            duration_seconds: 0,
            tenants: 1,
            max_concurrent_faults: 0,
        };
        let report = ChaosReport {
            run,
            tenant_isolation_violations: vec![],
            duplicate_side_effects: vec![],
            rejected_old_epoch_events: 1,
            backlog_recovered: true,
            memory_trend_kb: vec![],
            connection_trend: vec![],
            lag_trend_ms: vec![],
            recovery_time_ms: 100,
            soak_hours: 72,
        };
        assert!(report.backlog_recovered);
        assert_eq!(report.soak_hours, 72);
    }
}
