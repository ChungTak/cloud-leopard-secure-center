//! Authenticate use case.

use domain_identity::auth::{AuthenticationPolicy, AuthenticationResult};
use domain_identity::password::Argon2idPasswordHasher;
use domain_identity::user::{User, UserStatus, normalize_username};
use foundation::{ErrorCode, PlatformError, RequestContext};
use std::net::IpAddr;
use storage_api::{CredentialRepository, LoginAttemptRepository, TenantRepository, UserRepository};

/// Authenticate a user with username and password, recording login attempts
/// and enforcing rate/lockout policy.
#[allow(clippy::too_many_arguments)]
pub async fn authenticate(
    users: &dyn UserRepository,
    credentials: &dyn CredentialRepository,
    attempts: &dyn LoginAttemptRepository,
    tenants: &dyn TenantRepository,
    hasher: &Argon2idPasswordHasher,
    policy: &AuthenticationPolicy,
    ctx: &RequestContext,
    username: &str,
    password: &str,
    ip: Option<IpAddr>,
) -> Result<AuthenticationResult, PlatformError> {
    let tenant_id = ctx
        .tenant_id
        .ok_or_else(|| PlatformError::new(ErrorCode::Unauthenticated, "invalid credentials"))?;

    let ip_string = ip.as_ref().map(|ip| ip.to_string());

    let normalized_username = match normalize_username(username) {
        Ok(u) => u,
        Err(_) => {
            attempts
                .record(tenant_id, username, ip_string, false, ctx)
                .await?;
            return Ok(AuthenticationResult::InvalidCredentials);
        }
    };

    let user_result = users.by_username(&normalized_username, ctx).await;
    let user = match user_result {
        Ok(u) => u,
        Err(e) => {
            if e.code() == ErrorCode::Unavailable {
                return Err(e);
            }
            attempts
                .record(tenant_id, &normalized_username, ip_string, false, ctx)
                .await?;
            return Ok(AuthenticationResult::InvalidCredentials);
        }
    };

    if user.deleted_at.is_some() || user.status != UserStatus::Active {
        attempts
            .record(
                tenant_id,
                &normalized_username,
                ip_string.clone(),
                false,
                ctx,
            )
            .await?;
        return Ok(AuthenticationResult::InvalidCredentials);
    }

    let tenant_ctx = RequestContext {
        tenant_id: Some(user.tenant_id),
        ..ctx.clone()
    };
    let tenant = match tenants.by_id(user.tenant_id, &tenant_ctx).await {
        Ok(t) => t,
        Err(e) => {
            if e.code() == ErrorCode::Unavailable {
                return Err(e);
            }
            attempts
                .record(
                    tenant_id,
                    &normalized_username,
                    ip_string.clone(),
                    false,
                    ctx,
                )
                .await?;
            return Ok(AuthenticationResult::InvalidCredentials);
        }
    };
    if !tenant.allows_new_sessions() {
        attempts
            .record(
                tenant_id,
                &normalized_username,
                ip_string.clone(),
                false,
                ctx,
            )
            .await?;
        return Ok(AuthenticationResult::InvalidCredentials);
    }

    let credential = match credentials
        .by_user_and_type(user.id, "password_hash", ctx)
        .await
    {
        Ok(c) => c,
        Err(e) => {
            if e.code() == ErrorCode::Unavailable {
                return Err(e);
            }
            attempts
                .record(
                    tenant_id,
                    &normalized_username,
                    ip_string.clone(),
                    false,
                    ctx,
                )
                .await?;

            let identity_count = attempts
                .count_failures_by_identity(
                    tenant_id,
                    &normalized_username,
                    policy.window_seconds,
                    ctx,
                )
                .await?;
            let source_count = if let Some(ip) = ip_string.clone() {
                attempts
                    .count_failures_by_source(tenant_id, ip, policy.window_seconds, ctx)
                    .await?
            } else {
                0
            };

            if policy.identity_locked(identity_count) || policy.source_locked(source_count) {
                lock_user(users, user, ctx).await?;
            }

            return Ok(AuthenticationResult::InvalidCredentials);
        }
    };

    match hasher.verify(password, &credential.value) {
        Ok(true) => {
            if hasher.needs_rehash(&credential.value).unwrap_or(false)
                && let Ok(new_hash) = hasher.hash(password)
            {
                let mut credential = credential;
                credential.rotate(new_hash, "argon2id", &foundation::SystemClock)?;
                let expected = credential.revision.prev();
                credentials.update(&credential, expected, ctx).await?;
            }
            attempts
                .record(
                    tenant_id,
                    &normalized_username,
                    ip_string.clone(),
                    true,
                    ctx,
                )
                .await?;
            Ok(AuthenticationResult::Authenticated)
        }
        Ok(false) => {
            attempts
                .record(
                    tenant_id,
                    &normalized_username,
                    ip_string.clone(),
                    false,
                    ctx,
                )
                .await?;

            let identity_count = attempts
                .count_failures_by_identity(
                    tenant_id,
                    &normalized_username,
                    policy.window_seconds,
                    ctx,
                )
                .await?;
            let source_count = if let Some(ip) = ip_string.clone() {
                attempts
                    .count_failures_by_source(tenant_id, ip, policy.window_seconds, ctx)
                    .await?
            } else {
                0
            };

            if policy.identity_locked(identity_count) || policy.source_locked(source_count) {
                lock_user(users, user, ctx).await?;
            }

            Ok(AuthenticationResult::InvalidCredentials)
        }
        Err(_) => {
            attempts
                .record(
                    tenant_id,
                    &normalized_username,
                    ip_string.clone(),
                    false,
                    ctx,
                )
                .await?;

            let identity_count = attempts
                .count_failures_by_identity(
                    tenant_id,
                    &normalized_username,
                    policy.window_seconds,
                    ctx,
                )
                .await?;
            let source_count = if let Some(ip) = ip_string {
                attempts
                    .count_failures_by_source(tenant_id, ip, policy.window_seconds, ctx)
                    .await?
            } else {
                0
            };

            if policy.identity_locked(identity_count) || policy.source_locked(source_count) {
                lock_user(users, user, ctx).await?;
            }

            Ok(AuthenticationResult::InvalidCredentials)
        }
    }
}

async fn lock_user(
    users: &dyn UserRepository,
    mut user: User,
    ctx: &RequestContext,
) -> Result<(), PlatformError> {
    let expected = user.revision;
    user.lock(&foundation::SystemClock, ctx.actor_id)?;
    users.update(&user, expected, ctx).await
}
