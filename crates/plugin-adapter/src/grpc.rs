//! Process plugin gRPC host port.
//!
//! Phase 1 freezes the UDS/mTLS handshake and frame contract. Real gRPC server
//! and client code is deferred; the stub returns `Unsupported`/`Unavailable`.

use foundation::PluginId;

use crate::manifest::{PluginError, PluginErrorKind};

/// Handshake initiated by the plugin.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PluginHello {
    pub plugin_id: PluginId,
    pub version: String,
    pub instance: String,
    pub scope: Vec<String>,
    pub credits: u32,
}

/// Host response welcoming the plugin.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HostWelcome {
    pub heartbeat_seconds: u32,
    pub config_revision: u64,
    pub allowed_capabilities: Vec<String>,
}

/// Frame exchanged after handshake.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PluginFrame {
    Command {
        seq: u64,
        command: String,
        payload: serde_json::Value,
    },
    Result {
        ack: u64,
        success: bool,
        payload: serde_json::Value,
    },
    Event {
        seq: u64,
        event_type: String,
        payload: serde_json::Value,
    },
    Health {
        seq: u64,
    },
    Drain,
    Shutdown,
}

/// Process plugin host port.
#[async_trait::async_trait]
pub trait ProcessPluginHost: Send + Sync {
    async fn handshake(&self, hello: &PluginHello) -> Result<HostWelcome, PluginError>;
    async fn send_command(
        &self,
        plugin_id: PluginId,
        frame: &PluginFrame,
    ) -> Result<(), PluginError>;
    async fn receive_frame(&self, plugin_id: PluginId) -> Result<PluginFrame, PluginError>;
    async fn shutdown(&self, plugin_id: PluginId) -> Result<(), PluginError>;
}

/// Placeholder process plugin host.
#[derive(Debug, Clone, Default)]
pub struct UnsupportedProcessPluginHost {
    enabled: bool,
}

impl UnsupportedProcessPluginHost {
    /// Create the host. When `enabled` is true it reports `Unsupported`;
    /// otherwise `Unavailable`.
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

#[async_trait::async_trait]
impl ProcessPluginHost for UnsupportedProcessPluginHost {
    async fn handshake(&self, _hello: &PluginHello) -> Result<HostWelcome, PluginError> {
        self.reject("handshake")
    }

    async fn send_command(
        &self,
        _plugin_id: PluginId,
        _frame: &PluginFrame,
    ) -> Result<(), PluginError> {
        self.reject("send_command")
    }

    async fn receive_frame(&self, _plugin_id: PluginId) -> Result<PluginFrame, PluginError> {
        self.reject("receive_frame")
    }

    async fn shutdown(&self, _plugin_id: PluginId) -> Result<(), PluginError> {
        self.reject("shutdown")
    }
}

impl UnsupportedProcessPluginHost {
    fn reject<T>(&self, action: &str) -> Result<T, PluginError> {
        if self.enabled {
            Err(PluginError::new(
                PluginErrorKind::Unsupported,
                format!("process plugin gRPC host {action} is not implemented in this build"),
            ))
        } else {
            Err(PluginError::new(
                PluginErrorKind::Unavailable,
                "process plugin gRPC host is not configured",
            ))
        }
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

    fn make_hello() -> PluginHello {
        let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
        PluginHello {
            plugin_id: ok_or_panic(PluginId::generate(&generator)),
            version: "0.1.0".to_string(),
            instance: "instance-1".to_string(),
            scope: vec!["read".to_string()],
            credits: 10,
        }
    }

    #[tokio::test]
    async fn disabled_host_returns_unavailable() {
        let host = UnsupportedProcessPluginHost::new(false);
        let result = host.handshake(&make_hello()).await;
        assert_eq!(err_or_panic(result).kind, PluginErrorKind::Unavailable);
    }

    #[tokio::test]
    async fn enabled_host_returns_unsupported() {
        let host = UnsupportedProcessPluginHost::new(true);
        let result = host.handshake(&make_hello()).await;
        assert_eq!(err_or_panic(result).kind, PluginErrorKind::Unsupported);
    }

    #[test]
    fn frame_variants_roundtrip() {
        let frame = PluginFrame::Command {
            seq: 1,
            command: "do".to_string(),
            payload: serde_json::json!({"x": 1}),
        };
        let serialized = ok_or_panic(serde_json::to_string(&frame));
        let deserialized: PluginFrame = ok_or_panic(serde_json::from_str(&serialized));
        assert_eq!(frame, deserialized);
    }
}
