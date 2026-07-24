//! Session and refresh token primitives.

use foundation::{TenantId, UserId, UtcTimestamp, uuid::Uuid};

/// A stored refresh token. The raw token value is never kept here;
/// only its hash is persisted for reuse detection.
#[derive(Clone)]
pub struct RefreshToken {
    /// Stable token identifier.
    pub id: Uuid,
    /// Owning tenant.
    pub tenant_id: TenantId,
    /// Token owner.
    pub user_id: UserId,
    /// Family this token belongs to.
    pub family_id: Uuid,
    /// Hash of the raw token, used for lookup and replay detection.
    pub token_hash: String,
    /// User session generation this token is tied to.
    pub session_version: u64,
    /// Whether the token has already been used to refresh.
    pub used: bool,
    /// Expiration timestamp.
    pub expires_at: UtcTimestamp,
    /// Creation timestamp.
    pub created_at: UtcTimestamp,
}

impl std::fmt::Debug for RefreshToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RefreshToken")
            .field("id", &self.id)
            .field("tenant_id", &self.tenant_id)
            .field("user_id", &self.user_id)
            .field("family_id", &self.family_id)
            .field("token_hash", &"<redacted>")
            .field("session_version", &self.session_version)
            .field("used", &self.used)
            .field("expires_at", &self.expires_at)
            .field("created_at", &self.created_at)
            .finish()
    }
}

impl RefreshToken {
    /// Reconstruct a refresh token from persisted parts.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        id: Uuid,
        tenant_id: TenantId,
        user_id: UserId,
        family_id: Uuid,
        token_hash: impl Into<String>,
        session_version: u64,
        used: bool,
        expires_at: UtcTimestamp,
        created_at: UtcTimestamp,
    ) -> Result<Self, foundation::PlatformError> {
        let token_hash = token_hash.into();
        if token_hash.trim().is_empty() {
            return Err(foundation::PlatformError::invalid(
                "token_hash",
                "refresh token hash must not be empty",
            ));
        }
        if token_hash.len() > 512 {
            return Err(foundation::PlatformError::invalid(
                "token_hash",
                "refresh token hash must be at most 512 characters",
            ));
        }
        if expires_at <= created_at {
            return Err(foundation::PlatformError::invalid(
                "expires_at",
                "refresh token expiration must be after creation time",
            ));
        }
        Ok(Self {
            id,
            tenant_id,
            user_id,
            family_id,
            token_hash,
            session_version,
            used,
            expires_at,
            created_at,
        })
    }
}
