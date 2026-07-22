//! Cloud Leopard Secure Center foundation types and utilities.

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[allow(dead_code)]
fn touch_dependencies() {}
