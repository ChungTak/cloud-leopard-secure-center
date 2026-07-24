//! API key use cases.

use base64ct::{Base64UrlUnpadded, Encoding};
use domain_identity::api_key::ApiKey;
use domain_identity::user::UserStatus;
use foundation::{
    Clock, ErrorCode, PlatformError, RandomSource, RequestContext, UserId, UtcTimestamp, uuid::Uuid,
};
use sha2::{Digest, Sha256};
use storage_api::{ApiKeyRepository, TenantRepository, UserRepository};

/// A newly created API key. The raw token is only available here.
#[derive(Clone)]
pub struct CreatedApiKey {
    /// Persisted API key aggregate.
    pub api_key: ApiKey,
    /// Raw token shown once to the caller.
    pub raw_token: String,
}

impl std::fmt::Debug for CreatedApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreatedApiKey")
            .field("api_key", &self.api_key)
            .field("raw_token", &"<redacted>")
            .finish()
    }
}

/// Create a new API key for `owner_id`. The raw token is returned once;
/// only its hash is persisted.
#[allow(clippy::too_many_arguments)]
pub async fn create_api_key(
    users: &dyn UserRepository,
    repo: &dyn ApiKeyRepository,
    random: &dyn RandomSource,
    clock: &dyn Clock,
    ctx: &RequestContext,
    owner_id: UserId,
    name: impl Into<String>,
    scopes: Vec<String>,
    allowed_sources: Vec<String>,
    expires_at: UtcTimestamp,
) -> Result<CreatedApiKey, PlatformError> {
    // Verify the owner exists and is active in the current tenant before creating the key.
    let user = users.by_id(owner_id, ctx).await?;
    if user.deleted_at.is_some() || user.status != UserStatus::Active {
        return Err(PlatformError::invalid("owner_id", "user is not active"));
    }

    let id = foundation::generate_uuid(clock, random)?;

    let raw_token = generate_random_string(random, 32)?;
    let token_hash = hash_raw(&raw_token);

    let api_key = ApiKey::new(
        id,
        ctx.tenant_id.ok_or(PlatformError::new(
            ErrorCode::Unauthenticated,
            "missing tenant",
        ))?,
        owner_id,
        name,
        scopes,
        allowed_sources,
        token_hash,
        expires_at,
        clock.now(),
    )?;
    repo.create(&api_key, ctx).await?;

    Ok(CreatedApiKey { api_key, raw_token })
}

/// Verify a raw API key for `scope` from `source` at `now`.
/// Records usage on success.
#[allow(clippy::too_many_arguments)]
pub async fn verify_api_key(
    users: &dyn UserRepository,
    tenants: &dyn TenantRepository,
    repo: &dyn ApiKeyRepository,
    raw_token: &str,
    source: Option<&str>,
    scope: &str,
    now: UtcTimestamp,
    ctx: &RequestContext,
) -> Result<ApiKey, PlatformError> {
    let token_hash = hash_raw(raw_token);
    let mut api_key = match repo.by_token_hash(&token_hash, ctx).await? {
        Some(k) => k,
        None => {
            return Err(PlatformError::new(
                ErrorCode::Unauthenticated,
                "invalid api key",
            ));
        }
    };

    api_key.verify(source, scope, now)?;

    // Verify the owner is still active and belongs to this tenant before
    // recording usage; otherwise a key for a deleted/locked/disabled user or
    // another tenant could still authenticate.
    let mut owner_ctx = ctx.clone();
    owner_ctx.tenant_id = Some(api_key.tenant_id);
    let user = match users.by_id(api_key.owner_id, &owner_ctx).await {
        Ok(u) => u,
        Err(e) if e.code() == ErrorCode::NotFound => {
            return Err(PlatformError::new(
                ErrorCode::Unauthenticated,
                "invalid api key",
            ));
        }
        Err(e) => return Err(e),
    };
    if user.deleted_at.is_some()
        || user.status != UserStatus::Active
        || user.tenant_id != api_key.tenant_id
    {
        return Err(PlatformError::new(
            ErrorCode::Unauthenticated,
            "invalid api key",
        ));
    }

    let tenant = tenants.by_id(api_key.tenant_id, &owner_ctx).await?;
    if !tenant.allows_new_sessions() {
        return Err(PlatformError::new(
            ErrorCode::Unauthenticated,
            "invalid api key",
        ));
    }

    let recorded = repo.record_usage(&token_hash, now, ctx).await?;
    if !recorded {
        // The key was revoked or expired between the read and the write.
        return Err(PlatformError::new(
            ErrorCode::Unauthenticated,
            "api key is not valid",
        ));
    }
    api_key.record_usage(now);
    Ok(api_key)
}

/// Revoke an API key by id.
pub async fn revoke_api_key(
    repo: &dyn ApiKeyRepository,
    id: Uuid,
    clock: &dyn Clock,
    ctx: &RequestContext,
) -> Result<(), PlatformError> {
    let revoked_at = clock.now();
    let revoked = repo.revoke(id, revoked_at, ctx).await?;
    if !revoked {
        return Err(PlatformError::new(
            ErrorCode::NotFound,
            "api key not found or already revoked",
        ));
    }
    Ok(())
}

fn generate_random_string(random: &dyn RandomSource, len: usize) -> Result<String, PlatformError> {
    let mut bytes = vec![0u8; len];
    random.fill_bytes(&mut bytes)?;
    Ok(Base64UrlUnpadded::encode_string(&bytes))
}

fn hash_raw(raw: &str) -> String {
    let digest = Sha256::digest(raw.as_bytes());
    Base64UrlUnpadded::encode_string(&digest)
}
