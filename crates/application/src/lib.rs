//! Application layer: use cases, transactions, permission checks, projection, outbox.

pub mod authenticate;
pub mod session;
pub mod token_service;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn domain_identity_version() -> &'static str {
    domain_identity::version()
}
pub fn domain_organization_version() -> &'static str {
    domain_organization::version()
}
pub fn domain_authorization_version() -> &'static str {
    domain_authorization::version()
}
pub fn domain_resource_version() -> &'static str {
    domain_resource::version()
}
pub fn domain_audit_version() -> &'static str {
    domain_audit::version()
}
pub fn domain_configuration_version() -> &'static str {
    domain_configuration::version()
}
pub fn storage_api_version() -> &'static str {
    storage_api::version()
}
pub fn message_api_version() -> &'static str {
    message_api::version()
}
pub fn foundation_version() -> &'static str {
    foundation::version()
}

#[allow(dead_code)]
fn touch_dependencies() {
    let _v_domain_identity = domain_identity::version();
    let _v_domain_organization = domain_organization::version();
    let _v_domain_authorization = domain_authorization::version();
    let _v_domain_resource = domain_resource::version();
    let _v_domain_audit = domain_audit::version();
    let _v_domain_configuration = domain_configuration::version();
    let _v_storage_api = storage_api::version();
    let _v_message_api = message_api::version();
    let _v_foundation = foundation::version();
}
