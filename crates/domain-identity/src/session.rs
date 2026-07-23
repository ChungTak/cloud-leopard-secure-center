//! Session and refresh token primitives.

use foundation::{TenantId, UserId, UtcTimestamp, uuid::Uuid};

/// A stored refresh token. The raw token value is never kept here;
/// only its hash is persisted for reuse detection.
#[derive(Debug, Clone)]
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
