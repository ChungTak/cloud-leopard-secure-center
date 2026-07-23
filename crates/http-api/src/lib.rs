//! Axum HTTP transport adapter, OpenAPI definitions, and middleware.

pub mod dto;
pub mod error;
pub mod middleware;
pub mod openapi;
pub mod routes;

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
