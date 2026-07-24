//! Mappers between REST DTOs, cheetah Proto payloads, and the platform projection snapshot.

use foundation::{Deadline, TenantId};

use crate::dto::{
    CreateMediaSessionRestDto, CreateOperationRestDto, MediaSessionDto, OperationDto,
    SignalingDeviceDto,
};
use crate::{
    CreateMediaSessionRequest, CreateOperationRequest, MediaSession, MediaSessionState, Operation,
    OperationState, SignalingError, SignalingErrorKind,
};

/// Map a REST create-operation body to the domain request.
pub fn rest_to_create_operation(
    tenant_id: TenantId,
    body: CreateOperationRestDto,
    deadline: Deadline,
) -> Result<CreateOperationRequest, SignalingError> {
    body.validate()?;
    Ok(CreateOperationRequest {
        tenant_id,
        device_id: body.device_id,
        deadline,
        parameters: body.parameters,
    })
}

/// Map a REST create-media-session body to the domain request.
pub fn rest_to_create_media_session(
    tenant_id: TenantId,
    body: CreateMediaSessionRestDto,
    deadline: Deadline,
) -> Result<CreateMediaSessionRequest, SignalingError> {
    body.validate()?;
    Ok(CreateMediaSessionRequest {
        tenant_id,
        operation_id: body.operation_id,
        deadline,
        parameters: body.parameters,
    })
}

/// Map a domain operation to its REST DTO.
pub fn operation_to_rest(op: &Operation) -> OperationDto {
    OperationDto {
        id: op.id,
        tenant_id: op.tenant_id,
        device_id: op.device_id,
        state: state_to_string(op.state),
        deadline: op.deadline.timestamp().to_rfc3339(),
    }
}

/// Map a domain media session to its REST DTO.
pub fn media_session_to_rest(ms: &MediaSession) -> MediaSessionDto {
    MediaSessionDto {
        id: ms.id,
        tenant_id: ms.tenant_id,
        operation_id: ms.operation_id,
        state: media_state_to_string(ms.state),
        deadline: ms.deadline.timestamp().to_rfc3339(),
    }
}

/// Decode a cheetah Proto payload into a `SignalingDeviceDto`.
///
/// Phase 1: upstream proto descriptors are not published, so this always fails with
/// `Unsupported` to avoid guessing at an unpublished wire format.
pub fn proto_to_device(_payload: &[u8]) -> Result<SignalingDeviceDto, SignalingError> {
    Err(SignalingError::new(
        SignalingErrorKind::Unsupported,
        "proto decoding is not enabled until upstream descriptors are published",
    ))
}

/// Map a typed device DTO to the platform projection payload.
pub fn device_to_snapshot_payload(device: &SignalingDeviceDto) -> Result<String, SignalingError> {
    device.validate()?;
    serde_json::to_string(device).map_err(|e| {
        SignalingError::new(
            SignalingErrorKind::Invalid,
            format!("failed to serialize device snapshot: {e}"),
        )
    })
}

fn state_to_string(state: OperationState) -> String {
    match state {
        OperationState::Pending => "pending".to_string(),
        OperationState::Running => "running".to_string(),
        OperationState::Completed => "completed".to_string(),
        OperationState::Failed => "failed".to_string(),
        OperationState::Unknown => "unknown".to_string(),
    }
}

fn media_state_to_string(state: MediaSessionState) -> String {
    match state {
        MediaSessionState::Initial => "initial".to_string(),
        MediaSessionState::Active => "active".to_string(),
        MediaSessionState::Closed => "closed".to_string(),
        MediaSessionState::Failed => "failed".to_string(),
        MediaSessionState::Unknown => "unknown".to_string(),
    }
}
