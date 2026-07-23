//! Single-node REST + SSE adapter and JetStream projection consumer for
//! cheetah-signaling.
//!
//! Phase 1 freezes the event shapes and the `SignalingPort` adapter, but leaves the
//! actual HTTP/NATS clients unconfigured so the crate returns `Unavailable` when no
//! upstream base URL is supplied and `Unsupported` when one is supplied.

pub mod event;
pub mod jetstream;
pub mod reconciler;
pub mod worker;

use async_trait::async_trait;
use domain_signaling::{
    CreateMediaSessionRequest, CreateOperationRequest, MediaSession, Operation, SignalingError,
    SignalingErrorKind, SignalingPort, dto::SignalingDeviceDto,
};
use foundation::{Deadline, DeviceId, OperationId, TenantId};

/// REST + SSE signaling adapter.
#[derive(Debug, Clone, Default)]
pub struct RestSignalingAdapter {
    /// Upstream base URL. When `None`, every call returns `Unavailable`.
    base_url: Option<String>,
}

impl RestSignalingAdapter {
    /// Create a new adapter. `base_url` is optional until an upstream is configured.
    pub fn new(base_url: Option<String>) -> Self {
        Self { base_url }
    }

    fn unavailable() -> Result<SignalingDeviceDto, SignalingError> {
        Err(SignalingError::new(
            SignalingErrorKind::Unavailable,
            "signaling upstream is not configured",
        ))
    }

    fn unsupported() -> Result<SignalingDeviceDto, SignalingError> {
        Err(SignalingError::new(
            SignalingErrorKind::Unsupported,
            "REST signaling transport is not implemented in this build",
        ))
    }
}

#[async_trait]
impl SignalingPort for RestSignalingAdapter {
    async fn get_device(
        &self,
        _tenant_id: TenantId,
        _device_id: DeviceId,
        _deadline: Deadline,
    ) -> Result<SignalingDeviceDto, SignalingError> {
        match self.base_url {
            Some(_) => Self::unsupported(),
            None => Self::unavailable(),
        }
    }

    async fn create_operation(
        &self,
        _request: CreateOperationRequest,
    ) -> Result<Operation, SignalingError> {
        match self.base_url {
            Some(_) => Err(SignalingError::new(
                SignalingErrorKind::Unsupported,
                "create_operation is not implemented in this build",
            )),
            None => Err(SignalingError::new(
                SignalingErrorKind::Unavailable,
                "signaling upstream is not configured",
            )),
        }
    }

    async fn create_media_session(
        &self,
        _request: CreateMediaSessionRequest,
    ) -> Result<MediaSession, SignalingError> {
        match self.base_url {
            Some(_) => Err(SignalingError::new(
                SignalingErrorKind::Unsupported,
                "create_media_session is not implemented in this build",
            )),
            None => Err(SignalingError::new(
                SignalingErrorKind::Unavailable,
                "signaling upstream is not configured",
            )),
        }
    }

    async fn get_operation(
        &self,
        _tenant_id: TenantId,
        _operation_id: OperationId,
        _deadline: Deadline,
    ) -> Result<Operation, SignalingError> {
        match self.base_url {
            Some(_) => Err(SignalingError::new(
                SignalingErrorKind::Unsupported,
                "get_operation is not implemented in this build",
            )),
            None => Err(SignalingError::new(
                SignalingErrorKind::Unavailable,
                "signaling upstream is not configured",
            )),
        }
    }
}
