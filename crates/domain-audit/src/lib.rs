//! Audit aggregate (events, logs, retention).

pub mod audit_record;

pub use audit_record::{ActionRisk, AuditDetails, AuditRecord, AuditRecordId, AuditResult};

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
