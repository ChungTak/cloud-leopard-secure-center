//! Storage port traits (repository contract, unit of work).

use async_trait::async_trait;
use domain_identity::api_key::ApiKey;
use domain_identity::credential::Credential;
use domain_identity::mfa::MfaFactor;
use domain_identity::session::RefreshToken;
use domain_identity::user::User;
use domain_organization::tenant::Tenant;
use foundation::{PlatformError, RequestContext, Revision, TenantId, UserId, uuid::Uuid};

/// Page of results returned by a repository list query.
#[derive(Debug, Clone)]
pub struct Page<T> {
    /// Items in the page.
    pub items: Vec<T>,
    /// Opaque cursor for the next page, if any.
    pub next_cursor: Option<String>,
}

/// Unit of work boundary. Implementations wrap a transaction.
#[async_trait]
pub trait UnitOfWork: Send + Sync {
    /// Commit the transaction.
    async fn commit(self) -> Result<(), PlatformError>;
    /// Rollback the transaction.
    async fn rollback(self) -> Result<(), PlatformError>;
}

/// Repository contract for the `Tenant` aggregate.
///
/// All mutating methods take an `expected` revision and return
/// `REVISION_CONFLICT` or `NOT_FOUND` when the row is missing or stale.
#[async_trait]
pub trait TenantRepository: Send + Sync {
    /// Find a tenant by id, honoring the tenant context in `ctx`.
    async fn by_id(&self, id: TenantId, ctx: &RequestContext) -> Result<Tenant, PlatformError>;

    /// Persist a new tenant.
    async fn create(&self, tenant: &Tenant, ctx: &RequestContext) -> Result<(), PlatformError>;

    /// Update an existing tenant, failing if `expected` does not match.
    async fn update(
        &self,
        tenant: &Tenant,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Soft-delete a tenant by id when the current revision matches `expected`.
    async fn delete(
        &self,
        id: TenantId,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// List tenants visible in the current tenant context.
    async fn list(&self, ctx: &RequestContext) -> Result<Page<Tenant>, PlatformError>;
}

/// Repository contract for the `User` aggregate.
///
/// All mutating methods take an `expected` revision and return
/// `REVISION_CONFLICT` or `NOT_FOUND` when the row is missing or stale.
#[async_trait]
pub trait UserRepository: Send + Sync {
    /// Find a user by id, honoring the tenant context in `ctx`.
    async fn by_id(&self, id: UserId, ctx: &RequestContext) -> Result<User, PlatformError>;

    /// Find a user by normalized username.
    async fn by_username(
        &self,
        username: &str,
        ctx: &RequestContext,
    ) -> Result<User, PlatformError>;

    /// Persist a new user.
    async fn create(&self, user: &User, ctx: &RequestContext) -> Result<(), PlatformError>;

    /// Update an existing user, failing if `expected` does not match.
    async fn update(
        &self,
        user: &User,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Soft-delete a user by id when the current revision matches `expected`.
    async fn delete(
        &self,
        id: UserId,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// List users visible in the current tenant context.
    async fn list(&self, ctx: &RequestContext) -> Result<Page<User>, PlatformError>;
}

/// Repository contract for a user's `Credential`.
#[async_trait]
pub trait CredentialRepository: Send + Sync {
    /// Find the credential for a user by type.
    async fn by_user_and_type(
        &self,
        user_id: UserId,
        credential_type: &str,
        ctx: &RequestContext,
    ) -> Result<Credential, PlatformError>;

    /// Persist a new credential.
    async fn create(
        &self,
        credential: &Credential,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Update an existing credential.
    async fn update(
        &self,
        credential: &Credential,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Remove a user's credential.
    async fn delete(
        &self,
        user_id: UserId,
        credential_type: &str,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;
}

/// Repository contract for recording and querying login attempts.
#[async_trait]
pub trait LoginAttemptRepository: Send + Sync {
    /// Record a login attempt for audit and rate-limiting.
    async fn record(
        &self,
        tenant_id: TenantId,
        identity: &str,
        ip: Option<String>,
        success: bool,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Count recent failed attempts for the given identity.
    async fn count_failures_by_identity(
        &self,
        tenant_id: TenantId,
        identity: &str,
        window_seconds: i64,
        ctx: &RequestContext,
    ) -> Result<i64, PlatformError>;

    /// Count recent failed attempts from the given source IP.
    async fn count_failures_by_source(
        &self,
        tenant_id: TenantId,
        ip: String,
        window_seconds: i64,
        ctx: &RequestContext,
    ) -> Result<i64, PlatformError>;
}

/// Repository contract for sessions and refresh token families.
#[async_trait]
pub trait SessionRepository: Send + Sync {
    /// Persist a new refresh token.
    async fn save_refresh_token(
        &self,
        token: &RefreshToken,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Find a refresh token by its hash.
    async fn find_refresh_token_by_hash(
        &self,
        token_hash: &str,
        ctx: &RequestContext,
    ) -> Result<Option<RefreshToken>, PlatformError>;

    /// Mark a refresh token as used. Returns `Conflict` if already used.
    async fn mark_refresh_token_used(
        &self,
        token: &RefreshToken,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Revoke every token in the given family.
    async fn revoke_family(
        &self,
        family_id: Uuid,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Revoke all sessions for a user.
    async fn revoke_user_sessions(
        &self,
        user_id: UserId,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;
}

/// Repository contract for API keys.
#[async_trait]
pub trait ApiKeyRepository: Send + Sync {
    /// Persist a new API key.
    async fn create(&self, api_key: &ApiKey, ctx: &RequestContext) -> Result<(), PlatformError>;

    /// Find an API key by id.
    async fn by_id(&self, id: Uuid, ctx: &RequestContext) -> Result<ApiKey, PlatformError>;

    /// Find an API key by its token hash.
    async fn by_token_hash(
        &self,
        token_hash: &str,
        ctx: &RequestContext,
    ) -> Result<Option<ApiKey>, PlatformError>;

    /// Update an existing API key.
    async fn update(&self, api_key: &ApiKey, ctx: &RequestContext) -> Result<(), PlatformError>;

    /// List API keys for an owner.
    async fn list_by_owner(
        &self,
        owner_id: UserId,
        ctx: &RequestContext,
    ) -> Result<Page<ApiKey>, PlatformError>;
}

/// Repository contract for MFA factors.
#[async_trait]
pub trait MfaRepository: Send + Sync {
    /// Persist a new factor and its recovery codes.
    async fn save_factor(
        &self,
        factor: &MfaFactor,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Update an existing factor.
    async fn update_factor(
        &self,
        factor: &MfaFactor,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Find the active factor for a user, if any.
    async fn find_active_factor_by_user(
        &self,
        user_id: UserId,
        ctx: &RequestContext,
    ) -> Result<Option<MfaFactor>, PlatformError>;
}

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[allow(dead_code)]
fn touch_dependencies() {
    let _v_foundation = foundation::version();
    let _v_domain_identity = domain_identity::version();
}
