//! Per-request context extractor that combines request id, authentication, and tenant scope.

use axum::{extract::FromRequestParts, http::request::Parts};
use foundation::{MessageId, RequestContext, TenantId};

use crate::auth::optional_auth;
use crate::error::AppError;

/// Request context built by the HTTP layer and passed to application use cases.
#[derive(Debug, Clone)]
pub struct ApiRequestContext(pub RequestContext);

impl<S> FromRequestParts<S> for ApiRequestContext
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let request_id = request_id_from_parts(parts);
        let tenant = tenant_from_path(parts.uri.path());
        let auth = optional_auth(parts).await?;

        match (tenant, auth.as_ref()) {
            (Some(tenant_id), Some(auth_ctx)) if auth_ctx.tenant_id != tenant_id => {
                return Err(AppError::Denied);
            }
            (Some(_), None) => return Err(AppError::Unauthenticated),
            _ => {}
        }

        let mut ctx = RequestContext::default();
        if let Some(id) = request_id {
            ctx = ctx.with_request_id(id);
        }
        if let Some(auth) = auth {
            ctx = ctx.with_actor(auth.user_id).with_tenant(auth.tenant_id);
        } else if let Some(tenant_id) = tenant {
            ctx = ctx.with_tenant(tenant_id);
        }

        Ok(Self(ctx))
    }
}

fn request_id_from_parts(parts: &Parts) -> Option<MessageId> {
    parts
        .extensions
        .get::<tower_http::request_id::RequestId>()
        .and_then(|request_id| request_id.header_value().to_str().ok())
        .and_then(|text| MessageId::parse_str(text).ok())
}

/// Parse the tenant id from the first `/tenants/<uuid>` segment, if any.
fn tenant_from_path(path: &str) -> Option<TenantId> {
    let prefix = "/tenants/";
    let rest = path.strip_prefix(prefix)?;
    let segment = rest.split('/').next()?;
    TenantId::parse_str(segment).ok()
}
