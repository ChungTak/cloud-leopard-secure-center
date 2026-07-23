//! Test infrastructure, fixtures, and contract suite for the security platform.
//!
//! Phase 1 provides in-memory fakes and configuration-driven stubs. Real
//! PostgreSQL/NATS-backed test runs are left to the test runner environment.

#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

pub mod chaos;
pub mod contract_suite;
pub mod fixtures;
pub mod performance;

/// Return the crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
