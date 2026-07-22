//! Axum HTTP transport adapter.

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
