//! API key use cases.

use base64ct::{Base64UrlUnpadded, Encoding};
use domain_identity::api_key::ApiKey;
use foundation::{
    Clock, ErrorCode, PlatformError, RandomSource, RequestContext, UserId, UtcTimestamp, uuid::Uuid,
};
use sha2::{Digest, Sha256};
use storage_api::ApiKeyRepository;

/// A newly created API key. The raw token is only available here.
#[derive(Debug, Clone)]
pub struct CreatedApiKey {
    /// Persisted API key aggregate.
    pub api_key: ApiKey,
    /// Raw token shown once to the caller.
    pub raw_token: String,
}

/// Create a new API key for `owner_id`. The raw token is returned once;
/// only its hash is persisted.
#[allow(clippy::too_many_arguments)]
pub async fn create_api_key(
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
    let mut id_bytes = [0u8; 16];
    random.fill_bytes(&mut id_bytes)?;
    let id = Uuid::from_bytes(id_bytes);

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
    );
    repo.create(&api_key, ctx).await?;

    Ok(CreatedApiKey { api_key, raw_token })
}

/// Verify a raw API key for `scope` from `source` at `now`.
/// Records usage on success.
pub async fn verify_api_key(
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
    api_key.record_usage(now);
    repo.update(&api_key, ctx).await?;
    Ok(api_key)
}

/// Revoke an API key by id.
pub async fn revoke_api_key(
    repo: &dyn ApiKeyRepository,
    id: Uuid,
    clock: &dyn Clock,
    ctx: &RequestContext,
) -> Result<(), PlatformError> {
    let mut key = repo.by_id(id, ctx).await?;
    key.revoke(clock.now());
    repo.update(&key, ctx).await
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
