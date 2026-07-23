//! Performance baseline primitives.
//!
//! Phase 1 freezes the workload/runner contract. Real data generators,
//! request mixes, and P95 threshold validation are deferred to the benchmark
//! harness.

use std::collections::HashMap;

/// Performance test configuration.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PerformanceConfig {
    pub tenants: u32,
    pub users: u32,
    pub devices: u32,
    pub cameras: u32,
    pub concurrent_users: u32,
    pub duration_seconds: u32,
    pub hardware_profile: String,
    pub thresholds_p95_ms: HashMap<String, u32>,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        let mut thresholds = HashMap::new();
        thresholds.insert("db".to_string(), 50);
        thresholds.insert("authz".to_string(), 20);
        thresholds.insert("outbox".to_string(), 30);
        thresholds.insert("projection".to_string(), 100);
        thresholds.insert("player".to_string(), 200);
        Self {
            tenants: 100,
            users: 100_000,
            devices: 200_000,
            cameras: 200_000,
            concurrent_users: 1000,
            duration_seconds: 60,
            hardware_profile: "ci".to_string(),
            thresholds_p95_ms: thresholds,
        }
    }
}

/// Workload mix for a single scenario.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Workload {
    pub name: String,
    pub operations_per_second: u32,
    pub read_ratio: u8,
    pub write_ratio: u8,
}

/// Performance benchmark result.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PerformanceResult {
    pub config: PerformanceConfig,
    pub workloads: Vec<Workload>,
    pub p95_ms: HashMap<String, u32>,
    pub failures: Vec<String>,
}

impl PerformanceResult {
    /// Returns true if any reported P95 exceeds the configured threshold.
    pub fn threshold_violations(&self) -> Vec<String> {
        let mut violations = Vec::new();
        for (name, value) in &self.p95_ms {
            if let Some(threshold) = self.config.thresholds_p95_ms.get(name)
                && value > threshold
            {
                violations.push(format!("{name}: {value}ms > {threshold}ms"));
            }
        }
        violations
    }
}

/// Port for performance benchmark runners.
#[async_trait::async_trait]
pub trait PerformanceRunner: Send + Sync {
    async fn run(&self, config: &PerformanceConfig) -> Result<PerformanceResult, PerformanceError>;
}

/// Performance benchmark error.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error("{kind:?}: {message}")]
pub struct PerformanceError {
    pub kind: PerformanceErrorKind,
    pub message: String,
}

impl PerformanceError {
    pub fn new(kind: PerformanceErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

/// Kinds of performance failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PerformanceErrorKind {
    Unsupported,
    Unavailable,
    ThresholdViolation,
    Invalid,
}

/// Placeholder performance runner.
#[derive(Debug, Clone, Default)]
pub struct UnsupportedPerformanceRunner {
    enabled: bool,
}

impl UnsupportedPerformanceRunner {
    /// Create the runner. When `enabled` is true it reports `Unsupported`;
    /// otherwise `Unavailable`.
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

#[async_trait::async_trait]
impl PerformanceRunner for UnsupportedPerformanceRunner {
    async fn run(
        &self,
        _config: &PerformanceConfig,
    ) -> Result<PerformanceResult, PerformanceError> {
        if self.enabled {
            Err(PerformanceError::new(
                PerformanceErrorKind::Unsupported,
                "performance runner is not implemented in this build",
            ))
        } else {
            Err(PerformanceError::new(
                PerformanceErrorKind::Unavailable,
                "performance runner is not configured",
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
    async fn disabled_runner_returns_unavailable() {
        let runner = UnsupportedPerformanceRunner::new(false);
        let config = PerformanceConfig::default();
        let err = err_or_panic(runner.run(&config).await);
        assert_eq!(err.kind, PerformanceErrorKind::Unavailable);
    }

    #[tokio::test]
    async fn enabled_runner_returns_unsupported() {
        let runner = UnsupportedPerformanceRunner::new(true);
        let config = PerformanceConfig::default();
        let err = err_or_panic(runner.run(&config).await);
        assert_eq!(err.kind, PerformanceErrorKind::Unsupported);
    }

    #[test]
    fn result_reports_threshold_violations() {
        let mut config = PerformanceConfig::default();
        config.thresholds_p95_ms.insert("db".to_string(), 10);
        let result = PerformanceResult {
            config,
            workloads: vec![],
            p95_ms: [("db".to_string(), 25)].into_iter().collect(),
            failures: vec![],
        };
        let violations = result.threshold_violations();
        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("25ms > 10ms"));
    }
}
