//! REST and cheetah proto DTOs for signaling integration.
//!
//! Phase 1 freezes the shapes used by callers. Proto decoding is explicitly
//! rejected with `SignalingErrorKind::Unsupported` until upstream descriptors are
//! published.

use foundation::{DeviceId, MediaSessionId, OperationId, TenantId};
use serde::{Deserialize, Serialize};

/// Device projection as returned by a signaling upstream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalingDeviceDto {
    pub tenant_id: TenantId,
    pub device_id: DeviceId,
    pub online_state: String,
    pub observed_at: String,
    pub channels: Vec<SignalingChannelDto>,
}

/// Channel projection returned by a signaling upstream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalingChannelDto {
    pub channel_id: String,
    pub channel_name: String,
    pub is_enabled: bool,
}

/// REST request body to create an upstream operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateOperationRestDto {
    pub device_id: DeviceId,
    pub parameters: serde_json::Value,
}

/// REST request body to create a media session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMediaSessionRestDto {
    pub operation_id: OperationId,
    pub parameters: serde_json::Value,
}

/// Operation DTO returned by REST adapters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationDto {
    pub id: OperationId,
    pub tenant_id: TenantId,
    pub device_id: DeviceId,
    pub state: String,
    pub deadline: String,
}

/// Media session DTO returned by REST adapters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSessionDto {
    pub id: MediaSessionId,
    pub tenant_id: TenantId,
    pub operation_id: OperationId,
    pub state: String,
    pub deadline: String,
}
