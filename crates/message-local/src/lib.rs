//! In-memory message bus adapter.

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn message_api_version() -> &'static str {
    message_api::version()
}
pub fn foundation_version() -> &'static str {
    foundation::version()
}

#[allow(dead_code)]
fn touch_dependencies() {
    let _v_message_api = message_api::version();
    let _v_foundation = foundation::version();
}
