//! Configuration aggregate (definitions and scoped values).

pub mod config_definition;
pub mod config_value;

pub use config_definition::{ConfigDefinition, ConfigValueType};
pub use config_value::{ConfigScope, ConfigValue, ConfigValueId, resolve_config};

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
