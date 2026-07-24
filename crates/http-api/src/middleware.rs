//! HTTP middleware: request/trace IDs, CORS, body limit, timeout, security headers.

use axum::{
    Router,
    body::Body,
    http::{
        Request, StatusCode,
        header::{self, HeaderName, HeaderValue},
    },
    middleware::Next,
    response::{IntoResponse, Response},
};
use foundation::{Clock, MessageId, RandomSource, RequestContext, generate_uuid};
use std::sync::Arc;
use std::time::Duration;
use tower::{ServiceBuilder, timeout::TimeoutLayer};
use tower_http::{
    cors::{AllowOrigin, Any, CorsLayer},
    limit::RequestBodyLimitLayer,
    normalize_path::NormalizePathLayer,
    request_id::{MakeRequestId, PropagateRequestIdLayer, RequestId, SetRequestIdLayer},
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};

/// A CORS layer that denies every cross-origin request.
fn deny_cors() -> CorsLayer {
    CorsLayer::new().allow_origin(AllowOrigin::list(Vec::new()))
}

use crate::error::{ProblemDetails, from_middleware_error};

/// Default request body size limit in bytes (1 MiB).
const BODY_LIMIT: usize = 1024 * 1024;
/// Default request timeout.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Builds a `Router` with the standard middleware stack applied.
///
/// `cors_allowed_origins` controls the CORS policy:
/// - `None` uses the permissive test policy (only for local tests).
/// - `Some(vec![])` denies all cross-origin requests.
/// - `Some(origins)` allows the listed origins only. The wildcard `*` is
///   intentionally treated as an invalid origin so a misconfigured environment
///   cannot silently allow every origin.
///
/// `clock` and `random` are used to generate the `x-request-id` so request IDs
/// are deterministic and testable instead of pulling from the hidden system clock.
pub fn with_middleware(
    router: Router,
    cors_allowed_origins: Option<Vec<String>>,
    clock: impl Clock + 'static,
    random: impl RandomSource + 'static,
) -> Router {
    let request_id_header = HeaderName::from_static("x-request-id");
    let trace_id_header = HeaderName::from_static("x-trace-id");

    let cors = cors_layer(cors_allowed_origins);

    let router = router.layer(
        ServiceBuilder::new()
            // Security headers; outermost on response.
            .layer(SetResponseHeaderLayer::if_not_present(
                header::STRICT_TRANSPORT_SECURITY,
                HeaderValue::from_static("max-age=31536000; includeSubDomains"),
            ))
            .layer(SetResponseHeaderLayer::if_not_present(
                HeaderName::from_static("x-content-type-options"),
                HeaderValue::from_static("nosniff"),
            ))
            .layer(SetResponseHeaderLayer::if_not_present(
                HeaderName::from_static("x-frame-options"),
                HeaderValue::from_static("DENY"),
            ))
            .layer(SetResponseHeaderLayer::if_not_present(
                HeaderName::from_static("content-security-policy"),
                HeaderValue::from_static("default-src 'self'"),
            ))
            .layer(SetResponseHeaderLayer::if_not_present(
                HeaderName::from_static("referrer-policy"),
                HeaderValue::from_static("strict-origin-when-cross-origin"),
            ))
            .layer(TraceLayer::new_for_http())
            .layer(cors)
            // Generate and propagate request ids back to the client.
            .layer(SetRequestIdLayer::new(
                request_id_header.clone(),
                MakeUuidRequestId::new(clock, random),
            ))
            .layer(PropagateRequestIdLayer::new(request_id_header.clone()))
            .layer(PropagateRequestIdLayer::new(trace_id_header))
            // Build request-scoped context after the request id is generated.
            .layer(axum::middleware::from_fn(set_request_context))
            // Convert middleware failures (timeouts) into RFC 9457 responses.
            .layer(axum::error_handling::HandleErrorLayer::new(
                |err: axum::BoxError| async move {
                    IntoResponse::into_response(from_middleware_error(err))
                },
            ))
            .layer(TimeoutLayer::new(REQUEST_TIMEOUT))
            .layer(RequestBodyLimitLayer::new(BODY_LIMIT))
            // Normalize paths before routing.
            .layer(NormalizePathLayer::trim_trailing_slash()),
    );

    // Convert raw status-only error responses (e.g. payload too large) to RFC 9457.
    // Applied outside the main ServiceBuilder to avoid type-inference issues with from_fn.
    let router = router.layer(axum::middleware::map_response(map_problem_details));

    // Enforce pre-login and authenticated API rate limits.
    // The `RateLimitState` and `TrustedProxyConfig` extensions are supplied by the app.
    router.layer(axum::middleware::from_fn(crate::rate_limit::rate_limit))
}

/// Request ID generator backed by UUIDv7 from injected `Clock`/`RandomSource`.
#[derive(Clone)]
struct MakeUuidRequestId {
    clock: Arc<dyn Clock>,
    random: Arc<dyn RandomSource>,
}

impl MakeUuidRequestId {
    fn new(clock: impl Clock + 'static, random: impl RandomSource + 'static) -> Self {
        Self {
            clock: Arc::new(clock),
            random: Arc::new(random),
        }
    }
}

impl MakeRequestId for MakeUuidRequestId {
    fn make_request_id<B>(&mut self, _req: &Request<B>) -> Option<RequestId> {
        let id = generate_uuid(&*self.clock, &*self.random).ok()?;
        let value = HeaderValue::from_str(&id.to_string()).ok()?;
        Some(RequestId::new(value))
    }
}

/// Set a default request context extension using the generated request id.
async fn set_request_context(mut req: Request<Body>, next: Next) -> Response {
    if let Some(request_id) = req.extensions().get::<RequestId>().cloned()
        && let Ok(text) = request_id.header_value().to_str()
        && let Ok(message_id) = MessageId::parse_str(text)
    {
        req.extensions_mut().insert(RequestContext {
            request_id: Some(message_id),
            ..RequestContext::default()
        });
    }
    next.run(req).await
}

/// Wrap a response so it carries the current request id header.
pub fn response_with_context(response: &mut Response, ctx: &RequestContext) {
    if let Some(request_id) = &ctx.request_id
        && let Ok(value) = HeaderValue::from_str(&request_id.to_hyphenated())
    {
        response.headers_mut().insert("x-request-id", value);
    }
}

/// Convert status-only error responses from tower-http middleware into RFC 9457 documents.
async fn map_problem_details(response: Response) -> Response {
    let status = response.status();
    if !status.is_client_error() && !status.is_server_error() {
        return response;
    }
    if is_problem_json(response.headers()) {
        return response;
    }

    let detail = match status {
        StatusCode::PAYLOAD_TOO_LARGE => "request payload too large",
        StatusCode::GATEWAY_TIMEOUT => "request timeout",
        _ => status.canonical_reason().unwrap_or("error"),
    };

    let body = match serde_json::to_string(&ProblemDetails::new(status, detail, None)) {
        Ok(body) => body,
        Err(_) => return response,
    };

    let (mut parts, _body) = response.into_parts();
    parts.headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/problem+json"),
    );
    Response::from_parts(parts, Body::from(body))
}

fn is_problem_json(headers: &axum::http::HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("application/problem+json"))
}

fn cors_layer(cors_allowed_origins: Option<Vec<String>>) -> CorsLayer {
    match cors_allowed_origins {
        None => CorsLayer::permissive(),
        Some(origins) if origins.is_empty() => deny_cors(),
        Some(origins) => {
            let allowed: Vec<HeaderValue> = origins
                .iter()
                .filter(|o| o.trim() != "*")
                .filter_map(|o| o.trim().parse::<HeaderValue>().ok())
                .collect();
            if allowed.is_empty() {
                deny_cors()
            } else {
                CorsLayer::new()
                    .allow_origin(AllowOrigin::list(allowed))
                    .allow_methods(Any)
                    .allow_headers(Any)
            }
        }
    }
}
