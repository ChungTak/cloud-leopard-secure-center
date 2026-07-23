//! In-memory rate limiting for pre-login and authenticated API traffic.

use axum::{
    body::Body,
    extract::Request,
    http::{Extensions, HeaderMap, Method},
    middleware::Next,
    response::Response,
};
use base64ct::{Base64UrlUnpadded, Encoding};
use foundation::config::RateLimitConfig;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

use crate::client_ip::{TrustedProxyConfig, resolve_client_ip};
use crate::error::AppError;

/// Shared rate limit state for the HTTP server.
#[derive(Clone)]
pub struct RateLimitState {
    login: Arc<Mutex<HashMap<String, Bucket>>>,
    api: Arc<Mutex<HashMap<String, Bucket>>>,
    login_config: RateLimitConfig,
    api_config: RateLimitConfig,
}

#[derive(Debug, Clone, Copy)]
struct Bucket {
    window_start: Instant,
    count: u32,
}

impl RateLimitState {
    /// Create rate limiters from configuration.
    pub fn new(login_config: RateLimitConfig, api_config: RateLimitConfig) -> Self {
        Self {
            login: Arc::new(Mutex::new(HashMap::new())),
            api: Arc::new(Mutex::new(HashMap::new())),
            login_config,
            api_config,
        }
    }

    /// Check whether a request with the given key is allowed under the chosen bucket.
    pub async fn allow(&self, key: &str, is_login: bool) -> bool {
        let config = if is_login {
            self.login_config
        } else {
            self.api_config
        };
        let buckets = if is_login { &self.login } else { &self.api };
        if config.requests == 0 || config.window_seconds == 0 {
            return true;
        }

        let mut buckets = buckets.lock().await;
        let window = Duration::from_secs(config.window_seconds);
        let now = Instant::now();

        if let Some(bucket) = buckets.get_mut(key) {
            if now.duration_since(bucket.window_start) > window {
                bucket.window_start = now;
                bucket.count = 1;
                return true;
            }
            if bucket.count >= config.requests {
                return false;
            }
            bucket.count += 1;
            return true;
        }

        buckets.insert(
            key.to_string(),
            Bucket {
                window_start: now,
                count: 1,
            },
        );
        true
    }
}

impl std::fmt::Debug for RateLimitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RateLimitState")
            .field("login_config", &self.login_config)
            .field("api_config", &self.api_config)
            .finish_non_exhaustive()
    }
}

/// Middleware that enforces pre-login and authenticated rate limits.
pub async fn rate_limit(req: Request<Body>, next: Next) -> Result<Response, AppError> {
    let state = req
        .extensions()
        .get::<Arc<RateLimitState>>()
        .cloned()
        .ok_or(AppError::Internal)?;
    let proxy_config = req
        .extensions()
        .get::<TrustedProxyConfig>()
        .cloned()
        .unwrap_or_default();

    let is_login = is_login_request(&req);
    let headers = req.headers();
    let extensions = req.extensions();
    let key = if is_login {
        login_key(headers, extensions, &proxy_config)
    } else {
        api_key(headers, extensions, &proxy_config)
    };

    if !state.allow(&key, is_login).await {
        return Err(AppError::RateLimit);
    }

    Ok(next.run(req).await)
}

fn is_login_request(req: &Request<Body>) -> bool {
    let path = req.uri().path();
    req.method() == Method::POST && (path == "/login" || path == "/tokens")
}

fn login_key(headers: &HeaderMap, extensions: &Extensions, config: &TrustedProxyConfig) -> String {
    resolve_client_ip(headers, extensions, config)
        .map(|ip| format!("login:{ip}"))
        .unwrap_or_else(|| "login:unknown".to_string())
}

fn api_key(headers: &HeaderMap, extensions: &Extensions, config: &TrustedProxyConfig) -> String {
    if let Some(jti) = token_jti(headers) {
        return format!("api:{jti}");
    }
    resolve_client_ip(headers, extensions, config)
        .map(|ip| format!("api:{ip}"))
        .unwrap_or_else(|| "api:unknown".to_string())
}

fn token_jti(headers: &HeaderMap) -> Option<String> {
    let header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())?;
    let token = header.strip_prefix("Bearer ")?;
    let segments: Vec<&str> = token.split('.').collect();
    if segments.len() != 3 {
        return None;
    }
    let claims_bytes = Base64UrlUnpadded::decode_vec(segments[1]).ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&claims_bytes).ok()?;
    claims.get("jti")?.as_str().map(String::from)
}
