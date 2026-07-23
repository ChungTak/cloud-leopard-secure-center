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
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

use crate::{
    client_ip::{TrustedProxyConfig, resolve_client_ip},
    error::AppError,
};

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
    client_ip: String,
    idempotency_key: String,
}

#[derive(Debug, Clone)]
struct CachedResponse {
    status: StatusCode,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
    digest: String,
    expires_at: Instant,
}

impl IdempotencyState {
    /// Create an in-memory idempotency store with the given TTL.
    pub fn new(ttl: Duration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            ttl,
        }
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
    let state = req.extensions().get::<Arc<IdempotencyState>>().cloned();

    let key = match idempotency_key(&req) {
        Some(key) => key,
        None => return Ok(next.run(req).await),
    };

    let Some(state) = state else {
        return Err(AppError::Internal);
    };

    let (parts, body) = req.into_parts();
    let body_bytes = collect_body(body).await?;
    let digest = digest_bytes(&body_bytes);

    // Preserve the original request parts and re-inject the collected body.
    let req = Request::from_parts(parts, Body::from(body_bytes));

    // Hold the lock across the handler so concurrent requests with the same
    // idempotency key see a single execution result.
    let mut store = state.inner.lock().await;

    if let Some(cached) = store.get(&key) {
        if cached.digest == digest {
            return Ok(build_response(cached));
        }
        return Err(AppError::Conflict);
    }

    let response = next.run(req).await;
    let cached = cache_response(response, digest, state.ttl).await?;

    let now = Instant::now();
    store.retain(|_, cached| cached.expires_at > now);
    store.insert(key, cached.clone());

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

    let client_ip = client_ip_hint(req);

    Some(IdempotencyKey {
        method: req.method().to_string(),
        path: req.uri().path().to_string(),
        token_fingerprint,
        client_ip,
        idempotency_key: idempotency_key.to_string(),
    })
}

fn client_ip_hint(req: &Request<Body>) -> String {
    let config = req
        .extensions()
        .get::<TrustedProxyConfig>()
        .cloned()
        .unwrap_or_default();
    resolve_client_ip(req.headers(), req.extensions(), &config)
        .map(|ip| ip.to_string())
        .unwrap_or_default()
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

async fn cache_response(
    response: Response,
    digest: String,
    ttl: Duration,
) -> Result<CachedResponse, AppError> {
    let (parts, body) = response.into_parts();
    let bytes = collect_body(body).await?;
    Ok(CachedResponse {
        status: parts.status,
        headers: parts.headers,
        body: bytes,
        digest,
        expires_at: Instant::now() + ttl,
    })
}

fn build_response(cached: &CachedResponse) -> Response {
    let mut response = Response::new(Body::from(cached.body.clone()));
    *response.status_mut() = cached.status;
    *response.headers_mut() = cached.headers.clone();
    response
}
