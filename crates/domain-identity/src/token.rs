//! JWT access token claims.

use foundation::{ErrorCode, PlatformError, TenantId, UserId, UtcTimestamp};
use serde::{Deserialize, Serialize};

/// Claims carried by a stateless access token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessTokenClaims {
    /// Token subject (user id).
    pub sub: UserId,
    /// Tenant the user belongs to.
    pub tenant_id: TenantId,
    /// Session generation at issue time.
    pub session_version: u64,
    /// Intended audience.
    pub aud: String,
    /// Issuer.
    pub iss: String,
    /// Not-before time in seconds since the Unix epoch.
    #[serde(default)]
    pub nbf: i64,
    /// Expiration time in seconds since the Unix epoch.
    pub exp: i64,
    /// Unique token identifier.
    pub jti: String,
}

impl AccessTokenClaims {
    /// Return true if the token is expired at `now`.
    pub fn is_expired(&self, now: UtcTimestamp) -> bool {
        self.exp <= now.timestamp_millis() / 1000
    }

    /// Validate issuer, audience and expiration.
    pub fn validate(
        &self,
        expected_issuer: &str,
        expected_audience: &str,
        now: UtcTimestamp,
    ) -> Result<(), PlatformError> {
        if self.iss != expected_issuer {
            return Err(PlatformError::new(
                ErrorCode::Unauthenticated,
                "invalid token",
            ));
        }
        if self.aud != expected_audience {
            return Err(PlatformError::new(
                ErrorCode::Unauthenticated,
                "invalid token",
            ));
        }
        if self.is_expired(now) {
            return Err(PlatformError::new(
                ErrorCode::Unauthenticated,
                "invalid token",
            ));
        }
        Ok(())
    }
}
