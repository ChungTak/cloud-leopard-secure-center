//! Signaling events consumed from an upstream SSE stream.

use foundation::{DeviceId, TenantId, UtcTimestamp};
use serde::{Deserialize, Serialize};

/// An event received from an upstream signaling SSE stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalingEvent {
    /// `Last-Event-ID` value used for resumption and inbox deduplication.
    pub last_event_id: String,
    pub tenant_id: TenantId,
    pub device_id: DeviceId,
    pub observed_at: UtcTimestamp,
    pub payload: SignalingEventPayload,
}

/// Payload variants for signaling events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SignalingEventPayload {
    DeviceOnline,
    DeviceOffline,
    ChannelState {
        channel_id: String,
        #[serde(rename = "isEnabled")]
        is_enabled: bool,
    },
    /// A gap was detected in the upstream event stream.
    Gap,
}
