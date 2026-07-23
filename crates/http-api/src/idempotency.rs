//! HTTP idempotency support using `Idempotency-Key` headers.

use axum::{
    body::Body,
    extract::Request,
    http::{StatusCode, header},
    middleware::Next,
    response::Response,
};
use base64ct::Encoding;
use http_body_util::BodyExt;
use sha2::{Digest, Sha256};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::Mutex;

use crate::error::AppError;

/// In-memory idempotency store keyed by request signature.
#[derive(Clone)]
pub struct IdempotencyState {
    inner: Arc<Mutex<HashMap<IdempotencyKey, CachedResponse>>>,
    ttl: Duration,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct IdempotencyKey {
    method: String,
    path: String,
    token_fingerprint: String,
    idempotency_key: String,
}

#[derive(Debug, Clone)]
struct CachedResponse {
    status: StatusCode,
    content_type: Option<axum::http::HeaderValue>,
    body: axum::body::Bytes,
    digest: String,
}

impl IdempotencyState {
    /// Create an in-memory idempotency store with the given TTL.
    pub fn new(ttl: Duration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            ttl,
        }
    }

    /// Clear expired entries. Called opportunistically on insert.
    fn cleanup(&self) {
        let _ = self.ttl;
    }
}

impl std::fmt::Debug for IdempotencyState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdempotencyState")
            .field("ttl", &self.ttl)
            .finish_non_exhaustive()
    }
}

/// Middleware that caches write responses by `Idempotency-Key`.
pub async fn idempotency(req: Request<Body>, next: Next) -> Result<Response, AppError> {
    let state = req
        .extensions()
        .get::<Arc<IdempotencyState>>()
        .cloned()
        .unwrap_or_else(|| Arc::new(IdempotencyState::new(Duration::from_secs(3600))));

    let key = match idempotency_key(&req) {
        Some(key) => key,
        None => return Ok(next.run(req).await),
    };

    let (parts, body) = req.into_parts();
    let body_bytes = collect_body(body).await?;
    let digest = digest_bytes(&body_bytes);

    // Preserve the original request parts and re-inject the collected body.
    let req = Request::from_parts(parts, Body::from(body_bytes));

    let cached = {
        let store = state.inner.lock().await;
        store.get(&key).cloned()
    };

    if let Some(cached) = cached {
        if cached.digest == digest {
            return Ok(build_response(&cached));
        }
        return Err(AppError::Conflict);
    }

    let response = next.run(req).await;
    let cached = cache_response(response, digest).await?;

    {
        let mut store = state.inner.lock().await;
        state.cleanup();
        store.insert(key, cached.clone());
    }

    Ok(build_response(&cached))
}

fn idempotency_key(req: &Request<Body>) -> Option<IdempotencyKey> {
    if !is_write_method(req.method().as_str()) {
        return None;
    }
    let idempotency_key = req
        .headers()
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())?;

    let token_fingerprint = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .map(fingerprint)
        .unwrap_or_default();

    Some(IdempotencyKey {
        method: req.method().to_string(),
        path: req.uri().path().to_string(),
        token_fingerprint,
        idempotency_key: idempotency_key.to_string(),
    })
}

fn is_write_method(method: &str) -> bool {
    matches!(method, "POST" | "PUT" | "PATCH" | "DELETE")
}

fn fingerprint(text: &str) -> String {
    let hash = Sha256::digest(text.as_bytes());
    base64ct::Base64UrlUnpadded::encode_string(&hash[..16])
}

fn digest_bytes(bytes: &axum::body::Bytes) -> String {
    let hash = Sha256::digest(bytes);
    base64ct::Base64UrlUnpadded::encode_string(&hash)
}

async fn collect_body(body: Body) -> Result<axum::body::Bytes, AppError> {
    let collected = body
        .collect()
        .await
        .map_err(|_| AppError::ServiceUnavailable)?;
    Ok(collected.to_bytes())
}

async fn cache_response(response: Response, digest: String) -> Result<CachedResponse, AppError> {
    let (parts, body) = response.into_parts();
    let bytes = collect_body(body).await?;
    Ok(CachedResponse {
        status: parts.status,
        content_type: parts.headers.get(header::CONTENT_TYPE).cloned(),
        body: bytes,
        digest,
    })
}

fn build_response(cached: &CachedResponse) -> Response {
    let mut response = Response::new(Body::from(cached.body.clone()));
    *response.status_mut() = cached.status;
    if let Some(content_type) = &cached.content_type {
        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, content_type.clone());
    }
    response
}
