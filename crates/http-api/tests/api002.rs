//! Integration tests for API-002: token authentication, tenant boundary, proxies, and rate limits.

use std::{
    collections::HashSet,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use application::auth::{AuthContext, Authenticator};
use async_trait::async_trait;
use axum::{
    Json, Router,
    body::Body,
    extract::{ConnectInfo, Extension},
    http::Request,
    routing::{get, post},
};
use foundation::{
    ErrorCode, PlatformError, SystemClock, SystemIdGenerator, SystemRandom, TenantId, UserId,
};
use http_api::{
    client_ip::TrustedProxyConfig, context::ApiRequestContext, rate_limit::RateLimitState,
};
use http_body_util::BodyExt;
use serde_json::json;
use tokio::sync::Mutex;
use tower::util::ServiceExt;

fn ok_or_panic<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
    match result {
        Ok(v) => v,
        Err(e) => panic!("{e}"),
    }
}

fn tenant_id(seed: u128) -> TenantId {
    let id_gen = SystemIdGenerator::new(SystemClock, SystemRandom);
    // Deterministic enough for tests: generate and select by seed count.
    let mut id = ok_or_panic(TenantId::generate(&id_gen));
    for _ in 0..(seed % 10) {
        id = ok_or_panic(TenantId::generate(&id_gen));
    }
    id
}

fn user_id(seed: u128) -> UserId {
    let id_gen = SystemIdGenerator::new(SystemClock, SystemRandom);
    let mut id = ok_or_panic(UserId::generate(&id_gen));
    for _ in 0..(seed % 10) {
        id = ok_or_panic(UserId::generate(&id_gen));
    }
    id
}

#[derive(Clone)]
struct TestAuthenticator {
    user_id: UserId,
    tenant_id: TenantId,
    valid: std::collections::HashSet<String>,
    revoked: Arc<Mutex<HashSet<String>>>,
}

impl TestAuthenticator {
    fn new(user_id: UserId, tenant_id: TenantId, valid: &[&str]) -> Self {
        Self {
            user_id,
            tenant_id,
            valid: valid.iter().map(|s| (*s).to_string()).collect(),
            revoked: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    async fn revoke(&self, token: &str) {
        self.revoked.lock().await.insert(token.to_string());
    }
}

#[async_trait]
impl Authenticator for TestAuthenticator {
    async fn authenticate(&self, token: &str) -> Result<AuthContext, PlatformError> {
        if !self.valid.contains(token) {
            return Err(PlatformError::new(
                ErrorCode::Unauthenticated,
                "invalid token",
            ));
        }
        if self.revoked.lock().await.contains(token) {
            return Err(PlatformError::new(ErrorCode::Unauthenticated, "revoked"));
        }
        Ok(AuthContext {
            user_id: self.user_id,
            tenant_id: self.tenant_id,
            session_version: 1,
            jti: token.to_string(),
        })
    }
}

fn test_app(
    authenticator: Arc<dyn Authenticator>,
    rate_limit: Arc<RateLimitState>,
    proxy_config: TrustedProxyConfig,
) -> Router {
    let router = Router::new()
        .route("/tenants/{tenant_id}/profile", get(profile_handler))
        .route("/protected", get(protected_handler))
        .route("/client-ip", get(client_ip_handler))
        .route("/login", post(login_handler))
        .route("/tenants/{tenant_id}/tokens/refresh", post(login_handler));

    http_api::middleware::with_middleware(router, None, SystemClock, SystemRandom)
        .layer(Extension(authenticator))
        .layer(Extension(rate_limit))
        .layer(Extension(proxy_config))
}

fn default_rate() -> Arc<RateLimitState> {
    Arc::new(RateLimitState::new(
        foundation::config::RateLimitConfig {
            requests: 100,
            window_seconds: 60,
        },
        foundation::config::RateLimitConfig {
            requests: 100,
            window_seconds: 60,
        },
    ))
}

async fn profile_handler(
    ctx: ApiRequestContext,
) -> Result<Json<serde_json::Value>, http_api::error::AppError> {
    Ok(Json(json!({
        "actor_id": ctx.0.actor_id.map(|id| id.as_uuid().to_string()),
        "tenant_id": ctx.0.tenant_id.map(|id| id.as_uuid().to_string()),
    })))
}

async fn protected_handler(
    _auth: http_api::auth::Auth,
) -> Result<&'static str, http_api::error::AppError> {
    Ok("ok")
}

async fn client_ip_handler(client_ip: http_api::client_ip::ClientIp) -> Json<serde_json::Value> {
    Json(json!({
        "ip": client_ip.0.map(|ip| ip.to_string()),
    }))
}

async fn login_handler() -> &'static str {
    "ok"
}

fn build_request(method: &str, uri: &str, body: Body) -> Request<Body> {
    match Request::builder().method(method).uri(uri).body(body) {
        Ok(req) => req,
        Err(e) => panic!("failed to build request: {e}"),
    }
}

fn bearer(method: &str, uri: &str, token: &str) -> Request<Body> {
    match Request::builder()
        .method(method)
        .uri(uri)
        .header("Authorization", format!("Bearer {token}"))
        .body(Body::empty())
    {
        Ok(req) => req,
        Err(e) => panic!("failed to build request: {e}"),
    }
}

fn with_connect_info(req: Request<Body>, addr: SocketAddr) -> Request<Body> {
    let (mut parts, body) = req.into_parts();
    parts.extensions.insert(ConnectInfo(addr));
    Request::from_parts(parts, body)
}

#[tokio::test]
async fn missing_token_is_unauthenticated() {
    let tenant = tenant_id(0);
    let user = user_id(0);
    let auth = Arc::new(TestAuthenticator::new(user, tenant, &[]));
    let app = test_app(auth, default_rate(), TrustedProxyConfig::default());
    let req = build_request("GET", "/protected", Body::empty());
    let response = match app.oneshot(req).await {
        Ok(res) => res,
        Err(e) => panic!("request failed: {e}"),
    };
    assert_eq!(response.status().as_u16(), 401);
}

#[tokio::test]
async fn invalid_token_is_unauthenticated() {
    let tenant = tenant_id(0);
    let user = user_id(0);
    let auth = Arc::new(TestAuthenticator::new(user, tenant, &["valid"]));
    let app = test_app(auth, default_rate(), TrustedProxyConfig::default());
    let req = bearer("GET", "/protected", "wrong");
    let response = match app.oneshot(req).await {
        Ok(res) => res,
        Err(e) => panic!("request failed: {e}"),
    };
    assert_eq!(response.status().as_u16(), 401);
}

#[tokio::test]
async fn revoked_token_is_unauthenticated() {
    let tenant = tenant_id(0);
    let user = user_id(0);
    let auth = Arc::new(TestAuthenticator::new(user, tenant, &["session-abc"]));
    let token = "session-abc";
    auth.revoke(token).await;
    let app = test_app(auth, default_rate(), TrustedProxyConfig::default());
    let req = bearer("GET", "/protected", token);
    let response = match app.oneshot(req).await {
        Ok(res) => res,
        Err(e) => panic!("request failed: {e}"),
    };
    assert_eq!(response.status().as_u16(), 401);
}

#[tokio::test]
async fn valid_token_reaches_protected_route() {
    let tenant = tenant_id(0);
    let user = user_id(0);
    let auth = Arc::new(TestAuthenticator::new(user, tenant, &["valid"]));
    let app = test_app(auth, default_rate(), TrustedProxyConfig::default());
    let req = bearer("GET", "/protected", "valid");
    let response = match app.oneshot(req).await {
        Ok(res) => res,
        Err(e) => panic!("request failed: {e}"),
    };
    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
async fn tenant_path_mismatch_is_denied() {
    let tenant_a = tenant_id(0);
    let user = user_id(0);
    let auth = Arc::new(TestAuthenticator::new(user, tenant_a, &["valid"]));
    let app = test_app(auth, default_rate(), TrustedProxyConfig::default());
    let tenant_b = tenant_id(1);
    let uri = format!("/tenants/{}/profile", tenant_b.as_uuid());
    let req = bearer("GET", &uri, "valid");
    let response = match app.oneshot(req).await {
        Ok(res) => res,
        Err(e) => panic!("request failed: {e}"),
    };
    assert_eq!(response.status().as_u16(), 403);
}

#[tokio::test]
async fn tenant_path_match_returns_context() {
    let tenant = tenant_id(0);
    let user = user_id(0);
    let auth = Arc::new(TestAuthenticator::new(user, tenant, &["valid"]));
    let app = test_app(auth, default_rate(), TrustedProxyConfig::default());
    let uri = format!("/tenants/{}/profile", tenant.as_uuid());
    let req = bearer("GET", &uri, "valid");
    let response = match app.oneshot(req).await {
        Ok(res) => res,
        Err(e) => panic!("request failed: {e}"),
    };
    assert_eq!(response.status().as_u16(), 200);

    let body = match response.into_body().collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => panic!("failed to collect body: {e}"),
    };
    let payload: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => panic!("body is not valid JSON: {e}"),
    };
    assert_eq!(payload["tenant_id"], tenant.as_uuid().to_string());
    assert_eq!(payload["actor_id"], user.as_uuid().to_string());
}

#[tokio::test]
async fn untrusted_proxy_ignores_forwarded_for() {
    let tenant = tenant_id(0);
    let user = user_id(0);
    let auth = Arc::new(TestAuthenticator::new(user, tenant, &["valid"]));
    let app = test_app(auth, default_rate(), TrustedProxyConfig::default());
    let base = build_request("GET", "/client-ip", Body::empty());
    let (mut parts, body) = base.into_parts();
    parts.headers.insert(
        "X-Forwarded-For",
        axum::http::HeaderValue::from_static("1.2.3.4"),
    );
    let req = Request::from_parts(parts, body);
    let response = match app.oneshot(req).await {
        Ok(res) => res,
        Err(e) => panic!("request failed: {e}"),
    };
    assert_eq!(response.status().as_u16(), 200);

    let body = match response.into_body().collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => panic!("failed to collect body: {e}"),
    };
    let payload: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => panic!("body is not valid JSON: {e}"),
    };
    assert!(payload["ip"].is_null());
}

#[tokio::test]
async fn trusted_proxy_uses_forwarded_for() {
    let tenant = tenant_id(0);
    let user = user_id(0);
    let auth = Arc::new(TestAuthenticator::new(user, tenant, &["valid"]));
    let proxy =
        TrustedProxyConfig::parse(&["127.0.0.1/32".to_string()]).unwrap_or_else(|e| panic!("{e}"));
    let app = test_app(auth, default_rate(), proxy);

    let peer = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12345);
    let mut req = build_request("GET", "/client-ip", Body::empty());
    let (mut parts, body) = req.into_parts();
    parts.extensions.insert(ConnectInfo(peer));
    parts.headers.insert(
        "X-Forwarded-For",
        axum::http::HeaderValue::from_static("1.2.3.4, 5.6.7.8"),
    );
    req = Request::from_parts(parts, body);

    let response = match app.oneshot(req).await {
        Ok(res) => res,
        Err(e) => panic!("request failed: {e}"),
    };
    assert_eq!(response.status().as_u16(), 200);

    let body = match response.into_body().collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => panic!("failed to collect body: {e}"),
    };
    let payload: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => panic!("body is not valid JSON: {e}"),
    };
    // With a trusted proxy the rightmost untrusted address is used.
    assert_eq!(payload["ip"], "5.6.7.8");
}

#[tokio::test]
async fn untrusted_peer_does_not_use_forwarded_for() {
    let tenant = tenant_id(0);
    let user = user_id(0);
    let auth = Arc::new(TestAuthenticator::new(user, tenant, &["valid"]));
    let proxy =
        TrustedProxyConfig::parse(&["127.0.0.1/32".to_string()]).unwrap_or_else(|e| panic!("{e}"));
    let app = test_app(auth, default_rate(), proxy);

    let peer = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), 12345);
    let mut req = build_request("GET", "/client-ip", Body::empty());
    let (mut parts, body) = req.into_parts();
    parts.extensions.insert(ConnectInfo(peer));
    parts.headers.insert(
        "X-Forwarded-For",
        axum::http::HeaderValue::from_static("1.2.3.4"),
    );
    req = Request::from_parts(parts, body);

    let response = match app.oneshot(req).await {
        Ok(res) => res,
        Err(e) => panic!("request failed: {e}"),
    };
    assert_eq!(response.status().as_u16(), 200);

    let body = match response.into_body().collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => panic!("failed to collect body: {e}"),
    };
    let payload: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => panic!("body is not valid JSON: {e}"),
    };
    assert_eq!(payload["ip"], "10.0.0.1");
}

#[tokio::test]
async fn login_rate_limit_returns_429() {
    let tenant = tenant_id(0);
    let user = user_id(0);
    let auth = Arc::new(TestAuthenticator::new(user, tenant, &[]));
    let rate = Arc::new(RateLimitState::new(
        foundation::config::RateLimitConfig {
            requests: 1,
            window_seconds: 60,
        },
        foundation::config::RateLimitConfig {
            requests: 100,
            window_seconds: 60,
        },
    ));
    let app = test_app(auth, rate, TrustedProxyConfig::default());

    let peer = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12345);
    let req = with_connect_info(build_request("POST", "/login", Body::empty()), peer);
    let first = match app.clone().oneshot(req).await {
        Ok(res) => res,
        Err(e) => panic!("request failed: {e}"),
    };
    assert_eq!(first.status().as_u16(), 200);

    let req = with_connect_info(build_request("POST", "/login", Body::empty()), peer);
    let second = match app.oneshot(req).await {
        Ok(res) => res,
        Err(e) => panic!("request failed: {e}"),
    };
    assert_eq!(second.status().as_u16(), 429);
}

#[tokio::test]
async fn api_rate_limit_returns_429_by_token() {
    let tenant = tenant_id(0);
    let user = user_id(0);
    let auth = Arc::new(TestAuthenticator::new(user, tenant, &["token-a"]));
    let rate = Arc::new(RateLimitState::new(
        foundation::config::RateLimitConfig {
            requests: 100,
            window_seconds: 60,
        },
        foundation::config::RateLimitConfig {
            requests: 1,
            window_seconds: 60,
        },
    ));
    let app = test_app(auth, rate, TrustedProxyConfig::default());
    let uri = format!("/tenants/{}/profile", tenant.as_uuid());

    let first = match app.clone().oneshot(bearer("GET", &uri, "token-a")).await {
        Ok(res) => res,
        Err(e) => panic!("request failed: {e}"),
    };
    assert_eq!(first.status().as_u16(), 200);

    let second = match app.oneshot(bearer("GET", &uri, "token-a")).await {
        Ok(res) => res,
        Err(e) => panic!("request failed: {e}"),
    };
    assert_eq!(second.status().as_u16(), 429);
}

#[tokio::test]
async fn nested_token_refresh_uses_login_rate_limit() {
    let tenant = tenant_id(0);
    let user = user_id(0);
    let auth = Arc::new(TestAuthenticator::new(user, tenant, &[]));
    let rate = Arc::new(RateLimitState::new(
        foundation::config::RateLimitConfig {
            requests: 1,
            window_seconds: 60,
        },
        foundation::config::RateLimitConfig {
            requests: 100,
            window_seconds: 60,
        },
    ));
    let app = test_app(auth, rate, TrustedProxyConfig::default());

    let peer = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12345);
    let uri = format!("/tenants/{}/tokens/refresh", tenant.as_uuid());
    let req = with_connect_info(build_request("POST", &uri, Body::empty()), peer);
    let first = match app.clone().oneshot(req).await {
        Ok(res) => res,
        Err(e) => panic!("request failed: {e}"),
    };
    assert_eq!(first.status().as_u16(), 200);

    let req = with_connect_info(build_request("POST", &uri, Body::empty()), peer);
    let second = match app.oneshot(req).await {
        Ok(res) => res,
        Err(e) => panic!("request failed: {e}"),
    };
    assert_eq!(second.status().as_u16(), 429);
}
