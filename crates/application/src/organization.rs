//! Organization application use cases.

use async_trait::async_trait;
use domain_audit::audit_record::ActionRisk;
use domain_authorization::role_binding::ResourceRef;
use domain_organization::organization_unit::OrganizationUnit;
use foundation::{
    Clock, ErrorCode, IdGenerator, OrganizationId, PlatformError, RequestContext, Revision,
    TenantId,
};
use storage_api::{AuditWriter, ListOptions, OrganizationUnitRepository, Page};

use crate::authorization::AuthorizationPort;
use crate::usecase::{self, WriteRequest, WriteResponse};

/// Stable DTO for an organization unit.
#[derive(Debug, Clone)]
pub struct OrganizationUnitDto {
    pub id: OrganizationId,
    pub tenant_id: TenantId,
    pub parent_id: Option<OrganizationId>,
    pub code: String,
    pub name: String,
    pub revision: Revision,
}

impl From<&OrganizationUnit> for OrganizationUnitDto {
    fn from(u: &OrganizationUnit) -> Self {
        Self {
            id: u.id,
            tenant_id: u.tenant_id,
            parent_id: u.parent_id,
            code: u.code.clone(),
            name: u.name.clone(),
            revision: u.revision,
        }
    }
}

/// Request to create an organization unit.
#[derive(Debug, Clone)]
pub struct CreateOrganizationUnitRequest {
    pub tenant_id: TenantId,
    pub parent_id: Option<OrganizationId>,
    pub code: String,
    pub name: String,
}

/// Request to update an organization unit.
#[derive(Debug, Clone)]
pub struct UpdateOrganizationUnitRequest {
    pub id: OrganizationId,
    pub name: String,
}

/// Port for organization application use cases.
#[async_trait]
pub trait OrganizationUseCase: Send + Sync {
    async fn create(
        &self,
        request: WriteRequest<CreateOrganizationUnitRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<OrganizationUnitDto>, PlatformError>;

    async fn update(
        &self,
        request: WriteRequest<UpdateOrganizationUnitRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<OrganizationUnitDto>, PlatformError>;

    async fn get(
        &self,
        id: OrganizationId,
        ctx: &RequestContext,
    ) -> Result<OrganizationUnitDto, PlatformError>;

    async fn list(
        &self,
        ctx: &RequestContext,
        options: ListOptions,
    ) -> Result<Page<OrganizationUnitDto>, PlatformError>;
}

/// Default organization application service.
#[derive(Debug, Clone)]
pub struct OrganizationService<R, A, U, C, I> {
    repo: R,
    auth: A,
    audit: U,
    clock: C,
    id_gen: I,
}

impl<R, A, U, C, I> OrganizationService<R, A, U, C, I> {
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

#[async_trait]
impl<R, A, U, C, I> OrganizationUseCase for OrganizationService<R, A, U, C, I>
where
    R: OrganizationUnitRepository,
    A: AuthorizationPort,
    U: AuditWriter,
    C: Clock,
    I: IdGenerator,
{
    async fn create(
        &self,
        request: WriteRequest<CreateOrganizationUnitRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<OrganizationUnitDto>, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let tenant_id = request.payload.tenant_id;

        if ctx.tenant_id != Some(tenant_id) {
            return Err(PlatformError::new(
                ErrorCode::Denied,
                "tenant scope mismatch",
            ));
        }

        let resource = request
            .payload
            .parent_id
            .map(ResourceRef::Organization)
            .unwrap_or(ResourceRef::User(actor));
        let auth_req =
            usecase::tenant_authorization(actor, tenant_id, "tenant:organization:write", resource);
        usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        let id = OrganizationId::generate(&self.id_gen)?;
        let unit = OrganizationUnit::new(
            id,
            tenant_id,
            request.payload.parent_id,
            request.payload.code,
            request.payload.name,
            &self.clock,
            Some(actor),
        )?;

        self.repo.create(&unit, ctx).await?;

        usecase::audit_write(
            &self.audit,
            tenant_id,
            "user",
            actor.to_hyphenated(),
            "organization_unit.create",
            "organization_unit",
            id.to_hyphenated(),
            ActionRisk::High,
            serde_json::json!({"organization_unit_id": id.to_hyphenated(), "tenant_id": tenant_id.to_hyphenated()}),
            &self.clock,
            ctx,
        ).await?;

        Ok(WriteResponse::new(
            OrganizationUnitDto::from(&unit),
            unit.revision,
        ))
    }

    async fn update(
        &self,
        request: WriteRequest<UpdateOrganizationUnitRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<OrganizationUnitDto>, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let tenant_id = usecase::require_tenant(ctx)?;
        let expected = request.expected_revision.ok_or_else(|| {
            PlatformError::invalid("expected_revision", "revision is required for updates")
        })?;

        let auth_req = usecase::tenant_authorization(
            actor,
            tenant_id,
            "tenant:organization:write",
            ResourceRef::Organization(request.payload.id),
        );
        usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        let mut unit = self.repo.by_id(request.payload.id, ctx).await?;
        let unit_id = unit.id;

        unit.rename(request.payload.name, &self.clock, Some(actor))?;

        self.repo.update(&unit, expected, ctx).await?;

        usecase::audit_write(
            &self.audit,
            tenant_id,
            "user",
            actor.to_hyphenated(),
            "organization_unit.update",
            "organization_unit",
            unit_id.to_hyphenated(),
            ActionRisk::Normal,
            serde_json::json!({"organization_unit_id": unit_id.to_hyphenated()}),
            &self.clock,
            ctx,
        )
        .await?;

        Ok(WriteResponse::new(
            OrganizationUnitDto::from(&unit),
            unit.revision,
        ))
    }

    async fn get(
        &self,
        id: OrganizationId,
        ctx: &RequestContext,
    ) -> Result<OrganizationUnitDto, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let tenant_id = usecase::require_tenant(ctx)?;

        let auth_req = usecase::tenant_authorization(
            actor,
            tenant_id,
            "tenant:organization:read",
            ResourceRef::Organization(id),
        );
        usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        let unit = self.repo.by_id(id, ctx).await?;
        Ok(OrganizationUnitDto::from(&unit))
    }

    async fn list(
        &self,
        ctx: &RequestContext,
        options: ListOptions,
    ) -> Result<Page<OrganizationUnitDto>, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let tenant_id = usecase::require_tenant(ctx)?;

        let auth_req = usecase::tenant_authorization(
            actor,
            tenant_id,
            "tenant:organization:read",
            ResourceRef::User(actor),
        );
        usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        let page = self.repo.list(ctx, options).await?;
        Ok(Page {
            items: page.items.iter().map(OrganizationUnitDto::from).collect(),
            next_cursor: page.next_cursor,
        })
    }
}
