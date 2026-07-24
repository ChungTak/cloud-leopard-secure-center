//! Session and refresh token use cases.

use crate::token_service::TokenService;
use base64ct::Encoding;
use domain_identity::credential::CredentialType;
use domain_identity::password::Argon2idPasswordHasher;
use domain_identity::user::{User, UserStatus};
use foundation::{
    Clock, ErrorCode, PlatformError, RandomSource, RequestContext, UserId, UtcTimestamp,
};
use storage_api::{CredentialRepository, SessionRepository, TenantRepository, UserRepository};

/// A freshly issued token pair.
#[derive(Debug, Clone)]
pub struct TokenPair {
    /// JWT access token.
    pub access_token: String,
    /// Opaque refresh token.
    pub refresh_token: String,
}

/// Issue a new access/refresh token pair for an authenticated user.
pub async fn issue_token_pair(
    sessions: &dyn SessionRepository,
    token_service: &TokenService,
    random: &dyn RandomSource,
    clock: &dyn Clock,
    ctx: &RequestContext,
    user: &User,
    refresh_ttl: UtcTimestamp,
) -> Result<TokenPair, PlatformError> {
    let access_token = token_service.issue_access_token(
        user.id,
        user.tenant_id,
        user.session_version,
        clock.now(),
        generate_jti(random)?,
    )?;

    let family_id = foundation::generate_uuid(clock, random)?;
    let (refresh_token, stored) = token_service.generate_refresh_token(
        user.tenant_id,
        user.id,
        family_id,
        user.session_version,
        refresh_ttl,
        random,
        clock,
    )?;
    sessions.save_refresh_token(&stored, ctx).await?;

    Ok(TokenPair {
        access_token,
        refresh_token,
    })
}

/// Refresh a token pair. Detects refresh token replay and revokes the entire
/// family when a used token is presented again.
#[allow(clippy::too_many_arguments)]
pub async fn refresh_token_pair(
    users: &dyn UserRepository,
    sessions: &dyn SessionRepository,
    tenants: &dyn TenantRepository,
    token_service: &TokenService,
    random: &dyn RandomSource,
    clock: &dyn Clock,
    ctx: &RequestContext,
    raw_refresh_token: &str,
    refresh_ttl: UtcTimestamp,
) -> Result<TokenPair, PlatformError> {
    let hash = TokenService::hash_refresh_token(raw_refresh_token);
    let token = match sessions.find_refresh_token_by_hash(&hash, ctx).await? {
        Some(t) => t,
        None => {
            return Err(PlatformError::new(
                ErrorCode::Unauthenticated,
                "invalid token",
            ));
        }
    };

    if token.expires_at <= clock.now() {
        return Err(PlatformError::new(
            ErrorCode::Unauthenticated,
            "invalid token",
        ));
    }

    if token.used {
        sessions.revoke_family(token.family_id, ctx).await?;
        return Err(PlatformError::new(
            ErrorCode::Unauthenticated,
            "invalid token",
        ));
    }

    let user = match users.by_id(token.user_id, ctx).await {
        Ok(u) => u,
        Err(_) => {
            sessions.revoke_family(token.family_id, ctx).await?;
            return Err(PlatformError::new(
                ErrorCode::Unauthenticated,
                "invalid token",
            ));
        }
    };

    if user.deleted_at.is_some()
        || user.session_version != token.session_version
        || user.status != UserStatus::Active
    {
        sessions.revoke_family(token.family_id, ctx).await?;
        return Err(PlatformError::new(
            ErrorCode::Unauthenticated,
            "invalid token",
        ));
    }

    let tenant_ctx = RequestContext {
        tenant_id: Some(user.tenant_id),
        ..Default::default()
    };
    let tenant = match tenants.by_id(user.tenant_id, &tenant_ctx).await {
        Ok(t) => t,
        Err(_) => {
            sessions.revoke_family(token.family_id, ctx).await?;
            return Err(PlatformError::new(
                ErrorCode::Unauthenticated,
                "invalid token",
            ));
        }
    };
    if !tenant.allows_new_sessions() {
        sessions.revoke_family(token.family_id, ctx).await?;
        return Err(PlatformError::new(
            ErrorCode::Unauthenticated,
            "invalid token",
        ));
    }

    match sessions.mark_refresh_token_used(&token, ctx).await {
        Ok(()) => {}
        Err(PlatformError::Conflict) => {
            sessions.revoke_family(token.family_id, ctx).await?;
            return Err(PlatformError::new(
                ErrorCode::Unauthenticated,
                "invalid token",
            ));
        }
        Err(e) => return Err(e),
    }

    let access_token = token_service.issue_access_token(
        user.id,
        user.tenant_id,
        user.session_version,
        clock.now(),
        generate_jti(random)?,
    )?;

    let (new_refresh, stored) = token_service.generate_refresh_token(
        user.tenant_id,
        user.id,
        token.family_id,
        user.session_version,
        refresh_ttl,
        random,
        clock,
    )?;
    sessions.save_refresh_token(&stored, ctx).await?;

    Ok(TokenPair {
        access_token,
        refresh_token: new_refresh,
    })
}

/// Change the user's password after verifying the current one.
#[allow(clippy::too_many_arguments)]
pub async fn change_password(
    users: &dyn UserRepository,
    credentials: &dyn CredentialRepository,
    sessions: &dyn SessionRepository,
    hasher: &Argon2idPasswordHasher,
    clock: &dyn Clock,
    ctx: &RequestContext,
    user_id: UserId,
    old_password: &str,
    new_password: &str,
) -> Result<(), PlatformError> {
    let mut user = users.by_id(user_id, ctx).await?;
    if user.deleted_at.is_some() || user.status != UserStatus::Active {
        return Err(PlatformError::new(
            ErrorCode::Unauthenticated,
            "invalid credentials",
        ));
    }

    let mut credential = credentials
        .by_user_and_type(user.id, CredentialType::PasswordHash.as_str(), ctx)
        .await?;

    if !hasher
        .verify(old_password, &credential.value)
        .unwrap_or(false)
    {
        return Err(PlatformError::new(
            ErrorCode::Unauthenticated,
            "invalid credentials",
        ));
    }

    let new_hash = hasher.hash(new_password)?;
    let expected = credential.revision;
    credential.rotate(new_hash, "argon2id", clock)?;
    credentials.update(&credential, expected, ctx).await?;

    let expected_user = user.revision;
    user.bump_session_version(clock, ctx.actor_id)?;
    users.update(&user, expected_user, ctx).await?;
    sessions.revoke_user_sessions(user_id, ctx).await?;

    Ok(())
}

/// Disable a user and invalidate all their sessions.
pub async fn disable_user(
    users: &dyn UserRepository,
    sessions: &dyn SessionRepository,
    clock: &dyn Clock,
    ctx: &RequestContext,
    user_id: UserId,
) -> Result<(), PlatformError> {
    let mut user = users.by_id(user_id, ctx).await?;
    let expected = user.revision;
    user.disable(clock, ctx.actor_id)?;
    user.bump_session_version(clock, ctx.actor_id)?;
    users.update(&user, expected, ctx).await?;
    sessions.revoke_user_sessions(user_id, ctx).await?;
    Ok(())
}

/// Log a user out by invalidating the current session generation.
pub async fn logout(
    users: &dyn UserRepository,
    sessions: &dyn SessionRepository,
    clock: &dyn Clock,
    ctx: &RequestContext,
    user_id: UserId,
) -> Result<(), PlatformError> {
    let mut user = users.by_id(user_id, ctx).await?;
    let expected = user.revision;
    user.bump_session_version(clock, ctx.actor_id)?;
    users.update(&user, expected, ctx).await?;
    sessions.revoke_user_sessions(user_id, ctx).await?;
    Ok(())
}

fn generate_jti(random: &dyn RandomSource) -> Result<String, PlatformError> {
    let mut bytes = [0u8; 16];
    random.fill_bytes(&mut bytes)?;
    Ok(base64ct::Base64UrlUnpadded::encode_string(&bytes))
}
