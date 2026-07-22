//! Identity assurance levels used for high-risk actions.

use serde::{Deserialize, Serialize};

/// How strongly the current actor has been authenticated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssuranceLevel {
    /// No authentication.
    None,
    /// Authenticated with a password or single credential.
    Password,
    /// Authenticated with a second factor (MFA).
    Mfa,
    /// Authenticated with a hardware-backed or phishing-resistant factor.
    Hardware,
}

impl AssuranceLevel {
    /// Return true if `self` satisfies `requirement`.
    pub fn meets(&self, requirement: AssuranceLevel) -> bool {
        *self >= requirement
    }
}
