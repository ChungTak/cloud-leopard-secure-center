//! Identity aggregate (users, sessions, credentials, tokens, MFA, API keys).

pub mod api_key;
pub mod assurance;
pub mod auth;
pub mod credential;
pub mod mfa;
pub mod password;
pub mod session;
pub mod tenant;
pub mod token;
pub mod totp;
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
