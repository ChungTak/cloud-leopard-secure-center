//! Role application use cases.

use async_trait::async_trait;
use domain_audit::audit_record::ActionRisk;
use domain_authorization::permission::Permission;
use domain_authorization::role::Role;
use domain_authorization::role_binding::ResourceRef;
use foundation::{
    Clock, IdGenerator, PlatformError, RequestContext, Revision, RoleId, TenantId, uuid::Uuid,
};
use storage_api::{AuditWriter, ListOptions, Page, RoleRepository};

use crate::authorization::AuthorizationPort;
use crate::usecase::{self, WriteRequest, WriteResponse};

/// Stable DTO for a role.
#[derive(Debug, Clone)]
pub struct RoleDto {
    pub id: RoleId,
    pub tenant_id: Option<TenantId>,
    pub name: String,
    pub is_builtin: bool,
    pub permissions: Vec<String>,
    pub revision: Revision,
}

impl From<&Role> for RoleDto {
    fn from(r: &Role) -> Self {
        Self {
            id: r.id,
            tenant_id: r.tenant_id,
            name: r.name.clone(),
            is_builtin: r.is_builtin,
            permissions: r.permissions.clone(),
            revision: r.revision,
        }
    }
}

/// Request to create a role.
#[derive(Debug, Clone)]
pub struct CreateRoleRequest {
    pub tenant_id: Option<TenantId>,
    pub name: String,
    pub is_builtin: bool,
    pub permissions: Vec<String>,
}

/// Request to update a role.
#[derive(Debug, Clone)]
pub struct UpdateRoleRequest {
    pub id: RoleId,
    pub name: String,
}

/// Request to grant a permission.
#[derive(Debug, Clone)]
pub struct GrantPermissionRequest {
    pub role_id: RoleId,
    pub permission: String,
}

/// Request to revoke a permission.
#[derive(Debug, Clone)]
pub struct RevokePermissionRequest {
    pub role_id: RoleId,
    pub permission: String,
}

/// Port for role application use cases.
#[async_trait]
pub trait RoleUseCase: Send + Sync {
    async fn create(
        &self,
        request: WriteRequest<CreateRoleRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<RoleDto>, PlatformError>;

    async fn update(
        &self,
        request: WriteRequest<UpdateRoleRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<RoleDto>, PlatformError>;

    async fn grant_permission(
        &self,
        request: WriteRequest<GrantPermissionRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<RoleDto>, PlatformError>;

    async fn revoke_permission(
        &self,
        request: WriteRequest<RevokePermissionRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<RoleDto>, PlatformError>;

    async fn get(&self, id: RoleId, ctx: &RequestContext) -> Result<RoleDto, PlatformError>;

    async fn list(
        &self,
        ctx: &RequestContext,
        options: ListOptions,
    ) -> Result<Page<RoleDto>, PlatformError>;
}

/// Default role application service.
#[derive(Debug, Clone)]
pub struct RoleService<R, A, U, C, I> {
    repo: R,
    auth: A,
    audit: U,
    clock: C,
    id_gen: I,
}

impl<R, A, U, C, I> RoleService<R, A, U, C, I> {
    pub fn new(repo: R, auth: A, audit: U, clock: C, id_gen: I) -> Self {
        Self {
            repo,
            auth,
            audit,
            clock,
            id_gen,
        }
    }
}

fn parse_permissions(permissions: Vec<String>) -> Result<Vec<Permission>, PlatformError> {
    permissions
        .into_iter()
        .map(|p| Permission::parse(&p))
        .collect()
}

fn auth_for_role(
    actor: foundation::UserId,
    tenant_id: Option<TenantId>,
    action: &'static str,
) -> usecase::AuthorizationRequest {
    match tenant_id {
        Some(t) => usecase::tenant_authorization(actor, t, action, ResourceRef::User(actor)),
        None => usecase::platform_authorization(actor, action),
    }
}

#[async_trait]
impl<R, A, U, C, I> RoleUseCase for RoleService<R, A, U, C, I>
where
    R: RoleRepository,
    A: AuthorizationPort,
    U: AuditWriter,
    C: Clock,
    I: IdGenerator,
{
    async fn create(
        &self,
        request: WriteRequest<CreateRoleRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<RoleDto>, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let tenant_id = request.payload.tenant_id;
        let action = if tenant_id.is_some() {
            "tenant:role:write"
        } else {
            "platform:role:write"
        };
        let auth_req = auth_for_role(actor, tenant_id, action);
        usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        let id = RoleId::generate(&self.id_gen)?;
        let permissions = parse_permissions(request.payload.permissions)?;
        let role = Role::new(
            id,
            tenant_id,
            request.payload.name,
            request.payload.is_builtin,
            permissions,
            &self.clock,
            Some(actor),
        )?;

        self.repo.create(&role, ctx).await?;

        let audit_tenant = tenant_id.unwrap_or(TenantId::from_uuid(Uuid::nil()));
        usecase::audit_write(
            &self.audit,
            audit_tenant,
            "user",
            actor.to_hyphenated(),
            "role.create",
            "role",
            id.to_hyphenated(),
            ActionRisk::High,
            serde_json::json!({"role_id": id.to_hyphenated(), "tenant_id": tenant_id.map(|t| t.to_hyphenated())}),
            &self.clock,
            ctx,
        ).await?;

        Ok(WriteResponse::new(RoleDto::from(&role), role.revision))
    }

    async fn update(
        &self,
        request: WriteRequest<UpdateRoleRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<RoleDto>, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let expected = request.expected_revision.ok_or_else(|| {
            PlatformError::invalid("expected_revision", "revision is required for updates")
        })?;

        let mut role = self.repo.by_id(request.payload.id, ctx).await?;
        let role_id = role.id;
        let tenant_id = role.tenant_id;
        let action = if tenant_id.is_some() {
            "tenant:role:write"
        } else {
            "platform:role:write"
        };
        let auth_req = auth_for_role(actor, tenant_id, action);
        usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        role.rename(request.payload.name, &self.clock, Some(actor))?;

        self.repo.update(&role, expected, ctx).await?;

        let audit_tenant = tenant_id.unwrap_or(TenantId::from_uuid(Uuid::nil()));
        usecase::audit_write(
            &self.audit,
            audit_tenant,
            "user",
            actor.to_hyphenated(),
            "role.update",
            "role",
            role_id.to_hyphenated(),
            ActionRisk::Normal,
            serde_json::json!({"role_id": role_id.to_hyphenated()}),
            &self.clock,
            ctx,
        )
        .await?;

        Ok(WriteResponse::new(RoleDto::from(&role), role.revision))
    }

    async fn grant_permission(
        &self,
        request: WriteRequest<GrantPermissionRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<RoleDto>, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let expected = request.expected_revision.ok_or_else(|| {
            PlatformError::invalid(
                "expected_revision",
                "revision is required for permission changes",
            )
        })?;

        let mut role = self.repo.by_id(request.payload.role_id, ctx).await?;
        let role_id = role.id;
        let tenant_id = role.tenant_id;
        let action = if tenant_id.is_some() {
            "tenant:role:write"
        } else {
            "platform:role:write"
        };
        let auth_req = auth_for_role(actor, tenant_id, action);
        usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        let permission = Permission::parse(&request.payload.permission)?;
        role.grant_permission(permission, &self.clock, Some(actor))?;

        self.repo.update(&role, expected, ctx).await?;

        let audit_tenant = tenant_id.unwrap_or(TenantId::from_uuid(Uuid::nil()));
        usecase::audit_write(
            &self.audit,
            audit_tenant,
            "user",
            actor.to_hyphenated(),
            "role.grant_permission",
            "role",
            role_id.to_hyphenated(),
            ActionRisk::Critical,
            serde_json::json!({"role_id": role_id.to_hyphenated(), "permission": request.payload.permission}),
            &self.clock,
            ctx,
        ).await?;

        Ok(WriteResponse::new(RoleDto::from(&role), role.revision))
    }

    async fn revoke_permission(
        &self,
        request: WriteRequest<RevokePermissionRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<RoleDto>, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let expected = request.expected_revision.ok_or_else(|| {
            PlatformError::invalid(
                "expected_revision",
                "revision is required for permission changes",
            )
        })?;

        let mut role = self.repo.by_id(request.payload.role_id, ctx).await?;
        let role_id = role.id;
        let tenant_id = role.tenant_id;
        let action = if tenant_id.is_some() {
            "tenant:role:write"
        } else {
            "platform:role:write"
        };
        let auth_req = auth_for_role(actor, tenant_id, action);
        usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        role.revoke_permission(&request.payload.permission, &self.clock, Some(actor))?;

        self.repo.update(&role, expected, ctx).await?;

        let audit_tenant = tenant_id.unwrap_or(TenantId::from_uuid(Uuid::nil()));
        usecase::audit_write(
            &self.audit,
            audit_tenant,
            "user",
            actor.to_hyphenated(),
            "role.revoke_permission",
            "role",
            role_id.to_hyphenated(),
            ActionRisk::Critical,
            serde_json::json!({"role_id": role_id.to_hyphenated(), "permission": request.payload.permission}),
            &self.clock,
            ctx,
        ).await?;

        Ok(WriteResponse::new(RoleDto::from(&role), role.revision))
    }

    async fn get(&self, id: RoleId, ctx: &RequestContext) -> Result<RoleDto, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let tenant_id = ctx.tenant_id;
        let action = if tenant_id.is_some() {
            "tenant:role:read"
        } else {
            "platform:role:read"
        };
        let auth_req = auth_for_role(actor, tenant_id, action);
        usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        let role = self.repo.by_id(id, ctx).await?;
        Ok(RoleDto::from(&role))
    }

    async fn list(
        &self,
        ctx: &RequestContext,
        options: ListOptions,
    ) -> Result<Page<RoleDto>, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let tenant_id = ctx.tenant_id;
        let action = if tenant_id.is_some() {
            "tenant:role:read"
        } else {
            "platform:role:read"
        };
        let auth_req = auth_for_role(actor, tenant_id, action);
        usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        let page = self.repo.list(ctx, options).await?;
        Ok(Page {
            items: page.items.iter().map(RoleDto::from).collect(),
            next_cursor: page.next_cursor,
        })
    }
}
