//! Authorization port and default implementation.

use async_trait::async_trait;
use domain_authorization::permission::Permission;
use domain_authorization::role_binding::{ResourceRef, RoleBinding, Scope};
use foundation::{BindingId, Clock, PlatformError, RequestContext, TenantId, UserId, UtcTimestamp};
use storage_api::{
    OrganizationUnitRepository, RoleBindingRepository, RoleRepository, SpatialRepository,
};

/// Authorization decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Deny,
}

/// Machine-readable reason for an authorization decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    Allowed,
    NoBinding,
    Expired,
    PermissionNotGranted,
    ScopeMismatch,
}

/// Request to evaluate authorization.
#[derive(Debug, Clone)]
pub struct AuthorizationRequest {
    pub principal: UserId,
    pub tenant: TenantId,
    pub action: String,
    pub resource: ResourceRef,
    pub context: Option<serde_json::Value>,
}

/// Authorization response with decision and matched policy identifiers.
#[derive(Debug, Clone)]
pub struct AuthorizationResponse {
    pub decision: Decision,
    pub binding_ids: Vec<BindingId>,
    pub reason: Reason,
}

/// Port used by business modules to authorize actions on resources.
#[async_trait]
pub trait AuthorizationPort: Send + Sync {
    /// Evaluate whether `principal` may perform `action` on `resource`.
    async fn authorize(
        &self,
        req: AuthorizationRequest,
        ctx: &RequestContext,
    ) -> Result<AuthorizationResponse, PlatformError>;
}

/// Default authorization service using repository ports.
#[derive(Debug, Clone)]
pub struct AuthorizationService<R, B, O, S, C> {
    role_repo: R,
    binding_repo: B,
    organization_repo: O,
    spatial_repo: S,
    clock: C,
}

impl<R, B, O, S, C> AuthorizationService<R, B, O, S, C>
where
    R: RoleRepository,
    B: RoleBindingRepository,
    O: OrganizationUnitRepository,
    S: SpatialRepository,
    C: Clock,
{
    /// Build the authorization service from its dependencies.
    pub fn new(
        role_repo: R,
        binding_repo: B,
        organization_repo: O,
        spatial_repo: S,
        clock: C,
    ) -> Self {
        Self {
            role_repo,
            binding_repo,
            organization_repo,
            spatial_repo,
            clock,
        }
    }
}

#[async_trait]
impl<R, B, O, S, C> AuthorizationPort for AuthorizationService<R, B, O, S, C>
where
    R: RoleRepository,
    B: RoleBindingRepository,
    O: OrganizationUnitRepository,
    S: SpatialRepository,
    C: Clock,
{
    async fn authorize(
        &self,
        req: AuthorizationRequest,
        ctx: &RequestContext,
    ) -> Result<AuthorizationResponse, PlatformError> {
        let now = self.clock.now();

        if Permission::parse(&req.action).is_err() {
            return Ok(AuthorizationResponse {
                decision: Decision::Deny,
                binding_ids: vec![],
                reason: Reason::PermissionNotGranted,
            });
        }

        let page = self
            .binding_repo
            .list_by_principal(req.principal, ctx)
            .await?;

        let mut allowed = Vec::new();
        let mut expired = false;
        let mut permission_missing = false;
        let mut scope_mismatch = false;

        for binding in page.items {
            match evaluate(&self, &req, &binding, now, ctx).await? {
                Evaluation::Allow => allowed.push(binding.id),
                Evaluation::Expired => expired = true,
                Evaluation::PermissionMissing => permission_missing = true,
                Evaluation::ScopeMismatch => scope_mismatch = true,
            }
        }

        if !allowed.is_empty() {
            return Ok(AuthorizationResponse {
                decision: Decision::Allow,
                binding_ids: allowed,
                reason: Reason::Allowed,
            });
        }

        let reason = if scope_mismatch {
            Reason::ScopeMismatch
        } else if permission_missing {
            Reason::PermissionNotGranted
        } else if expired {
            Reason::Expired
        } else {
            Reason::NoBinding
        };

        Ok(AuthorizationResponse {
            decision: Decision::Deny,
            binding_ids: vec![],
            reason,
        })
    }
}

enum Evaluation {
    Allow,
    Expired,
    PermissionMissing,
    ScopeMismatch,
}

async fn evaluate<R, B, O, S, C>(
    service: &AuthorizationService<R, B, O, S, C>,
    req: &AuthorizationRequest,
    binding: &RoleBinding,
    now: UtcTimestamp,
    ctx: &RequestContext,
) -> Result<Evaluation, PlatformError>
where
    R: RoleRepository,
    B: RoleBindingRepository,
    O: OrganizationUnitRepository,
    S: SpatialRepository,
    C: Clock,
{
    if !binding.is_valid_at(now) {
        return Ok(Evaluation::Expired);
    }

    let role = service.role_repo.by_id(binding.role_id, ctx).await?;
    if !role.permissions.contains(&req.action) {
        return Ok(Evaluation::PermissionMissing);
    }

    if !matches_scope(service, &req.resource, &binding.scope, ctx).await? {
        return Ok(Evaluation::ScopeMismatch);
    }

    Ok(Evaluation::Allow)
}

async fn matches_scope<R, B, O, S, C>(
    service: &AuthorizationService<R, B, O, S, C>,
    resource: &ResourceRef,
    scope: &Scope,
    ctx: &RequestContext,
) -> Result<bool, PlatformError>
where
    R: RoleRepository,
    B: RoleBindingRepository,
    O: OrganizationUnitRepository,
    S: SpatialRepository,
    C: Clock,
{
    match scope {
        Scope::Tenant => Ok(true),
        Scope::OrganizationSubtree(ancestor) => {
            if let ResourceRef::Organization(descendant) = resource {
                service
                    .organization_repo
                    .is_descendant_of(*ancestor, *descendant, ctx)
                    .await
            } else {
                Ok(false)
            }
        }
        Scope::AreaSubtree(ancestor) => {
            if let ResourceRef::Area(descendant) = resource {
                service
                    .spatial_repo
                    .is_area_descendant_of(*ancestor, *descendant, ctx)
                    .await
            } else {
                Ok(false)
            }
        }
        Scope::ResourceSet(resources) => Ok(resources.contains(resource)),
    }
}
