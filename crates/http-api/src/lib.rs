//! Axum HTTP transport adapter, OpenAPI definitions, and middleware.

pub mod auth;
pub mod client_ip;
pub mod context;
pub mod dto;
pub mod error;
pub mod etag;
pub mod idempotency;
pub mod middleware;
pub mod openapi;
pub mod pagination;
pub mod rate_limit;
pub mod routes;
pub mod sse;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn application_version() -> &'static str {
    application::version()
}
pub fn foundation_version() -> &'static str {
    foundation::version()
}

#[allow(dead_code)]
fn touch_dependencies() {
    let _v_application = application::version();
    let _v_foundation = foundation::version();
}
