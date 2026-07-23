//! HTTP error handling and RFC 9457 Problem Details.

use axum::{
    body::Body,
    extract::rejection::JsonRejection,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use foundation::PlatformError;
use serde::Serialize;

/// RFC 9457 Problem Details response.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemDetails {
    #[serde(rename = "type")]
    pub problem_type: String,
    pub title: String,
    pub status: u16,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
}

impl ProblemDetails {
    /// Create a problem details value for the given status and public message.
    pub fn new(status: StatusCode, detail: impl Into<String>, instance: Option<String>) -> Self {
        Self {
            problem_type: "about:blank".to_string(),
            title: status.canonical_reason().unwrap_or("Error").to_string(),
            status: status.as_u16(),
            detail: detail.into(),
            instance,
        }
    }
}

/// Application-level HTTP error type.
#[derive(Debug, Clone, thiserror::Error)]
pub enum AppError {
    #[error("bad request: {field}: {message}")]
    BadRequest { field: String, message: String },
    #[error("unauthenticated")]
    Unauthenticated,
    #[error("denied")]
    Denied,
    #[error("not found")]
    NotFound,
    #[error("conflict")]
    Conflict,
    #[error("precondition failed")]
    VersionMismatch,
    #[error("unprocessable entity")]
    UnprocessableEntity(String),
    #[error("rate limit exceeded")]
    RateLimit,
    #[error("request timeout")]
    Timeout,
    #[error("payload too large")]
    PayloadTooLarge,
    #[error("service unavailable")]
    ServiceUnavailable,
    #[error("not implemented")]
    NotImplemented,
    #[error("internal server error")]
    Internal,
}

impl AppError {
    /// HTTP status code for this error.
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::BadRequest { .. } => StatusCode::BAD_REQUEST,
            Self::Unauthenticated => StatusCode::UNAUTHORIZED,
            Self::Denied => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Conflict => StatusCode::CONFLICT,
            Self::VersionMismatch => StatusCode::PRECONDITION_FAILED,
            Self::UnprocessableEntity(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::RateLimit => StatusCode::TOO_MANY_REQUESTS,
            Self::Timeout => StatusCode::REQUEST_TIMEOUT,
            Self::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            Self::NotImplemented => StatusCode::NOT_IMPLEMENTED,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Public-safe detail message.
    pub fn detail(&self) -> String {
        match self {
            Self::BadRequest { field, message } => format!("{field}: {message}"),
            Self::UnprocessableEntity(message) => message.clone(),
            _ => self
                .status_code()
                .canonical_reason()
                .unwrap_or("Error")
                .to_string(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let detail = self.detail();
        let problem = ProblemDetails::new(status, detail, None);

        let body = match serde_json::to_string(&problem) {
            Ok(body) => body,
            Err(_) => return (status, Body::empty()).into_response(),
        };

        let mut response = Response::new(Body::from(body));
        *response.status_mut() = status;
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/problem+json"),
        );
        response
    }
}

impl From<PlatformError> for AppError {
    fn from(value: PlatformError) -> Self {
        match value {
            PlatformError::Invalid { field, message } => Self::BadRequest { field, message },
            PlatformError::Unauthenticated => Self::Unauthenticated,
            PlatformError::Denied => Self::Denied,
            PlatformError::NotFound => Self::NotFound,
            PlatformError::Exists | PlatformError::Conflict => Self::Conflict,
            PlatformError::RateLimit => Self::RateLimit,
            PlatformError::Timeout => Self::Timeout,
            PlatformError::Cancelled
            | PlatformError::Unavailable
            | PlatformError::UnknownOutcome => Self::ServiceUnavailable,
            PlatformError::Unsupported => Self::NotImplemented,
            PlatformError::VersionMismatch => Self::VersionMismatch,
            PlatformError::Internal => Self::Internal,
        }
    }
}

impl From<JsonRejection> for AppError {
    fn from(rejection: JsonRejection) -> Self {
        Self::UnprocessableEntity(rejection.to_string())
    }
}

/// Convert middleware errors (timeouts, body limits) into `AppError`.
pub fn from_middleware_error(err: axum::BoxError) -> AppError {
    if err.is::<tower::timeout::error::Elapsed>() {
        return AppError::Timeout;
    }
    if err.downcast_ref::<std::convert::Infallible>().is_some() {
        return AppError::Internal;
    }
    if err.to_string().to_lowercase().contains("length limit") {
        return AppError::PayloadTooLarge;
    }
    AppError::ServiceUnavailable
}
