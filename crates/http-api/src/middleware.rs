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
use foundation::{IdGenerator, MessageId, RequestContext, SystemClock, SystemRandom};
use std::time::Duration;
use tower::{ServiceBuilder, timeout::TimeoutLayer};
use tower_http::{
    cors::CorsLayer,
    limit::RequestBodyLimitLayer,
    normalize_path::NormalizePathLayer,
    request_id::{MakeRequestId, PropagateRequestIdLayer, RequestId, SetRequestIdLayer},
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};

use crate::error::{ProblemDetails, from_middleware_error};

/// Default request body size limit in bytes (1 MiB).
const BODY_LIMIT: usize = 1024 * 1024;
/// Default request timeout.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Builds a `Router` with the standard middleware stack applied.
pub fn with_middleware(router: Router) -> Router {
    let request_id_header = HeaderName::from_static("x-request-id");
    let trace_id_header = HeaderName::from_static("x-trace-id");

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
            .layer(CorsLayer::permissive())
            // Generate and propagate request ids back to the client.
            .layer(SetRequestIdLayer::new(
                request_id_header.clone(),
                MakeUuidRequestId,
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

/// Request ID generator backed by `Uuid::new_v7`.
#[derive(Clone, Copy, Debug)]
struct MakeUuidRequestId;

impl MakeRequestId for MakeUuidRequestId {
    fn make_request_id<B>(&mut self, _req: &Request<B>) -> Option<RequestId> {
        let generator = foundation::SystemIdGenerator::new(SystemClock, SystemRandom);
        let id = generator.generate().ok()?;
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
        StatusCode::REQUEST_TIMEOUT => "request timeout",
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
