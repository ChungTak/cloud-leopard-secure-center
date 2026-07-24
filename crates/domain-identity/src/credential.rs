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
#[derive(Clone)]
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

impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credential")
            .field("tenant_id", &self.tenant_id)
            .field("user_id", &self.user_id)
            .field("credential_type", &self.credential_type)
            .field("value", &"<redacted>")
            .field("parameters", &self.parameters)
            .field("revision", &self.revision)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

impl Credential {
    /// Create a new password credential.
    pub fn new_password(
        tenant_id: TenantId,
        user_id: UserId,
        phc_hash: impl AsRef<str>,
        parameters: impl AsRef<str>,
        clock: &dyn foundation::Clock,
    ) -> Result<Self, PlatformError> {
        let value = phc_hash.as_ref();
        let parameters = parameters.as_ref();
        validate_credential_value(value)?;
        validate_credential_parameters(parameters)?;
        let value = value.to_string();
        let parameters = parameters.to_string();
        let now = clock.now();
        Ok(Self {
            tenant_id,
            user_id,
            credential_type: CredentialType::PasswordHash,
            value,
            parameters,
            revision: Revision::initial(),
            created_at: now,
            updated_at: now,
        })
    }

    /// Reconstruct a credential from persisted parts.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        tenant_id: TenantId,
        user_id: UserId,
        credential_type: CredentialType,
        value: impl AsRef<str>,
        parameters: impl AsRef<str>,
        revision: Revision,
        created_at: UtcTimestamp,
        updated_at: UtcTimestamp,
    ) -> Result<Self, PlatformError> {
        let value = value.as_ref();
        let parameters = parameters.as_ref();
        validate_credential_value(value)?;
        validate_credential_parameters(parameters)?;
        let value = value.to_string();
        let parameters = parameters.to_string();
        Ok(Self {
            tenant_id,
            user_id,
            credential_type,
            value,
            parameters,
            revision,
            created_at,
            updated_at,
        })
    }

    /// Replace the stored value and bump revision.
    pub fn rotate(
        &mut self,
        phc_hash: impl AsRef<str>,
        parameters: impl AsRef<str>,
        clock: &dyn foundation::Clock,
    ) -> Result<(), PlatformError> {
        let value = phc_hash.as_ref();
        let parameters = parameters.as_ref();
        validate_credential_value(value)?;
        validate_credential_parameters(parameters)?;
        let value = value.to_string();
        let parameters = parameters.to_string();
        self.value = value;
        self.parameters = parameters;
        self.updated_at = clock.now();
        self.revision = self.revision.next();
        Ok(())
    }
}

fn validate_credential_value(value: &str) -> Result<(), PlatformError> {
    if value.trim().is_empty() {
        return Err(PlatformError::invalid(
            "credential_value",
            "credential value must not be empty",
        ));
    }
    if value.len() > 1024 {
        return Err(PlatformError::invalid(
            "credential_value",
            "credential value must be at most 1024 characters",
        ));
    }
    Ok(())
}

fn validate_credential_parameters(parameters: &str) -> Result<(), PlatformError> {
    if parameters.trim().is_empty() {
        return Err(PlatformError::invalid(
            "credential_parameters",
            "credential parameters must not be empty",
        ));
    }
    if parameters.len() > 1024 {
        return Err(PlatformError::invalid(
            "credential_parameters",
            "credential parameters must be at most 1024 characters",
        ));
    }
    Ok(())
}
