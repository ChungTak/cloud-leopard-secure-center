//! Release, upgrade, and disaster recovery operations.
//!
//! Phase 1 freezes the data structures and ports. Real build pipelines,
//! rolling upgrades, and disaster recovery orchestration are deferred.

pub mod release;
pub mod upgrade;

/// Return the crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Return the `foundation` version this crate depends on.
pub fn foundation_version() -> &'static str {
    foundation::version()
}
