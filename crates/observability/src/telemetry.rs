//! Telemetry, tracing, and metrics primitives.
//!
//! Phase 1 freezes the configuration and port shapes. Real `tracing`/OpenTelemetry
//! initialization is deferred; the stub returns `Unsupported`/`Unavailable`.

use std::collections::{HashMap, HashSet};

use foundation::TenantId;

/// W3C trace context propagated across HTTP/UoW/NATS/signaling/plugin boundaries.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TraceContext {
    pub trace_id: String,
    pub parent_id: Option<String>,
    pub trace_state: String,
    pub sampled: bool,
}

impl TraceContext {
    /// Parse a `traceparent` header value.
    pub fn parse_traceparent(value: &str) -> Result<Self, TelemetryError> {
        let parts: Vec<&str> = value.split('-').collect();
        if parts.len() < 4 {
            return Err(TelemetryError::new(
                TelemetryErrorKind::Invalid,
                "invalid traceparent format",
            ));
        }
        let sampled = parts[3] == "01";
        Ok(Self {
            trace_id: parts[1].to_string(),
            parent_id: Some(parts[2].to_string()),
            trace_state: String::new(),
            sampled,
        })
    }
}

/// Configuration for telemetry exporters.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TelemetryConfig {
    /// OTLP/collector endpoint. When `None`, telemetry is unavailable.
    pub exporter_endpoint: Option<String>,
    /// Service name used for traces and metrics.
    pub service_name: String,
    /// Labels that are globally safe to add to metrics.
    pub safe_labels: HashSet<String>,
}

/// Metric value with labels. High-cardinality IDs must not be used as labels.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MetricPoint {
    pub name: String,
    pub labels: HashMap<String, String>,
    pub value: i64,
}

/// In-memory metric registry that enforces the safe-label policy.
#[derive(Debug, Clone, Default)]
pub struct MetricRegistry {
    safe_labels: HashSet<String>,
    points: Vec<MetricPoint>,
}

impl MetricRegistry {
    pub fn new(safe_labels: HashSet<String>) -> Self {
        Self {
            safe_labels,
            points: Vec::new(),
        }
    }

    /// Record a metric point, rejecting labels that are not in the safe set.
    pub fn record(&mut self, point: MetricPoint) -> Result<(), TelemetryError> {
        for key in point.labels.keys() {
            if !self.safe_labels.contains(key) {
                return Err(TelemetryError::new(
                    TelemetryErrorKind::Invalid,
                    format!("label '{key}' is not in the safe-label allowlist"),
                ));
            }
        }
        self.points.push(point);
        Ok(())
    }
}

/// Telemetry error.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error("{kind:?}: {message}")]
pub struct TelemetryError {
    pub kind: TelemetryErrorKind,
    pub message: String,
}

impl TelemetryError {
    pub fn new(kind: TelemetryErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

/// Kinds of telemetry failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TelemetryErrorKind {
    Invalid,
    Unsupported,
    Unavailable,
    Backpressure,
}

/// Port for telemetry initialization.
#[async_trait::async_trait]
pub trait TelemetryInitializer: Send + Sync {
    async fn init(&self, config: &TelemetryConfig) -> Result<(), TelemetryError>;
    async fn redact(&self, text: &str, tenant_id: TenantId) -> String;
}

/// Placeholder telemetry initializer.
#[derive(Debug, Clone, Default)]
pub struct UnsupportedTelemetryInitializer {
    enabled: bool,
}

impl UnsupportedTelemetryInitializer {
    /// Create the initializer. When `enabled` is true it reports `Unsupported`;
    /// otherwise `Unavailable`.
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

#[async_trait::async_trait]
impl TelemetryInitializer for UnsupportedTelemetryInitializer {
    async fn init(&self, config: &TelemetryConfig) -> Result<(), TelemetryError> {
        if config.exporter_endpoint.is_none() && !self.enabled {
            return Err(TelemetryError::new(
                TelemetryErrorKind::Unavailable,
                "telemetry exporter is not configured",
            ));
        }
        Err(TelemetryError::new(
            TelemetryErrorKind::Unsupported,
            "telemetry initialization is not implemented in this build",
        ))
    }

    async fn redact(&self, _text: &str, _tenant_id: TenantId) -> String {
        "[REDACTED]".to_string()
    }
}

#[cfg(test)]
mod tests {
    use foundation::{SystemClock, SystemIdGenerator, SystemRandom};

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

    #[test]
    fn traceparent_parses_sampled_flag() {
        let ctx = ok_or_panic(TraceContext::parse_traceparent(
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
        ));
        assert!(ctx.sampled);
        assert_eq!(ctx.trace_id, "4bf92f3577b34da6a3ce929d0e0e4736");
    }

    #[test]
    fn metric_registry_rejects_high_cardinality_labels() {
        let mut registry = MetricRegistry::new(["tenant_bucket".to_string()].into_iter().collect());
        let point = MetricPoint {
            name: "requests".to_string(),
            labels: [("user_id".to_string(), "123".to_string())]
                .into_iter()
                .collect(),
            value: 1,
        };
        let err = err_or_panic(registry.record(point));
        assert_eq!(err.kind, TelemetryErrorKind::Invalid);
    }

    #[tokio::test]
    async fn unconfigured_initializer_returns_unavailable() {
        let init = UnsupportedTelemetryInitializer::new(false);
        let config = TelemetryConfig::default();
        let result = init.init(&config).await;
        assert_eq!(err_or_panic(result).kind, TelemetryErrorKind::Unavailable);
    }

    #[tokio::test]
    async fn redaction_masks_text() {
        let init = UnsupportedTelemetryInitializer::new(false);
        let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
        let redacted = init
            .redact("secret=abc", TenantId::generate(&generator))
            .await;
        assert_eq!(redacted, "[REDACTED]");
    }
}
