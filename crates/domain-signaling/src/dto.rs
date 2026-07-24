//! REST and cheetah proto DTOs for signaling integration.
//!
//! Phase 1 freezes the shapes used by callers. Proto decoding is explicitly
//! rejected with `SignalingErrorKind::Unsupported` until upstream descriptors are
//! published.

use std::collections::HashSet;

use foundation::{DeviceId, MediaSessionId, OperationId, TenantId};
use serde::{Deserialize, Serialize};

use crate::{SignalingError, SignalingErrorKind};

const MAX_ONLINE_STATE_LEN: usize = 64;
const MAX_OBSERVED_AT_LEN: usize = 64;
const MAX_CHANNEL_ID_LEN: usize = 256;
const MAX_CHANNEL_NAME_LEN: usize = 256;
const MAX_CHANNELS: usize = 256;
const MAX_PARAMETERS_BYTES: usize = 64 * 1024;
const MAX_STATE_STRING_LEN: usize = 64;
const MAX_DEADLINE_STRING_LEN: usize = 64;

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

impl SignalingDeviceDto {
    /// Validate that the device projection fields are within bounds. This should
    /// be called immediately after deserializing a response from an upstream
    /// signaling system so that oversized or malicious payloads are rejected
    /// before being stored in a projection.
    pub fn validate(&self) -> Result<(), SignalingError> {
        if self.online_state.len() > MAX_ONLINE_STATE_LEN {
            return Err(SignalingError::new(
                SignalingErrorKind::Invalid,
                "online_state exceeds maximum length",
            ));
        }
        if self.observed_at.len() > MAX_OBSERVED_AT_LEN {
            return Err(SignalingError::new(
                SignalingErrorKind::Invalid,
                "observed_at exceeds maximum length",
            ));
        }
        if self.channels.len() > MAX_CHANNELS {
            return Err(SignalingError::new(
                SignalingErrorKind::Invalid,
                "channels exceed maximum count",
            ));
        }
        let mut seen = HashSet::new();
        for channel in &self.channels {
            channel.validate()?;
            if !seen.insert(&channel.channel_id) {
                return Err(SignalingError::new(
                    SignalingErrorKind::Invalid,
                    "duplicate channel_id in device projection",
                ));
            }
        }
        Ok(())
    }
}

/// Channel projection returned by a signaling upstream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalingChannelDto {
    pub channel_id: String,
    pub channel_name: String,
    pub is_enabled: bool,
}

impl SignalingChannelDto {
    /// Validate channel fields before accepting an upstream projection.
    pub fn validate(&self) -> Result<(), SignalingError> {
        if self.channel_id.is_empty() || self.channel_id.len() > MAX_CHANNEL_ID_LEN {
            return Err(SignalingError::new(
                SignalingErrorKind::Invalid,
                "channel_id is empty or exceeds maximum length",
            ));
        }
        if self.channel_name.len() > MAX_CHANNEL_NAME_LEN {
            return Err(SignalingError::new(
                SignalingErrorKind::Invalid,
                "channel_name exceeds maximum length",
            ));
        }
        Ok(())
    }
}

/// REST request body to create an upstream operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateOperationRestDto {
    pub device_id: DeviceId,
    pub parameters: serde_json::Value,
}

impl CreateOperationRestDto {
    /// Validate the request body before it is forwarded to the upstream
    /// signaling system.
    pub fn validate(&self) -> Result<(), SignalingError> {
        if serde_json::to_vec(&self.parameters)
            .map_err(|e| {
                SignalingError::new(
                    SignalingErrorKind::Invalid,
                    format!("failed to serialize parameters: {e}"),
                )
            })?
            .len()
            > MAX_PARAMETERS_BYTES
        {
            return Err(SignalingError::new(
                SignalingErrorKind::Invalid,
                "parameters exceed maximum byte size",
            ));
        }
        Ok(())
    }
}

/// REST request body to create a media session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMediaSessionRestDto {
    pub operation_id: OperationId,
    pub parameters: serde_json::Value,
}

impl CreateMediaSessionRestDto {
    /// Validate the request body before it is forwarded to the upstream
    /// signaling system.
    pub fn validate(&self) -> Result<(), SignalingError> {
        if serde_json::to_vec(&self.parameters)
            .map_err(|e| {
                SignalingError::new(
                    SignalingErrorKind::Invalid,
                    format!("failed to serialize parameters: {e}"),
                )
            })?
            .len()
            > MAX_PARAMETERS_BYTES
        {
            return Err(SignalingError::new(
                SignalingErrorKind::Invalid,
                "parameters exceed maximum byte size",
            ));
        }
        Ok(())
    }
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

impl OperationDto {
    /// Validate wire representation returned by a REST adapter.
    pub fn validate(&self) -> Result<(), SignalingError> {
        if self.state.len() > MAX_STATE_STRING_LEN {
            return Err(SignalingError::new(
                SignalingErrorKind::Invalid,
                "state exceeds maximum length",
            ));
        }
        if self.deadline.len() > MAX_DEADLINE_STRING_LEN {
            return Err(SignalingError::new(
                SignalingErrorKind::Invalid,
                "deadline exceeds maximum length",
            ));
        }
        Ok(())
    }
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

impl MediaSessionDto {
    /// Validate wire representation returned by a REST adapter.
    pub fn validate(&self) -> Result<(), SignalingError> {
        if self.state.len() > MAX_STATE_STRING_LEN {
            return Err(SignalingError::new(
                SignalingErrorKind::Invalid,
                "state exceeds maximum length",
            ));
        }
        if self.deadline.len() > MAX_DEADLINE_STRING_LEN {
            return Err(SignalingError::new(
                SignalingErrorKind::Invalid,
                "deadline exceeds maximum length",
            ));
        }
        Ok(())
    }
}
