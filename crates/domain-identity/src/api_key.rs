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
    ) -> Result<Self, PlatformError> {
        let name = name.into();
        validate_name(&name)?;
        validate_scopes(&scopes)?;
        validate_allowed_sources(&allowed_sources)?;
        validate_token_hash(&token_hash)?;
        if expires_at <= created_at {
            return Err(PlatformError::invalid(
                "expires_at",
                "api key expiration must be after creation time",
            ));
        }
        Ok(Self {
            id,
            tenant_id,
            owner_id,
            name,
            scopes,
            allowed_sources,
            token_hash,
            expires_at,
            revoked_at: None,
            created_at,
            last_used_at: None,
        })
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

    /// Reconstruct an API key from persisted parts.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        id: Uuid,
        tenant_id: TenantId,
        owner_id: UserId,
        name: impl Into<String>,
        scopes: Vec<String>,
        allowed_sources: Vec<String>,
        token_hash: impl Into<String>,
        expires_at: UtcTimestamp,
        revoked_at: Option<UtcTimestamp>,
        created_at: UtcTimestamp,
        last_used_at: Option<UtcTimestamp>,
    ) -> Result<Self, PlatformError> {
        let name = name.into();
        let token_hash = token_hash.into();
        validate_name(&name)?;
        validate_scopes(&scopes)?;
        validate_allowed_sources(&allowed_sources)?;
        validate_token_hash(&token_hash)?;
        if expires_at <= created_at {
            return Err(PlatformError::invalid(
                "expires_at",
                "api key expiration must be after creation time",
            ));
        }
        if let Some(revoked_at) = revoked_at
            && revoked_at < created_at
        {
            return Err(PlatformError::invalid(
                "revoked_at",
                "api key revocation time must not be before creation time",
            ));
        }
        if let Some(last_used_at) = last_used_at
            && (last_used_at < created_at || last_used_at >= expires_at)
        {
            return Err(PlatformError::invalid(
                "last_used_at",
                "api key last used time must be between creation and expiration",
            ));
        }
        Ok(Self {
            id,
            tenant_id,
            owner_id,
            name,
            scopes,
            allowed_sources,
            token_hash,
            expires_at,
            revoked_at,
            created_at,
            last_used_at,
        })
    }
}

fn validate_name(name: &str) -> Result<(), PlatformError> {
    if name.trim().is_empty() {
        return Err(PlatformError::invalid(
            "api_key_name",
            "api key name must not be empty",
        ));
    }
    if name.len() > 128 {
        return Err(PlatformError::invalid(
            "api_key_name",
            "api key name must be at most 128 characters",
        ));
    }
    Ok(())
}

fn validate_scopes(scopes: &[String]) -> Result<(), PlatformError> {
    if scopes.is_empty() {
        return Err(PlatformError::invalid(
            "api_key_scopes",
            "api key must have at least one scope",
        ));
    }
    for scope in scopes {
        if scope.trim().is_empty() {
            return Err(PlatformError::invalid(
                "api_key_scopes",
                "api key scope must not be empty",
            ));
        }
        if scope.len() > 128 {
            return Err(PlatformError::invalid(
                "api_key_scopes",
                "api key scope must be at most 128 characters",
            ));
        }
    }
    Ok(())
}

fn validate_allowed_sources(sources: &[String]) -> Result<(), PlatformError> {
    for source in sources {
        if source.trim().is_empty() {
            return Err(PlatformError::invalid(
                "api_key_allowed_sources",
                "allowed source must not be empty",
            ));
        }
        if source.len() > 128 {
            return Err(PlatformError::invalid(
                "api_key_allowed_sources",
                "allowed source must be at most 128 characters",
            ));
        }
    }
    Ok(())
}

fn validate_token_hash(token_hash: &str) -> Result<(), PlatformError> {
    if token_hash.trim().is_empty() {
        return Err(PlatformError::invalid(
            "token_hash",
            "api key token hash must not be empty",
        ));
    }
    if token_hash.len() > 256 {
        return Err(PlatformError::invalid(
            "token_hash",
            "api key token hash must be at most 256 characters",
        ));
    }
    Ok(())
}
