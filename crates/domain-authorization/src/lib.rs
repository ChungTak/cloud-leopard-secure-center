//! Authorization aggregate (roles, role bindings, scopes).

pub mod permission;
pub mod role;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn foundation_version() -> &'static str {
    foundation::version()
}

#[allow(dead_code)]
fn touch_dependencies() {
    let _v_foundation = foundation::version();
}
