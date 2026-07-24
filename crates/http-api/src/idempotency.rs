//! HTTP idempotency support using `Idempotency-Key` headers.

use axum::{body::Body, extract::Request, http::StatusCode, middleware::Next, response::Response};
use base64ct::Encoding;
use http_body_util::{BodyExt, Limited};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

use crate::{
    auth::extract_bearer,
    client_ip::{TrustedProxyConfig, resolve_client_ip},
    error::AppError,
    middleware::BODY_LIMIT,
};

/// Maximum number of completed idempotent responses to keep in memory.
const MAX_CACHE_ENTRIES: usize = 10_000;

/// In-memory idempotency store keyed by request signature.
#[derive(Clone)]
pub struct IdempotencyState {
    /// Completed responses indexed by idempotency key.
    cache: Arc<Mutex<HashMap<IdempotencyKey, CachedResponse>>>,
    /// Insertion order used for FIFO eviction when the cache exceeds its size cap.
    order: Arc<Mutex<VecDeque<IdempotencyKey>>>,
    /// Per-key claims used to serialize concurrent requests sharing the same
    /// idempotency key without blocking unrelated requests.
    claims: Arc<Mutex<HashMap<IdempotencyKey, Arc<Mutex<()>>>>>,
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
            cache: Arc::new(Mutex::new(HashMap::new())),
            order: Arc::new(Mutex::new(VecDeque::new())),
            claims: Arc::new(Mutex::new(HashMap::new())),
            ttl,
        }
    }
}

impl std::fmt::Debug for IdempotencyState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdempotencyState")
            .field("ttl", &self.ttl)
            .field("max_entries", &MAX_CACHE_ENTRIES)
            .finish_non_exhaustive()
    }
}

/// Middleware that caches write responses by `Idempotency-Key`.
pub async fn idempotency(req: Request<Body>, next: Next) -> Result<Response, AppError> {
    // Idempotency is only active when the application supplies a state store.
    // Tests and other callers without the extension continue without caching.
    let Some(state) = req.extensions().get::<Arc<IdempotencyState>>().cloned() else {
        return Ok(next.run(req).await);
    };

    let key = match idempotency_key(&req) {
        Some(key) => key,
        None => return Ok(next.run(req).await),
    };

    let (parts, body) = req.into_parts();
    let body_bytes = collect_body(body).await?;
    let digest = digest_bytes(&body_bytes);

    // Preserve the original request parts and re-inject the collected body.
    let req = Request::from_parts(parts, Body::from(body_bytes));

    // Acquire a per-key claim so concurrent requests with the same idempotency
    // key wait for a single execution, while unrelated requests are not blocked.
    let claim = {
        let mut claims = state.claims.lock().await;
        Arc::clone(
            claims
                .entry(key.clone())
                .or_insert_with(|| Arc::new(Mutex::new(()))),
        )
    };
    let _guard = claim.lock().await;

    // Check the cache for a replay while holding the per-key claim. The cache
    // lock is only held during lookup/insertion so unrelated idempotent
    // requests can execute concurrently.
    let cached = {
        let mut cache = state.cache.lock().await;
        let now = Instant::now();
        cache.retain(|_, cached| cached.expires_at > now);
        cache.get(&key).cloned()
    };

    if let Some(cached) = cached {
        drop(_guard);
        state.claims.lock().await.remove(&key);
        if cached.digest == digest {
            return Ok(build_response(&cached));
        }
        return Err(AppError::Conflict);
    }

    // Run the handler without holding the global cache lock. Server errors are
    // not cached so a transient failure can be retried instead of replayed.
    let response = next.run(req).await;
    if response.status().is_server_error() {
        drop(_guard);
        state.claims.lock().await.remove(&key);
        return Ok(response);
    }

    let cached = match cache_response(response, digest, state.ttl).await {
        Ok(c) => c,
        Err(e) => {
            drop(_guard);
            state.claims.lock().await.remove(&key);
            return Err(e);
        }
    };

    {
        let mut cache = state.cache.lock().await;
        let mut order = state.order.lock().await;
        let now = Instant::now();
        cache.retain(|_, cached| cached.expires_at > now);
        while order.front().is_some_and(|k| !cache.contains_key(k)) {
            order.pop_front();
        }
        cache.insert(key.clone(), cached.clone());
        order.push_back(key.clone());
        while cache.len() > MAX_CACHE_ENTRIES {
            match order.pop_front() {
                Some(old) => {
                    cache.remove(&old);
                }
                None => break,
            }
        }
    }

    drop(_guard);
    state.claims.lock().await.remove(&key);
    Ok(build_response(&cached))
}

fn idempotency_key(req: &Request<Body>) -> Option<IdempotencyKey> {
    if !is_write_method(req.method().as_str()) {
        return None;
    }
    let idempotency_key = req
        .headers()
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())?;

    let token_fingerprint = extract_bearer(req.headers())
        .map(|token| fingerprint(&token))
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
    // Apply the same limit as RequestBodyLimitLayer while collecting the body
    // for replay, so idempotent requests cannot be used to bypass size limits.
    let limited = Limited::new(body, BODY_LIMIT);
    let collected = limited.collect().await.map_err(|err| {
        if err.to_string().to_lowercase().contains("length limit") {
            AppError::PayloadTooLarge
        } else {
            AppError::ServiceUnavailable
        }
    })?;
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
