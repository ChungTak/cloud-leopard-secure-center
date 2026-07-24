//! Integration tests for HTTP middleware, RFC 9457 problem details and status codes.

use std::{path::PathBuf, sync::Arc};

use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{Extension, Path},
    http::Request,
    routing::{get, post},
};
use foundation::config::RateLimitConfig;
use http_api::{client_ip::TrustedProxyConfig, error::AppError, rate_limit::RateLimitState};
use http_body_util::BodyExt;
use tower::util::ServiceExt;

fn test_app() -> Router {
    test_app_with_cors(None)
}

fn test_app_with_cors(origins: Option<Vec<String>>) -> Router {
    let router = http_api::routes::router()
        .route("/errors/{code}", get(error_handler))
        .route("/echo", post(echo));
    let rate_limit = Arc::new(RateLimitState::new(
        RateLimitConfig {
            requests: 1000,
            window_seconds: 60,
        },
        RateLimitConfig {
            requests: 1000,
            window_seconds: 60,
        },
    ));
    http_api::middleware::with_middleware(router, origins)
        .layer(Extension(rate_limit))
        .layer(Extension(TrustedProxyConfig::default()))
}

async fn echo(_body: Bytes) -> &'static str {
    "ok"
}

async fn error_handler(Path(code): Path<String>) -> Result<Json<&'static str>, AppError> {
    match code.as_str() {
        "400" => Err(AppError::BadRequest {
            field: "field".to_string(),
            message: "bad request".to_string(),
        }),
        "401" => Err(AppError::Unauthenticated),
        "403" => Err(AppError::Denied),
        "404" => Err(AppError::NotFound),
        "409" => Err(AppError::Conflict),
        "412" => Err(AppError::VersionMismatch),
        "422" => Err(AppError::UnprocessableEntity("unprocessable".to_string())),
        "429" => Err(AppError::RateLimit),
        "503" => Err(AppError::ServiceUnavailable),
        _ => Err(AppError::NotFound),
    }
}

fn build_request(method: &str, uri: &str, body: Body) -> Request<Body> {
    match Request::builder().method(method).uri(uri).body(body) {
        Ok(req) => req,
        Err(e) => panic!("failed to build request: {e}"),
    }
}

#[tokio::test]
async fn status_codes_return_problem_details_without_internal_source() {
    let app = test_app();
    let cases = [
        ("400", 400),
        ("401", 401),
        ("403", 403),
        ("404", 404),
        ("409", 409),
        ("412", 412),
        ("422", 422),
        ("429", 429),
        ("503", 503),
    ];

    for (code, expected) in cases {
        let request = build_request("GET", &format!("/errors/{code}"), Body::empty());
        let response = match app.clone().oneshot(request).await {
            Ok(res) => res,
            Err(e) => panic!("request failed: {e}"),
        };

        assert_eq!(response.status().as_u16(), expected);

        let body = match response.into_body().collect().await {
            Ok(collected) => collected.to_bytes(),
            Err(e) => panic!("failed to collect body: {e}"),
        };

        let problem: serde_json::Value = match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(e) => panic!("body is not valid JSON: {e}"),
        };

        assert_eq!(problem["status"].as_u64(), Some(u64::from(expected)));
        assert!(problem["detail"].is_string());
        assert_eq!(
            problem["type"].as_str(),
            Some("about:blank"),
            "problem type should use the RFC 9457 default URI"
        );
        assert!(
            body.windows("source".len())
                .find(|w| w == b"source")
                .is_none(),
            "problem details must not leak internal source keys"
        );
    }
}

#[tokio::test]
async fn payload_too_large_is_converted_to_problem_details() {
    let app = test_app();
    let payload = vec![0u8; 2 * 1024 * 1024];
    let request = build_request("POST", "/echo", Body::from(payload));
    let response = match app.oneshot(request).await {
        Ok(res) => res,
        Err(e) => panic!("request failed: {e}"),
    };

    assert_eq!(response.status().as_u16(), 413);

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(content_type.contains("application/problem+json"));

    let body = match response.into_body().collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => panic!("failed to collect body: {e}"),
    };
    let problem: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => panic!("body is not valid JSON: {e}"),
    };
    assert_eq!(problem["status"].as_u64(), Some(413));
}

#[tokio::test]
async fn health_returns_security_headers_and_request_id() {
    let app = test_app();
    let request = build_request("GET", "/health", Body::empty());
    let response = match app.oneshot(request).await {
        Ok(res) => res,
        Err(e) => panic!("request failed: {e}"),
    };

    assert_eq!(response.status().as_u16(), 200);
    assert!(response.headers().get("x-request-id").is_some());
    assert!(response.headers().get("x-content-type-options").is_some());
    assert!(response.headers().get("x-frame-options").is_some());
    assert!(
        response
            .headers()
            .get("strict-transport-security")
            .is_some()
    );
    assert!(response.headers().get("content-security-policy").is_some());
    assert!(response.headers().get("referrer-policy").is_some());

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(content_type.contains("application/json"));
}

#[tokio::test]
async fn cors_preflight_responds_with_allowed_origin() {
    let app = test_app();
    let request = match Request::builder()
        .method("OPTIONS")
        .uri("/health")
        .header("Origin", "http://example.com")
        .header("Access-Control-Request-Method", "GET")
        .body(Body::empty())
    {
        Ok(req) => req,
        Err(e) => panic!("failed to build request: {e}"),
    };

    let response = match app.oneshot(request).await {
        Ok(res) => res,
        Err(e) => panic!("request failed: {e}"),
    };

    assert_eq!(response.status().as_u16(), 200);
    assert!(
        response
            .headers()
            .get("access-control-allow-origin")
            .is_some()
    );
    assert!(
        response
            .headers()
            .get("access-control-allow-methods")
            .is_some()
    );
}

#[tokio::test]
async fn empty_cors_allowed_origins_denies_cross_origin() {
    let app = test_app_with_cors(Some(vec![]));
    let request = match Request::builder()
        .method("OPTIONS")
        .uri("/health")
        .header("Origin", "http://example.com")
        .header("Access-Control-Request-Method", "GET")
        .body(Body::empty())
    {
        Ok(req) => req,
        Err(e) => panic!("failed to build request: {e}"),
    };

    let response = match app.oneshot(request).await {
        Ok(res) => res,
        Err(e) => panic!("request failed: {e}"),
    };

    assert_eq!(response.status().as_u16(), 200);
    assert!(
        response.headers().get("access-control-allow-origin").is_none(),
        "empty allowed origins must not permit cross-origin requests"
    );
}

#[tokio::test]
async fn wildcard_cors_allowed_origins_is_treated_as_deny() {
    let app = test_app_with_cors(Some(vec!["*".to_string()]));
    let request = match Request::builder()
        .method("OPTIONS")
        .uri("/health")
        .header("Origin", "http://example.com")
        .header("Access-Control-Request-Method", "GET")
        .body(Body::empty())
    {
        Ok(req) => req,
        Err(e) => panic!("failed to build request: {e}"),
    };

    let response = match app.oneshot(request).await {
        Ok(res) => res,
        Err(e) => panic!("request failed: {e}"),
    };

    assert_eq!(response.status().as_u16(), 200);
    assert!(
        response.headers().get("access-control-allow-origin").is_none(),
        "wildcard must not be accepted as a configured origin"
    );
}

#[test]
fn openapi_snapshot_matches_committed_file() {
    let json = http_api::openapi::ApiDoc::json();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("openapi.json");

    if std::env::var("UPDATE_OPENAPI").is_ok() {
        if let Err(e) = std::fs::write(&path, json) {
            panic!("failed to write openapi snapshot: {e}");
        }
        return;
    }

    let expected = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) => panic!("failed to read openapi.json: {e}"),
    };

    if json != expected {
        panic!("openapi.json is out of date; run with UPDATE_OPENAPI=1");
    }
}
