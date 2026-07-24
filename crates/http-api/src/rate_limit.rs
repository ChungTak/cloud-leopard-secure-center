//! In-memory rate limiting for pre-login and authenticated API traffic.

use axum::{
    body::Body,
    extract::Request,
    http::{Extensions, HeaderMap, Method},
    middleware::Next,
    response::Response,
};
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
            if now
                .checked_duration_since(bucket.window_start)
                .is_some_and(|d| d > window)
            {
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
        Self::prune(&mut buckets, window, now);
        true
    }

    fn prune(buckets: &mut HashMap<String, Bucket>, window: Duration, now: Instant) {
        const MAX_KEYS: usize = 10_000;

        // Remove expired buckets first.
        buckets.retain(|_, bucket| {
            now.checked_duration_since(bucket.window_start)
                .is_some_and(|d| d <= window)
        });

        if buckets.len() <= MAX_KEYS {
            return;
        }

        // Fall back to removing the oldest buckets by window start.
        let mut items: Vec<(String, Bucket)> = buckets.drain().collect();
        items.sort_by_key(|a| a.1.window_start);
        let keep = items.len().saturating_sub(MAX_KEYS);
        buckets.extend(items.into_iter().skip(keep));
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
    if req.method() != Method::POST {
        return false;
    }
    let path = req.uri().path();
    // Rate limiting applies to login and token issuance, which may be mounted
    // under a prefix such as `/api/v1` or nested under `/api/v1/tenants/{id}`.
    let base = path.strip_prefix("/api/v1").unwrap_or(path);
    base.split('/')
        .filter(|s| !s.is_empty())
        .any(|s| s == "login" || s == "tokens")
}

fn login_key(headers: &HeaderMap, extensions: &Extensions, config: &TrustedProxyConfig) -> String {
    resolve_client_ip(headers, extensions, config)
        .map(|ip| format!("login:{ip}"))
        .unwrap_or_else(|| "login:unknown".to_string())
}

fn api_key(headers: &HeaderMap, extensions: &Extensions, config: &TrustedProxyConfig) -> String {
    // Rate limiting runs before authentication, so we must not trust any
    // client-controlled identifier extracted from the Authorization header.
    // Use the resolved client IP (or a safe fallback) as the key.
    resolve_client_ip(headers, extensions, config)
        .map(|ip| format!("api:{ip}"))
        .unwrap_or_else(|| "api:unknown".to_string())
}
