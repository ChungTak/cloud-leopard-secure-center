//! Wasm WIT host port.
//!
//! Phase 1 freezes the host capabilities exposed to guest plugins:
//! `log`, `read_config`, `query_resource`, `create_alarm`, and `publish_event`.
//! Real Wasmtime sandboxing (fuel, epoch deadline, memory/call/output/event/log
//! limits) is deferred.

use foundation::{AlarmId, DeviceId, PluginId, TenantId};

use crate::manifest::{PluginError, PluginErrorKind};

/// Resource identifier passed to `query_resource`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ResourceQuery {
    pub tenant_id: TenantId,
    pub device_id: Option<DeviceId>,
    pub resource_type: String,
}

/// Event published by a plugin.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PluginEvent {
    pub tenant_id: TenantId,
    pub plugin_id: PluginId,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub causation: Vec<String>,
    pub depth: u32,
}

/// Execution limits for a single plugin invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WasmLimits {
    pub fuel: u64,
    pub memory_pages: u32,
    pub max_calls: u32,
    pub max_output_bytes: u32,
    pub max_events: u32,
    pub max_log_lines: u32,
}

impl Default for WasmLimits {
    fn default() -> Self {
        Self {
            fuel: 1_000_000,
            memory_pages: 128,
            max_calls: 10_000,
            max_output_bytes: 64 * 1024,
            max_events: 256,
            max_log_lines: 1024,
        }
    }
}

/// WIT host capabilities.
#[async_trait::async_trait]
pub trait WitHost: Send + Sync {
    async fn log(&self, plugin: PluginId, level: &str, message: &str) -> Result<(), PluginError>;
    async fn read_config(&self, plugin: PluginId, key: &str) -> Result<String, PluginError>;
    async fn query_resource(
        &self,
        plugin: PluginId,
        query: &ResourceQuery,
    ) -> Result<serde_json::Value, PluginError>;
    async fn create_alarm(
        &self,
        plugin: PluginId,
        tenant_id: TenantId,
        title: &str,
        severity: u8,
    ) -> Result<AlarmId, PluginError>;
    async fn publish_event(&self, plugin: PluginId, event: &PluginEvent)
    -> Result<(), PluginError>;
    fn limits(&self) -> WasmLimits;
}

/// Placeholder WIT host.
#[derive(Debug, Clone, Default)]
pub struct UnsupportedWitHost {
    enabled: bool,
}

impl UnsupportedWitHost {
    /// Create the host. When `enabled` is true it reports `Unsupported`;
    /// otherwise `Unavailable`.
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

#[async_trait::async_trait]
impl WitHost for UnsupportedWitHost {
    async fn log(
        &self,
        _plugin: PluginId,
        _level: &str,
        _message: &str,
    ) -> Result<(), PluginError> {
        self.reject("log")
    }

    async fn read_config(&self, _plugin: PluginId, _key: &str) -> Result<String, PluginError> {
        self.reject("read_config")
    }

    async fn query_resource(
        &self,
        _plugin: PluginId,
        _query: &ResourceQuery,
    ) -> Result<serde_json::Value, PluginError> {
        self.reject("query_resource")
    }

    async fn create_alarm(
        &self,
        _plugin: PluginId,
        _tenant_id: TenantId,
        _title: &str,
        _severity: u8,
    ) -> Result<AlarmId, PluginError> {
        self.reject("create_alarm")
    }

    async fn publish_event(
        &self,
        _plugin: PluginId,
        _event: &PluginEvent,
    ) -> Result<(), PluginError> {
        self.reject("publish_event")
    }

    fn limits(&self) -> WasmLimits {
        WasmLimits::default()
    }
}

impl UnsupportedWitHost {
    fn reject<T>(&self, action: &str) -> Result<T, PluginError> {
        if self.enabled {
            Err(PluginError::new(
                PluginErrorKind::Unsupported,
                format!("WIT host {action} is not implemented in this build"),
            ))
        } else {
            Err(PluginError::new(
                PluginErrorKind::Unavailable,
                "WIT host is not configured",
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
    async fn disabled_wit_host_returns_unavailable() {
        let host = UnsupportedWitHost::new(false);
        let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
        let plugin_id = PluginId::generate(&generator).expect("generate plugin id");
        let result = host.log(plugin_id, "info", "x").await;
        assert_eq!(err_or_panic(result).kind, PluginErrorKind::Unavailable);
    }

    #[tokio::test]
    async fn enabled_wit_host_returns_unsupported() {
        let host = UnsupportedWitHost::new(true);
        let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
        let plugin_id = PluginId::generate(&generator).expect("generate plugin id");
        let result = host.read_config(plugin_id, "key").await;
        assert_eq!(err_or_panic(result).kind, PluginErrorKind::Unsupported);
    }

    #[test]
    fn default_limits_are_finite() {
        let host = UnsupportedWitHost::new(false);
        let limits = host.limits();
        assert!(limits.fuel > 0 && limits.memory_pages > 0);
    }
}
