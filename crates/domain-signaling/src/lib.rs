//! Signaling domain: contract, typed identifiers, and port for cheetah-signaling integration.
//!
//! Phase 1 leaves the actual transport implementation as `UNSUPPORTED`; the port and
//! error taxonomy are frozen so adapters can be plugged in later without changing callers.

use foundation::{Deadline, DeviceId, MediaSessionId, OperationId, TenantId};
use serde::{Deserialize, Serialize};

pub mod dto;
pub mod mapper;

#[cfg(test)]
mod tests;

/// Errors that can occur when interacting with an upstream signaling system.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error("{kind:?}: {message}")]
pub struct SignalingError {
    pub kind: SignalingErrorKind,
    pub message: String,
}

impl SignalingError {
    /// Create a new signaling error.
    pub fn new(kind: SignalingErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SignalingErrorKind {
    /// The capability is not implemented in this build.
    Unsupported,
    /// The upstream service is unreachable or unavailable.
    Unavailable,
    /// The outcome of an operation could not be determined.
    UnknownOutcome,
    /// The request deadline was exceeded.
    Timeout,
    /// The request was rejected as invalid.
    Invalid,
    /// The caller is not authorized.
    Unauthorized,
}

/// Request to create a signaling operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateOperationRequest {
    pub tenant_id: TenantId,
    pub device_id: DeviceId,
    pub deadline: Deadline,
    pub parameters: serde_json::Value,
}

/// Request to create a media session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateMediaSessionRequest {
    pub tenant_id: TenantId,
    pub operation_id: OperationId,
    pub deadline: Deadline,
    pub parameters: serde_json::Value,
}

/// An upstream operation handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    pub id: OperationId,
    pub tenant_id: TenantId,
    pub device_id: DeviceId,
    pub state: OperationState,
    pub deadline: Deadline,
}

/// An upstream media session handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaSession {
    pub id: MediaSessionId,
    pub tenant_id: TenantId,
    pub operation_id: OperationId,
    pub state: MediaSessionState,
    pub deadline: Deadline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum OperationState {
    Pending,
    Running,
    Completed,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum MediaSessionState {
    Initial,
    Active,
    Closed,
    Failed,
    Unknown,
}

/// Port abstraction for upstream signaling integration.
#[async_trait::async_trait]
pub trait SignalingPort: Send + Sync {
    /// Get a device projection from the upstream system.
    async fn get_device(
        &self,
        tenant_id: TenantId,
        device_id: DeviceId,
        deadline: Deadline,
    ) -> Result<dto::SignalingDeviceDto, SignalingError>;

    /// Create an operation on the upstream system.
    async fn create_operation(
        &self,
        request: CreateOperationRequest,
    ) -> Result<Operation, SignalingError>;

    /// Create a media session on the upstream system.
    async fn create_media_session(
        &self,
        request: CreateMediaSessionRequest,
    ) -> Result<MediaSession, SignalingError>;

    /// Get an operation by id.
    async fn get_operation(
        &self,
        tenant_id: TenantId,
        operation_id: OperationId,
        deadline: Deadline,
    ) -> Result<Operation, SignalingError>;
}

/// A stub port that always returns `UNSUPPORTED`.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnsupportedSignalingPort;

#[async_trait::async_trait]
impl SignalingPort for UnsupportedSignalingPort {
    async fn get_device(
        &self,
        _tenant_id: TenantId,
        _device_id: DeviceId,
        _deadline: Deadline,
    ) -> Result<dto::SignalingDeviceDto, SignalingError> {
        Err(SignalingError::new(
            SignalingErrorKind::Unsupported,
            "signaling integration is not enabled in this build",
        ))
    }

    async fn create_operation(
        &self,
        _request: CreateOperationRequest,
    ) -> Result<Operation, SignalingError> {
        Err(SignalingError::new(
            SignalingErrorKind::Unsupported,
            "create_operation is not supported in this build",
        ))
    }

    async fn create_media_session(
        &self,
        _request: CreateMediaSessionRequest,
    ) -> Result<MediaSession, SignalingError> {
        Err(SignalingError::new(
            SignalingErrorKind::Unsupported,
            "create_media_session is not supported in this build",
        ))
    }

    async fn get_operation(
        &self,
        _tenant_id: TenantId,
        _operation_id: OperationId,
        _deadline: Deadline,
    ) -> Result<Operation, SignalingError> {
        Err(SignalingError::new(
            SignalingErrorKind::Unsupported,
            "get_operation is not supported in this build",
        ))
    }
}
