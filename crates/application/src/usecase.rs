//! Application use-case template and shared primitives.
//!
//! All write use cases follow the same ordered flow:
//!
//! 1. Validate the request.
//! 2. Resolve the tenant / actor from the request context.
//! 3. Authorize the action against the authorization port.
//! 4. Load the aggregate (for updates).
//! 5. Apply the domain mutation.
//! 6. Persist the aggregate, outbox message, and audit record inside a unit
//!    of work.
//! 7. Return a stable DTO.

use async_trait::async_trait;
use domain_audit::audit_record::{ActionRisk, AuditDetails, AuditRecord, AuditResult};
use domain_authorization::role_binding::ResourceRef;
use foundation::{Clock, ErrorCode, PlatformError, RequestContext, Revision, TenantId, UserId, uuid::Uuid};

pub use crate::authorization::{
    AuthorizationPort, AuthorizationRequest, AuthorizationResponse, Decision,
};

/// Context supplied by the caller to make an operation idempotent.
#[derive(Debug, Clone)]
pub struct IdempotencyContext {
    /// Client-provided idempotency key.
    pub key: String,
    /// Endpoint or operation scope the key belongs to.
    pub endpoint: String,
    /// Digest of the request payload.
    pub digest: String,
}

/// Wraps a write command with the optimistic-lock and idempotency metadata
/// required by every mutating application use case.
#[derive(Debug, Clone)]
pub struct WriteRequest<T> {
    /// Expected aggregate revision; required for updates and optional for
    /// creates (ignored on create).
    pub expected_revision: Option<Revision>,
    /// Optional idempotency context supplied by the caller.
    pub idempotency: Option<IdempotencyContext>,
    /// Domain-specific command payload.
    pub payload: T,
}

impl<T> WriteRequest<T> {
    /// Create a new write request for an update command.
    pub fn for_update(expected: Revision, payload: T) -> Self {
        Self {
            expected_revision: Some(expected),
            idempotency: None,
            payload,
        }
    }

    /// Create a new write request for a create command.
    pub fn for_create(payload: T) -> Self {
        Self {
            expected_revision: None,
            idempotency: None,
            payload,
        }
    }

    /// Attach an idempotency context.
    pub fn with_idempotency(mut self, idempotency: IdempotencyContext) -> Self {
        self.idempotency = Some(idempotency);
        self
    }
}

/// Result of a mutating use case.
#[derive(Debug, Clone)]
pub struct WriteResponse<T> {
    /// Stable DTO returned to the caller.
    pub data: T,
    /// Aggregate revision after the write.
    pub revision: Revision,
}

impl<T> WriteResponse<T> {
    /// Create a new write response.
    pub fn new(data: T, revision: Revision) -> Self {
        Self { data, revision }
    }
}

/// Marker trait for application use cases.
///
/// Concrete use cases implement this trait with domain-specific request and
/// response types.
#[async_trait]
pub trait UseCase: Send + Sync {
    /// Input request type.
    type Request;
    /// Output response type.
    type Response;

    /// Execute the use case.
    async fn execute(
        &self,
        request: &Self::Request,
        ctx: &RequestContext,
    ) -> Result<Self::Response, PlatformError>;
}

/// Build an authorization request for a platform-scoped action.
pub fn platform_authorization(principal: UserId, action: &'static str) -> AuthorizationRequest {
    AuthorizationRequest {
        principal,
        tenant: TenantId::from_uuid(Uuid::nil()),
        action: action.to_string(),
        resource: ResourceRef::User(principal),
        context: None,
    }
}

/// Build an authorization request for a tenant-scoped action on a concrete
/// resource.
pub fn tenant_authorization(
    principal: UserId,
    tenant: foundation::TenantId,
    action: &'static str,
    resource: ResourceRef,
) -> AuthorizationRequest {
    AuthorizationRequest {
        principal,
        tenant,
        action: action.to_string(),
        resource,
        context: None,
    }
}

/// Require a tenant id from the request context.
pub fn require_tenant(ctx: &RequestContext) -> Result<foundation::TenantId, PlatformError> {
    ctx.tenant_id.ok_or_else(|| {
        PlatformError::new(
            ErrorCode::Unauthenticated,
            "tenant context is required".to_string(),
        )
    })
}

/// Require an actor from the request context.
pub fn require_actor(ctx: &RequestContext) -> Result<UserId, PlatformError> {
    ctx.actor_id.ok_or_else(|| {
        PlatformError::new(ErrorCode::Unauthenticated, "actor is required".to_string())
    })
}

/// Authorize an action and return a denied error if the decision is not allow.
pub async fn authorize_or_fail(
    auth: &dyn AuthorizationPort,
    req: AuthorizationRequest,
    ctx: &RequestContext,
) -> Result<(), PlatformError> {
    let resp = auth.authorize(req, ctx).await?;
    if resp.decision != Decision::Allow {
        return Err(PlatformError::new(
            ErrorCode::Denied,
            "authorization denied".to_string(),
        ));
    }
    Ok(())
}

/// Helper to build and write an audit record for a successful write.
#[allow(clippy::too_many_arguments)]
pub async fn audit_write(
    audit: &dyn storage_api::AuditWriter,
    tenant_id: foundation::TenantId,
    actor_type: &'static str,
    actor_id: String,
    action: &'static str,
    target_type: &'static str,
    target_id: String,
    risk: ActionRisk,
    details: serde_json::Value,
    clock: &dyn Clock,
    ctx: &RequestContext,
) -> Result<(), PlatformError> {
    let details = AuditDetails::new(action, details.to_string())
        .map_err(|e| PlatformError::invalid("audit_details", e.to_string()))?;
    let mut record = AuditRecord::new(
        tenant_id,
        actor_type,
        actor_id,
        action,
        target_type,
        target_id,
        AuditResult::Success,
        risk,
        details,
        clock,
    )
    .map_err(|e| PlatformError::invalid("audit_record", e.to_string()))?;
    if let Some(request_id) = ctx.request_id {
        record = record.with_request_id(request_id.to_string());
    }
    if let Some(trace_id) = &ctx.trace_id {
        record = record.with_trace_id(trace_id.clone());
    }
    audit.write(&record, ctx).await?;
    Ok(())
}

/// Check whether the operation has exceeded its deadline.
pub fn check_deadline(ctx: &RequestContext, clock: &dyn Clock) -> Result<(), PlatformError> {
    if let Some(deadline) = ctx.deadline
        && deadline.is_expired(clock)
    {
        return Err(PlatformError::new(
            ErrorCode::Cancelled,
            "request deadline exceeded".to_string(),
        ));
    }
    Ok(())
}
