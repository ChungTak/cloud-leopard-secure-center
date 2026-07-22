//! Cloud Leopard Secure Center foundation types and utilities.

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[allow(dead_code)]
fn touch_dependencies() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_non_empty() {
        assert!(!version().is_empty());
    }
}
