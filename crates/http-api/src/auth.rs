//! HTTP authentication extractor and helpers.

use application::auth::{AuthContext, Authenticator};
use async_trait::async_trait;
use axum::{
    extract::FromRequestParts,
    http::{StatusCode, header, request::Parts},
};
use foundation::{ErrorCode, PlatformError};
use std::sync::Arc;

use crate::error::AppError;

/// Maximum byte length of a Bearer token carried in the `Authorization` header.
const MAX_BEARER_TOKEN_BYTES: usize = 8192;

/// Authenticated actor extracted from an `Authorization: Bearer <token>` header.
#[derive(Debug, Clone)]
pub struct Auth(pub AuthContext);

impl<S> FromRequestParts<S> for Auth
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let token = extract_bearer(&parts.headers)?;
        let authenticator = authenticator(parts)?;
        let context = authenticator
            .authenticate(&token)
            .await
            .map_err(AppError::from)?;
        Ok(Self(context))
    }
}

/// Extract a bearer token from the `Authorization` header.
pub fn extract_bearer(headers: &axum::http::HeaderMap) -> Result<String, AppError> {
    let header = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(AppError::Unauthenticated)?;
    let mut parts = header.split_whitespace();
    let scheme = parts.next().ok_or(AppError::Unauthenticated)?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return Err(AppError::Unauthenticated);
    }
    let token = parts.next().ok_or(AppError::Unauthenticated)?;
    if parts.next().is_some() || token.is_empty() {
        return Err(AppError::Unauthenticated);
    }
    if token.len() > MAX_BEARER_TOKEN_BYTES {
        return Err(AppError::BadRequest {
            field: "authorization".to_string(),
            message: "bearer token is too long".to_string(),
        });
    }
    Ok(token.to_string())
}

/// Look up the shared `Authenticator` extension.
pub fn authenticator(parts: &Parts) -> Result<Arc<dyn Authenticator>, AppError> {
    parts
        .extensions
        .get::<Arc<dyn Authenticator>>()
        .cloned()
        .ok_or(AppError::Internal)
}

/// Optional authentication: returns `Some(AuthContext)` when a valid bearer token is present.
pub async fn optional_auth(parts: &Parts) -> Result<Option<AuthContext>, AppError> {
    let token = match extract_bearer(&parts.headers) {
        Ok(token) => token,
        Err(_) => return Ok(None),
    };
    let authenticator = authenticator(parts)?;
    authenticator
        .authenticate(&token)
        .await
        .map(Some)
        .map_err(AppError::from)
}

/// Build an HTTP 401 `WWW-Authenticate` response for missing credentials.
pub fn www_authenticate() -> (StatusCode, [(&'static str, &'static str); 1], ()) {
    (
        StatusCode::UNAUTHORIZED,
        [("WWW-Authenticate", "Bearer")],
        (),
    )
}

/// Fallback authenticator that rejects every token.
///
/// Useful as a safe default in binaries that have not yet wired a real
/// `TokenAuthenticator`; it prevents `Auth`/`ApiRequestContext` from leaking
/// an internal server error when a client sends an `Authorization` header.
#[derive(Debug, Clone)]
pub struct DenyAllAuthenticator;

#[async_trait]
impl Authenticator for DenyAllAuthenticator {
    async fn authenticate(&self, _token: &str) -> Result<AuthContext, PlatformError> {
        Err(PlatformError::new(
            ErrorCode::Unauthenticated,
            "invalid token",
        ))
    }
}
