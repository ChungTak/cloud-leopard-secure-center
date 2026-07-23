//! Plugin adapter: manifest, Wasm WIT host, and process gRPC host stubs.
//!
//! Phase 1 freezes the plugin SDK contracts and lifecycle. Real Wasmtime gRPC
//! hosts are not linked; unimplemented paths return `Unsupported`.

pub mod manifest;

/// Return the crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Return the `foundation` version this adapter depends on.
pub fn foundation_version() -> &'static str {
    foundation::version()
}
