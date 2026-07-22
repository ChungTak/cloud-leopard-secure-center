//! Identity aggregate (users, sessions, credentials).

pub mod auth;
pub mod credential;
pub mod password;
pub mod tenant;
pub mod user;

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
