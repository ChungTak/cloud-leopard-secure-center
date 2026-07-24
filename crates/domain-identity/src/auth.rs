//! Authentication domain logic and policy.

/// Result of a login attempt.
/// `InvalidCredentials` is returned for all failures to avoid leaking
/// whether the account exists or why the login was denied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthenticationResult {
    /// Authentication succeeded.
    Authenticated,
    /// Authentication failed; no account detail is exposed.
    InvalidCredentials,
}

/// Rate and lockout policy.
#[derive(Debug, Clone, Copy)]
pub struct AuthenticationPolicy {
    /// Failed attempts allowed per identity within the window.
    pub max_attempts_per_identity: u32,
    /// Failed attempts allowed per source IP within the window.
    pub max_attempts_per_source: u32,
    /// Sliding window in seconds. `u64` prevents negative values from silently
    /// disabling the lockout window in the SQL interval expression.
    pub window_seconds: u64,
}

impl AuthenticationPolicy {
    /// Return true if the number of failures from one identity exceeds the policy.
    /// A `max_attempts_per_identity` of `0` is treated as "no limit" rather than
    /// locking immediately, preventing a misconfigured policy from banning every user.
    pub fn identity_locked(&self, failures: i64) -> bool {
        let max = self.max_attempts_per_identity as i64;
        max > 0 && failures >= max
    }

    /// Return true if the number of failures from one source exceeds the policy.
    pub fn source_locked(&self, failures: i64) -> bool {
        let max = self.max_attempts_per_source as i64;
        max > 0 && failures >= max
    }
}

impl Default for AuthenticationPolicy {
    /// Default policy: 5 per identity, 20 per source, 15 minute window.
    fn default() -> Self {
        Self {
            max_attempts_per_identity: 5,
            max_attempts_per_source: 20,
            window_seconds: 900,
        }
    }
}
