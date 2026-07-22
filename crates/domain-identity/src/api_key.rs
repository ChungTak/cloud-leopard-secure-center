//! Service account and API key aggregate.

use foundation::{ErrorCode, PlatformError, TenantId, UserId, UtcTimestamp, uuid::Uuid};

/// An API key tied to a user or service account.
#[derive(Debug, Clone)]
pub struct ApiKey {
    pub id: Uuid,
    pub tenant_id: TenantId,
    pub owner_id: UserId,
    pub name: String,
    pub scopes: Vec<String>,
    pub allowed_sources: Vec<String>,
    pub token_hash: String,
    pub expires_at: UtcTimestamp,
    pub revoked_at: Option<UtcTimestamp>,
    pub created_at: UtcTimestamp,
    pub last_used_at: Option<UtcTimestamp>,
}

impl ApiKey {
    /// Create a new API key. The raw token is never stored here; only its hash.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: Uuid,
        tenant_id: TenantId,
        owner_id: UserId,
        name: impl Into<String>,
        scopes: Vec<String>,
        allowed_sources: Vec<String>,
        token_hash: String,
        expires_at: UtcTimestamp,
        created_at: UtcTimestamp,
    ) -> Self {
        Self {
            id,
            tenant_id,
            owner_id,
            name: name.into(),
            scopes,
            allowed_sources,
            token_hash,
            expires_at,
            revoked_at: None,
            created_at,
            last_used_at: None,
        }
    }

    /// Verify that this key can be used from `source` for `scope` at `now`.
    pub fn verify(
        &self,
        source: Option<&str>,
        scope: &str,
        now: UtcTimestamp,
    ) -> Result<(), PlatformError> {
        if self.revoked_at.is_some() {
            return Err(PlatformError::new(
                ErrorCode::Unauthenticated,
                "api key is not valid",
            ));
        }
        if self.expires_at <= now {
            return Err(PlatformError::new(
                ErrorCode::Unauthenticated,
                "api key is not valid",
            ));
        }
        if !self.scopes.iter().any(|s| s == scope) {
            return Err(PlatformError::new(
                ErrorCode::Denied,
                "scope is not allowed for this key",
            ));
        }
        if !self.allowed_sources.is_empty() {
            let allowed = source
                .map(|s| self.allowed_sources.iter().any(|a| a == s))
                .unwrap_or(false);
            if !allowed {
                return Err(PlatformError::new(
                    ErrorCode::Denied,
                    "source is not allowed for this key",
                ));
            }
        }
        Ok(())
    }

    /// Revoke this key.
    pub fn revoke(&mut self, now: UtcTimestamp) {
        self.revoked_at = Some(now);
    }

    /// Record that the key was just used.
    pub fn record_usage(&mut self, now: UtcTimestamp) {
        self.last_used_at = Some(now);
    }
}
