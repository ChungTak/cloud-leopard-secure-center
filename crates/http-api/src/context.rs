//! Per-request context extractor that combines request id, authentication, and tenant scope.

use axum::{
    extract::FromRequestParts,
    http::{HeaderMap, request::Parts},
};
use foundation::{MessageId, OrganizationId, RequestContext, TenantId};

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
        let tenant = tenant_from_path(parts.uri.path())?;
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
        if let Some(id) = header_message_id(&parts.headers, "x-correlation-id") {
            ctx = ctx.with_correlation_id(id);
        }
        if let Some(id) = header_str(&parts.headers, "x-trace-id") {
            ctx = ctx.with_trace_id(id)?;
        }
        if let Some(id) = header_organization_id(&parts.headers, "x-organization-id") {
            ctx = ctx.with_organization_id(id);
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
        .or_else(|| header_message_id(&parts.headers, "x-request-id"))
}

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .filter(|text| !text.is_empty())
}

fn header_message_id(headers: &HeaderMap, name: &str) -> Option<MessageId> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .and_then(|text| MessageId::parse_str(text).ok())
}

fn header_organization_id(headers: &HeaderMap, name: &str) -> Option<OrganizationId> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .and_then(|text| OrganizationId::parse_str(text).ok())
}

/// Parse the tenant id from the first `/tenants/<uuid>` segment, if any.
/// Returns `Ok(None)` when no tenant segment is present, and `Err` when a
/// tenant segment contains an invalid UUID so that malformed paths fail early.
fn tenant_from_path(path: &str) -> Result<Option<TenantId>, AppError> {
    let mut segments = path.split('/');
    while let Some(segment) = segments.next() {
        if segment == "tenants" {
            let Some(raw) = segments.next() else {
                return Ok(None);
            };
            return TenantId::parse_str(raw)
                .map(Some)
                .map_err(|_| AppError::BadRequest {
                    field: "tenant_id".to_string(),
                    message: "tenant id in path is not a valid uuid".to_string(),
                });
        }
    }
    Ok(None)
}
