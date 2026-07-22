//! PostgreSQL storage adapter implementing storage-api ports.

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn storage_api_version() -> &'static str {
    storage_api::version()
}
pub fn foundation_version() -> &'static str {
    foundation::version()
}

#[allow(dead_code)]
fn touch_dependencies() {
    let _v_storage_api = storage_api::version();
    let _v_foundation = foundation::version();
}
