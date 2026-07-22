//! Credential aggregate for user authentication secrets.

use foundation::{PlatformError, Revision, TenantId, UserId, UtcTimestamp};

/// Credential type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialType {
    /// Argon2id password hash.
    PasswordHash,
}

impl CredentialType {
    /// Parse a credential type string.
    pub fn parse(input: &str) -> Result<Self, PlatformError> {
        match input {
            "password_hash" => Ok(Self::PasswordHash),
            _ => Err(PlatformError::invalid("credential_type", "unknown type")),
        }
    }

    /// Return the canonical string representation.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::PasswordHash => "password_hash",
        }
    }
}

/// A stored credential for a user.
#[derive(Debug, Clone)]
pub struct Credential {
    /// Owning tenant.
    pub tenant_id: TenantId,
    /// User this credential belongs to.
    pub user_id: UserId,
    /// Credential type.
    pub credential_type: CredentialType,
    /// Stored credential value, e.g. a PHC string.
    pub value: String,
    /// Algorithm/version parameters stored alongside the value.
    pub parameters: String,
    /// Optimistic lock version.
    pub revision: Revision,
    /// Creation timestamp.
    pub created_at: UtcTimestamp,
    /// Last update timestamp.
    pub updated_at: UtcTimestamp,
}

impl Credential {
    /// Create a new password credential.
    pub fn new_password(
        tenant_id: TenantId,
        user_id: UserId,
        phc_hash: impl Into<String>,
        parameters: impl Into<String>,
        clock: &dyn foundation::Clock,
    ) -> Self {
        let now = clock.now();
        Self {
            tenant_id,
            user_id,
            credential_type: CredentialType::PasswordHash,
            value: phc_hash.into(),
            parameters: parameters.into(),
            revision: Revision::initial(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Replace the stored value and bump revision.
    pub fn rotate(
        &mut self,
        phc_hash: impl Into<String>,
        parameters: impl Into<String>,
        clock: &dyn foundation::Clock,
    ) {
        self.value = phc_hash.into();
        self.parameters = parameters.into();
        self.updated_at = clock.now();
        self.revision = self.revision.next();
    }
}
