//! Authentication port and token-backed implementation.

use async_trait::async_trait;
use domain_identity::user::{User, UserStatus};
use foundation::{Clock, ErrorCode, PlatformError, RequestContext, TenantId, UserId};
use std::sync::Arc;
use storage_api::{TenantRepository, UserRepository};

use crate::token_service::TokenService;

/// Convert repository errors during authentication into a safe response.
/// `NotFound`, `Invalid` and other user-side failures become `Unauthenticated`
/// so the caller cannot distinguish missing users from bad tokens.
/// `Unavailable` is preserved so load-balancers and clients can react to a
/// database outage instead of treating it as a credentials failure.
fn auth_error(e: PlatformError) -> PlatformError {
    if e.code() == ErrorCode::Unavailable {
        return e;
    }
    PlatformError::new(ErrorCode::Unauthenticated, "invalid token")
}

/// Authenticated actor context extracted from a valid access token.
#[derive(Debug, Clone)]
pub struct AuthContext {
    /// Authenticated user.
    pub user_id: UserId,
    /// Tenant scope carried by the token.
    pub tenant_id: TenantId,
    /// Session generation at issue time.
    pub session_version: u64,
    /// Token identifier.
    pub jti: String,
}

/// Verifies access tokens and turns them into an `AuthContext`.
#[async_trait]
pub trait Authenticator: Send + Sync {
    /// Authenticate the given raw token.
    async fn authenticate(&self, token: &str) -> Result<AuthContext, PlatformError>;
}

/// Token-backed authenticator that validates algorithm, claims, signature,
/// current session version, user status, and tenant lifecycle.
#[derive(Clone)]
pub struct TokenAuthenticator {
    token_service: TokenService,
    users: Arc<dyn UserRepository>,
    tenants: Arc<dyn TenantRepository>,
    clock: Arc<dyn Clock>,
}

impl std::fmt::Debug for TokenAuthenticator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenAuthenticator").finish_non_exhaustive()
    }
}

impl TokenAuthenticator {
    /// Create a token authenticator.
    pub fn new(
        token_service: TokenService,
        users: Arc<dyn UserRepository>,
        tenants: Arc<dyn TenantRepository>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            token_service,
            users,
            tenants,
            clock,
        }
    }

    /// Determine whether the user referenced by the token is allowed to authenticate.
    fn user_is_valid(&self, user: &User) -> Result<(), PlatformError> {
        if user.deleted_at.is_some() || user.status != UserStatus::Active {
            return Err(PlatformError::new(
                ErrorCode::Unauthenticated,
                "invalid token",
            ));
        }
        Ok(())
    }
}

#[async_trait]
impl Authenticator for TokenAuthenticator {
    async fn authenticate(&self, token: &str) -> Result<AuthContext, PlatformError> {
        // Verify the token's signature and core claims before touching the user
        // repository, so forged or expired tokens cannot be used to probe users.
        let claims = self
            .token_service
            .verify_access_token_claims(token, self.clock.now())?;

        let ctx = RequestContext {
            tenant_id: Some(claims.tenant_id),
            ..Default::default()
        };

        let user = self
            .users
            .by_id(claims.sub, &ctx)
            .await
            .map_err(auth_error)?;
        self.user_is_valid(&user)?;

        if claims.session_version != user.session_version {
            return Err(PlatformError::new(
                ErrorCode::Unauthenticated,
                "invalid token",
            ));
        }
        if claims.tenant_id != user.tenant_id {
            return Err(PlatformError::new(
                ErrorCode::Unauthenticated,
                "invalid token",
            ));
        }

        let tenant = self
            .tenants
            .by_id(user.tenant_id, &ctx)
            .await
            .map_err(auth_error)?;
        if !tenant.allows_new_sessions() {
            return Err(PlatformError::new(
                ErrorCode::Unauthenticated,
                "invalid token",
            ));
        }

        Ok(AuthContext {
            user_id: user.id,
            tenant_id: user.tenant_id,
            session_version: user.session_version,
            jti: claims.jti,
        })
    }
}

/// Authenticator that always rejects. Useful for tests and public routes.
#[derive(Debug, Clone, Copy)]
pub struct RejectingAuthenticator;

#[async_trait]
impl Authenticator for RejectingAuthenticator {
    async fn authenticate(&self, _token: &str) -> Result<AuthContext, PlatformError> {
        Err(PlatformError::new(
            ErrorCode::Unauthenticated,
            "invalid token",
        ))
    }
}

/// Authenticator that trusts a single token for testing.
#[derive(Debug, Clone)]
pub struct FakeAuthenticator {
    pub token: String,
    pub context: AuthContext,
}

#[async_trait]
impl Authenticator for FakeAuthenticator {
    async fn authenticate(&self, token: &str) -> Result<AuthContext, PlatformError> {
        if token == self.token {
            Ok(self.context.clone())
        } else {
            Err(PlatformError::new(
                ErrorCode::Unauthenticated,
                "invalid token",
            ))
        }
    }
}

/// Build a `RequestContext` with the actor and tenant from the authenticated context.
pub fn ctx_with_auth(
    base: RequestContext,
    auth: &AuthContext,
    deadline: Option<foundation::Deadline>,
) -> RequestContext {
    let ctx = base.with_actor(auth.user_id).with_tenant(auth.tenant_id);
    match deadline {
        Some(d) => ctx.with_deadline(d),
        None => ctx,
    }
}

/// Helper to create an `AuthContext` for tests.
pub fn test_auth_context(user_id: UserId, tenant_id: TenantId) -> AuthContext {
    AuthContext {
        user_id,
        tenant_id,
        session_version: 1,
        jti: "test-jti".to_string(),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use base64ct::Encoding;
    use foundation::{
        FakeClock, SystemClock, SystemIdGenerator, SystemRandom, UtcTimestamp, chrono::TimeZone,
    };

    #[test]
    fn token_service_issues_and_verifies_with_nbf() {
        let service = TokenService::new(
            b"a-very-secret-key-of-at-least-32-bytes",
            "clsc",
            "clsc-api",
            3600,
        )
        .expect("valid secret");
        let id_gen = SystemIdGenerator::new(SystemClock, SystemRandom);
        let user_id = UserId::generate(&id_gen).expect("generate user id");
        let tenant_id = TenantId::generate(&id_gen).expect("generate tenant id");
        let token = service
            .issue_access_token(user_id, tenant_id, 1, SystemClock.now(), "jti")
            .expect("issue");
        let claims = service
            .verify_access_token(&token, SystemClock.now(), 1)
            .expect("verify");
        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.nbf, SystemClock.now().timestamp_millis() / 1000);
    }

    #[test]
    fn algorithm_confusion_is_rejected() {
        let service = TokenService::new(
            b"a-very-secret-key-of-at-least-32-bytes",
            "clsc",
            "clsc-api",
            3600,
        )
        .expect("valid secret");
        let id_gen = SystemIdGenerator::new(SystemClock, SystemRandom);
        let user_id = UserId::generate(&id_gen).expect("generate user id");
        let tenant_id = TenantId::generate(&id_gen).expect("generate tenant id");
        let mut token = service
            .issue_access_token(user_id, tenant_id, 1, SystemClock.now(), "jti")
            .expect("issue");
        let parts: Vec<&str> = token.split('.').collect();
        let evil_header =
            base64ct::Base64UrlUnpadded::encode_string(br#"{"alg":"none","typ":"JWT"}"#);
        token = format!("{}.{}.{}", evil_header, parts[1], parts[2]);
        assert!(
            service
                .verify_access_token(&token, SystemClock.now(), 1)
                .is_err()
        );
    }

    #[test]
    fn nbf_in_the_future_is_rejected() {
        let service = TokenService::new(
            b"a-very-secret-key-of-at-least-32-bytes",
            "clsc",
            "clsc-api",
            3600,
        )
        .expect("valid secret");
        let id_gen = SystemIdGenerator::new(SystemClock, SystemRandom);
        let user_id = UserId::generate(&id_gen).expect("generate user id");
        let tenant_id = TenantId::generate(&id_gen).expect("generate tenant id");
        let token = service
            .issue_access_token(user_id, tenant_id, 1, SystemClock.now(), "jti")
            .expect("issue");
        let clock = FakeClock::from_millis(0);
        assert!(service.verify_access_token(&token, clock.now(), 1).is_err());
    }

    #[test]
    fn expired_token_is_rejected() {
        let service = TokenService::new(
            b"a-very-secret-key-of-at-least-32-bytes",
            "clsc",
            "clsc-api",
            3600,
        )
        .expect("valid secret");
        let id_gen = SystemIdGenerator::new(SystemClock, SystemRandom);
        let user_id = UserId::generate(&id_gen).expect("generate user id");
        let tenant_id = TenantId::generate(&id_gen).expect("generate tenant id");
        let now = UtcTimestamp::from(foundation::chrono::Utc.timestamp_opt(1000, 0).unwrap());
        let token = service
            .issue_access_token(user_id, tenant_id, 1, now, "jti")
            .expect("issue");
        let later = UtcTimestamp::from(foundation::chrono::Utc.timestamp_opt(10000, 0).unwrap());
        assert!(service.verify_access_token(&token, later, 1).is_err());
    }

    #[test]
    fn wrong_issuer_or_audience_is_rejected() {
        let issuer = TokenService::new(
            b"a-very-secret-key-of-at-least-32-bytes",
            "clsc",
            "clsc-api",
            3600,
        )
        .expect("valid secret");
        let verifier = TokenService::new(
            b"a-very-secret-key-of-at-least-32-bytes",
            "other-issuer",
            "other-audience",
            3600,
        )
        .expect("valid secret");
        let id_gen = SystemIdGenerator::new(SystemClock, SystemRandom);
        let user_id = UserId::generate(&id_gen).expect("generate user id");
        let tenant_id = TenantId::generate(&id_gen).expect("generate tenant id");
        let token = issuer
            .issue_access_token(user_id, tenant_id, 1, SystemClock.now(), "jti")
            .expect("issue");
        assert!(
            verifier
                .verify_access_token(&token, SystemClock.now(), 1)
                .is_err()
        );
    }

    #[test]
    fn wrong_session_version_is_rejected() {
        let service = TokenService::new(
            b"a-very-secret-key-of-at-least-32-bytes",
            "clsc",
            "clsc-api",
            3600,
        )
        .expect("valid secret");
        let id_gen = SystemIdGenerator::new(SystemClock, SystemRandom);
        let user_id = UserId::generate(&id_gen).expect("generate user id");
        let tenant_id = TenantId::generate(&id_gen).expect("generate tenant id");
        let token = service
            .issue_access_token(user_id, tenant_id, 1, SystemClock.now(), "jti")
            .expect("issue");
        assert!(
            service
                .verify_access_token(&token, SystemClock.now(), 2)
                .is_err()
        );
    }

    #[test]
    fn tampered_signature_is_rejected() {
        let service = TokenService::new(
            b"a-very-secret-key-of-at-least-32-bytes",
            "clsc",
            "clsc-api",
            3600,
        )
        .expect("valid secret");
        let id_gen = SystemIdGenerator::new(SystemClock, SystemRandom);
        let user_id = UserId::generate(&id_gen).expect("generate user id");
        let tenant_id = TenantId::generate(&id_gen).expect("generate tenant id");
        let token = service
            .issue_access_token(user_id, tenant_id, 1, SystemClock.now(), "jti")
            .expect("issue");
        let mut parts: Vec<char> = token.chars().collect();
        if let Some(last) = parts.last_mut() {
            *last = if *last == 'a' { 'b' } else { 'a' };
        }
        let tampered: String = parts.into_iter().collect();
        assert!(
            service
                .verify_access_token(&tampered, SystemClock.now(), 1)
                .is_err()
        );
    }

    #[test]
    fn rs256_algorithm_is_rejected() {
        let service = TokenService::new(
            b"a-very-secret-key-of-at-least-32-bytes",
            "clsc",
            "clsc-api",
            3600,
        )
        .expect("valid secret");
        let id_gen = SystemIdGenerator::new(SystemClock, SystemRandom);
        let user_id = UserId::generate(&id_gen).expect("generate user id");
        let tenant_id = TenantId::generate(&id_gen).expect("generate tenant id");
        let mut token = service
            .issue_access_token(user_id, tenant_id, 1, SystemClock.now(), "jti")
            .expect("issue");
        let segments: Vec<&str> = token.split('.').collect();
        let evil_header =
            base64ct::Base64UrlUnpadded::encode_string(br#"{"alg":"RS256","typ":"JWT"}"#);
        token = format!("{}.{}.{}", evil_header, segments[1], segments[2]);
        assert!(
            service
                .verify_access_token(&token, SystemClock.now(), 1)
                .is_err()
        );
    }

    #[test]
    fn empty_jti_is_rejected_at_issue() {
        let service = TokenService::new(
            b"a-very-secret-key-of-at-least-32-bytes",
            "clsc",
            "clsc-api",
            3600,
        )
        .expect("valid secret");
        let id_gen = SystemIdGenerator::new(SystemClock, SystemRandom);
        let user_id = UserId::generate(&id_gen).expect("generate user id");
        let tenant_id = TenantId::generate(&id_gen).expect("generate tenant id");
        assert!(
            service
                .issue_access_token(user_id, tenant_id, 1, SystemClock.now(), "")
                .is_err()
        );
    }
}
