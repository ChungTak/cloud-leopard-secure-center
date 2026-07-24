//! Media entitlement application use cases.

use async_trait::async_trait;
use domain_authorization::role_binding::ResourceRef;
use domain_media::{
    CreateEntitlementRequest as DomainCreateRequest, MediaAction, MediaError, MediaErrorKind,
    MediaPort, PlaybackEntitlement, PlayerPolicy,
};
use foundation::{
    CameraId, Clock, Deadline, EntitlementId, PlatformError, RequestContext, TenantId, UserId,
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
    pub main_source: Option<String>,
    pub sub_source: Option<String>,
    pub player_policy: PlayerPolicy,
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
            session_id: e.session.as_ref().map(|s| s.session_id.clone()),
            main_source: e.main_source.clone(),
            sub_source: e.sub_source.clone(),
            player_policy: e.player_policy.clone(),
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
    pub protocol: String,
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
pub struct MediaService<M, A, C> {
    port: M,
    auth: A,
    clock: C,
}

impl<M, A, C: Clock> MediaService<M, A, C> {
    /// Create a new media service.
    pub fn new(port: M, auth: A, clock: C) -> Self {
        Self { port, auth, clock }
    }

    fn build_request(
        tenant_id: TenantId,
        principal_id: UserId,
        camera_id: CameraId,
        actions: Vec<MediaAction>,
        protocol: String,
        ctx: &RequestContext,
        clock: &dyn Clock,
    ) -> DomainCreateRequest {
        DomainCreateRequest {
            tenant_id,
            principal_id,
            camera_id,
            actions,
            protocol,
            deadline: ctx.deadline.unwrap_or_else(|| {
                let now: chrono::DateTime<chrono::Utc> = clock.now().into();
                Deadline::new(UtcTimestamp::from(now + chrono::Duration::seconds(30)))
            }),
        }
    }
}

#[async_trait]
impl<M, A, C: Clock> MediaUseCase for MediaService<M, A, C>
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
                actor,
                request.camera_id,
                request.actions,
                request.protocol,
                ctx,
                &self.clock,
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
