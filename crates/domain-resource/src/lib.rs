//! Resource aggregate (devices, cameras, bindings, catalog).

pub mod camera;
pub mod device;
pub mod external_binding;
pub mod projection;
pub mod tag;

pub use tag::ResourceType;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn foundation_version() -> &'static str {
    foundation::version()
}

pub fn domain_organization_version() -> &'static str {
    domain_organization::version()
}

#[allow(dead_code)]
fn touch_dependencies() {
    let _v_foundation = foundation::version();
    let _v_domain_organization = domain_organization::version();
}
