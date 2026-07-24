//! Media entitlement domain: playback authorization and player policy.
//!
//! Phase 1 leaves the upstream media session state machine as `UNSUPPORTED`;
//! the entitlement aggregate and port are frozen so adapters can be added later.

use foundation::{CameraId, Deadline, EntitlementId, OperationId, TenantId, UtcTimestamp};
use serde::{Deserialize, Serialize};

const MAX_SESSION_ID_LEN: usize = 256;
const MAX_PROTOCOL_LEN: usize = 64;

/// Errors that can occur in media entitlement processing.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error("{kind:?}: {message}")]
pub struct MediaError {
    pub kind: MediaErrorKind,
    pub message: String,
}

impl MediaError {
    pub fn new(kind: MediaErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum MediaErrorKind {
    Unsupported,
    Unavailable,
    UnknownOutcome,
    Timeout,
    Invalid,
    Unauthorized,
    Denied,
}

/// Allowed media actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum MediaAction {
    Live,
    Playback,
    Download,
    Ptz,
}

/// A token-scoped media session identifier that does not expose the upstream URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaSession {
    pub tenant_id: TenantId,
    pub principal_id: Option<foundation::UserId>,
    pub camera_id: CameraId,
    pub session_id: String,
    pub protocol: String,
}

impl MediaSession {
    pub fn new(
        tenant_id: TenantId,
        principal_id: Option<foundation::UserId>,
        camera_id: CameraId,
        session_id: impl AsRef<str>,
        protocol: impl AsRef<str>,
    ) -> Result<Self, MediaError> {
        let session_id = session_id.as_ref();
        validate_media_string(session_id, "session_id", MAX_SESSION_ID_LEN)?;
        let protocol = protocol.as_ref();
        validate_media_string(protocol, "protocol", MAX_PROTOCOL_LEN)?;
        Ok(Self {
            tenant_id,
            principal_id,
            camera_id,
            session_id: session_id.to_string(),
            protocol: protocol.to_string(),
        })
    }
}

/// Token binding for a media session. The token is opaque and the URL is not
/// logged or serialized to clients.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaToken {
    pub tenant_id: TenantId,
    pub principal_id: Option<foundation::UserId>,
    pub camera_id: CameraId,
    pub session_id: String,
    pub protocol: String,
    pub expires_at: UtcTimestamp,
}

impl MediaToken {
    pub fn new(
        tenant_id: TenantId,
        principal_id: Option<foundation::UserId>,
        camera_id: CameraId,
        session_id: impl AsRef<str>,
        protocol: impl AsRef<str>,
        expires_at: UtcTimestamp,
    ) -> Result<Self, MediaError> {
        let session_id = session_id.as_ref();
        validate_media_string(session_id, "session_id", MAX_SESSION_ID_LEN)?;
        let protocol = protocol.as_ref();
        validate_media_string(protocol, "protocol", MAX_PROTOCOL_LEN)?;
        Ok(Self {
            tenant_id,
            principal_id,
            camera_id,
            session_id: session_id.to_string(),
            protocol: protocol.to_string(),
            expires_at,
        })
    }
}

/// Player policy delivered to the client with an entitlement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PlayerPolicy {
    pub autoplay: bool,
    pub controls: bool,
    pub muted: bool,
    pub allowed_actions: Vec<MediaAction>,
}

/// A playback entitlement granted to a principal for a camera.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackEntitlement {
    pub id: EntitlementId,
    pub tenant_id: TenantId,
    pub camera_id: CameraId,
    pub actions: Vec<MediaAction>,
    pub operation_id: OperationId,
    pub session: Option<MediaSession>,
    pub main_source: Option<String>,
    pub sub_source: Option<String>,
    pub player_policy: PlayerPolicy,
    pub token: MediaToken,
    pub expires_at: UtcTimestamp,
    pub revoked_at: Option<UtcTimestamp>,
}

/// Request to create a playback entitlement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateEntitlementRequest {
    pub tenant_id: TenantId,
    pub principal_id: foundation::UserId,
    pub camera_id: CameraId,
    pub actions: Vec<MediaAction>,
    pub protocol: String,
    pub deadline: Deadline,
}

impl CreateEntitlementRequest {
    /// Create a validated entitlement request.
    pub fn new(
        tenant_id: TenantId,
        principal_id: foundation::UserId,
        camera_id: CameraId,
        actions: Vec<MediaAction>,
        protocol: impl AsRef<str>,
        deadline: Deadline,
    ) -> Result<Self, MediaError> {
        let protocol = protocol.as_ref();
        validate_media_string(protocol, "protocol", MAX_PROTOCOL_LEN)?;
        Ok(Self {
            tenant_id,
            principal_id,
            camera_id,
            actions,
            protocol: protocol.to_string(),
            deadline,
        })
    }
}

fn validate_media_string(value: &str, field: &str, max: usize) -> Result<(), MediaError> {
    if value.trim().is_empty() || value.len() > max {
        return Err(MediaError::new(
            MediaErrorKind::Invalid,
            format!("{field} is empty or exceeds maximum length"),
        ));
    }
    Ok(())
}

/// Port for media entitlement operations.
#[async_trait::async_trait]
pub trait MediaPort: Send + Sync {
    /// Create an entitlement, starting a signaling operation if needed.
    async fn create_entitlement(
        &self,
        request: CreateEntitlementRequest,
    ) -> Result<PlaybackEntitlement, MediaError>;

    /// Get an existing entitlement by id.
    async fn get_entitlement(
        &self,
        tenant_id: TenantId,
        entitlement_id: EntitlementId,
    ) -> Result<PlaybackEntitlement, MediaError>;

    /// Revoke an entitlement.
    async fn revoke_entitlement(
        &self,
        tenant_id: TenantId,
        entitlement_id: EntitlementId,
    ) -> Result<PlaybackEntitlement, MediaError>;
}

/// Stub media port that always returns `Unsupported`.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnsupportedMediaPort;

#[async_trait::async_trait]
impl MediaPort for UnsupportedMediaPort {
    async fn create_entitlement(
        &self,
        _request: CreateEntitlementRequest,
    ) -> Result<PlaybackEntitlement, MediaError> {
        Err(MediaError::new(
            MediaErrorKind::Unsupported,
            "media entitlement creation is not enabled in this build",
        ))
    }

    async fn get_entitlement(
        &self,
        _tenant_id: TenantId,
        _entitlement_id: EntitlementId,
    ) -> Result<PlaybackEntitlement, MediaError> {
        Err(MediaError::new(
            MediaErrorKind::Unsupported,
            "media entitlement retrieval is not enabled in this build",
        ))
    }

    async fn revoke_entitlement(
        &self,
        _tenant_id: TenantId,
        _entitlement_id: EntitlementId,
    ) -> Result<PlaybackEntitlement, MediaError> {
        Err(MediaError::new(
            MediaErrorKind::Unsupported,
            "media entitlement revocation is not enabled in this build",
        ))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use futures::executor::block_on;

    use super::*;
    use foundation::chrono::{DateTime, Duration, Utc};
    use foundation::{
        CameraId, Clock, SystemClock, SystemIdGenerator, SystemRandom, TenantId, UserId,
        UtcTimestamp,
    };

    #[test]
    fn unsupported_create_entitlement_returns_unsupported() {
        let port = UnsupportedMediaPort;
        let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
        let now: DateTime<Utc> = SystemClock.now().into();
        let request = CreateEntitlementRequest {
            tenant_id: TenantId::generate(&generator).expect("generate tenant id"),
            principal_id: UserId::generate(&generator).expect("generate user id"),
            camera_id: CameraId::generate(&generator).expect("generate camera id"),
            actions: vec![MediaAction::Live],
            protocol: "webrtc".to_string(),
            deadline: Deadline::new(UtcTimestamp::from(now + Duration::seconds(30))),
        };
        match block_on(port.create_entitlement(request)) {
            Ok(_) => panic!("expected unsupported"),
            Err(e) => assert_eq!(e.kind, MediaErrorKind::Unsupported),
        }
    }
}
