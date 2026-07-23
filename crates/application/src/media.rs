//! Media entitlement application use cases.

use async_trait::async_trait;
use domain_authorization::role_binding::ResourceRef;
use domain_media::{
    CreateEntitlementRequest as DomainCreateRequest, MediaAction, MediaError, MediaErrorKind,
    MediaPort, PlaybackEntitlement,
};
use foundation::{
    CameraId, Deadline, EntitlementId, PlatformError, RequestContext, TenantId, UserId,
    UtcTimestamp, chrono,
};

use crate::authorization::{AuthorizationPort, AuthorizationRequest};

/// Stable DTO for a playback entitlement.
#[derive(Debug, Clone)]
pub struct PlaybackEntitlementDto {
    pub id: EntitlementId,
    pub tenant_id: TenantId,
    pub camera_id: CameraId,
    pub actions: Vec<MediaAction>,
    pub session_id: Option<String>,
    pub expires_at: String,
    pub revoked_at: Option<String>,
}

impl From<&PlaybackEntitlement> for PlaybackEntitlementDto {
    fn from(e: &PlaybackEntitlement) -> Self {
        Self {
            id: e.id,
            tenant_id: e.tenant_id,
            camera_id: e.camera_id,
            actions: e.actions.clone(),
            session_id: e.session_id.clone(),
            expires_at: e.expires_at.to_rfc3339(),
            revoked_at: e.revoked_at.map(|t| t.to_rfc3339()),
        }
    }
}

/// Request to create a playback entitlement.
#[derive(Debug, Clone)]
pub struct CreateEntitlementRequest {
    pub tenant_id: TenantId,
    pub camera_id: CameraId,
    pub actions: Vec<MediaAction>,
}

/// Port for media entitlement use cases.
#[async_trait]
pub trait MediaUseCase: Send + Sync {
    /// Create a playback entitlement.
    async fn create(
        &self,
        request: CreateEntitlementRequest,
        ctx: &RequestContext,
    ) -> Result<PlaybackEntitlementDto, PlatformError>;

    /// Get a playback entitlement.
    async fn get(
        &self,
        tenant_id: TenantId,
        entitlement_id: EntitlementId,
        ctx: &RequestContext,
    ) -> Result<PlaybackEntitlementDto, PlatformError>;

    /// Revoke a playback entitlement.
    async fn revoke(
        &self,
        tenant_id: TenantId,
        entitlement_id: EntitlementId,
        ctx: &RequestContext,
    ) -> Result<PlaybackEntitlementDto, PlatformError>;
}

/// Default media entitlement application service.
#[derive(Debug, Clone)]
pub struct MediaService<M, A> {
    port: M,
    auth: A,
}

impl<M, A> MediaService<M, A> {
    /// Create a new media service.
    pub fn new(port: M, auth: A) -> Self {
        Self { port, auth }
    }

    fn build_request(
        tenant_id: TenantId,
        camera_id: CameraId,
        actions: Vec<MediaAction>,
        ctx: &RequestContext,
    ) -> DomainCreateRequest {
        DomainCreateRequest {
            tenant_id,
            camera_id,
            actions,
            deadline: ctx.deadline.unwrap_or_else(|| {
                Deadline::new(UtcTimestamp::from(
                    chrono::Utc::now() + chrono::Duration::seconds(30),
                ))
            }),
        }
    }
}

#[async_trait]
impl<M, A> MediaUseCase for MediaService<M, A>
where
    M: MediaPort,
    A: AuthorizationPort,
{
    async fn create(
        &self,
        request: CreateEntitlementRequest,
        ctx: &RequestContext,
    ) -> Result<PlaybackEntitlementDto, PlatformError> {
        let actor = require_actor(ctx)?;
        let auth_req = AuthorizationRequest {
            principal: actor,
            tenant: request.tenant_id,
            action: "media:entitlement:create".to_string(),
            resource: ResourceRef::Camera(request.camera_id),
            context: None,
        };
        crate::usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        let entitlement = self
            .port
            .create_entitlement(Self::build_request(
                request.tenant_id,
                request.camera_id,
                request.actions,
                ctx,
            ))
            .await
            .map_err(map_media_error)?;

        Ok(PlaybackEntitlementDto::from(&entitlement))
    }

    async fn get(
        &self,
        tenant_id: TenantId,
        entitlement_id: EntitlementId,
        ctx: &RequestContext,
    ) -> Result<PlaybackEntitlementDto, PlatformError> {
        let actor = require_actor(ctx)?;
        let auth_req = AuthorizationRequest {
            principal: actor,
            tenant: tenant_id,
            action: "media:entitlement:read".to_string(),
            resource: ResourceRef::User(actor),
            context: None,
        };
        crate::usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        let entitlement = self
            .port
            .get_entitlement(tenant_id, entitlement_id)
            .await
            .map_err(map_media_error)?;

        Ok(PlaybackEntitlementDto::from(&entitlement))
    }

    async fn revoke(
        &self,
        tenant_id: TenantId,
        entitlement_id: EntitlementId,
        ctx: &RequestContext,
    ) -> Result<PlaybackEntitlementDto, PlatformError> {
        let actor = require_actor(ctx)?;
        let auth_req = AuthorizationRequest {
            principal: actor,
            tenant: tenant_id,
            action: "media:entitlement:revoke".to_string(),
            resource: ResourceRef::User(actor),
            context: None,
        };
        crate::usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        let entitlement = self
            .port
            .revoke_entitlement(tenant_id, entitlement_id)
            .await
            .map_err(map_media_error)?;

        Ok(PlaybackEntitlementDto::from(&entitlement))
    }
}

fn require_actor(ctx: &RequestContext) -> Result<UserId, PlatformError> {
    ctx.actor_id.ok_or(PlatformError::Unauthenticated)
}

fn map_media_error(e: MediaError) -> PlatformError {
    match e.kind {
        MediaErrorKind::Unsupported => PlatformError::Unsupported,
        MediaErrorKind::Unavailable => PlatformError::Unavailable,
        MediaErrorKind::UnknownOutcome => PlatformError::UnknownOutcome,
        MediaErrorKind::Timeout => PlatformError::Timeout,
        MediaErrorKind::Invalid => PlatformError::invalid("media", e.message),
        MediaErrorKind::Unauthorized => PlatformError::Unauthenticated,
        MediaErrorKind::Denied => PlatformError::Denied,
        _ => PlatformError::Internal,
    }
}
