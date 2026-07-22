//! Storage port traits (repository contract, unit of work).

use async_trait::async_trait;
use domain_identity::tenant::Tenant;
use domain_identity::user::User;
use foundation::{PlatformError, RequestContext, Revision, TenantId, UserId};

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

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[allow(dead_code)]
fn touch_dependencies() {
    let _v_foundation = foundation::version();
    let _v_domain_identity = domain_identity::version();
}
