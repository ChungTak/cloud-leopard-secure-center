//! Configuration aggregate (definitions and scoped values).

pub mod config_definition;
pub mod config_value;

pub use config_definition::{ConfigDefinition, ConfigValueType};
pub use config_value::{ConfigScope, ConfigValue, ConfigValueId, resolve_config};

pub(crate) const MAX_CONFIG_KEY_BYTES: usize = 256;
pub(crate) const MAX_CONFIG_VALUE_BYTES: usize = 1024 * 1024;

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
