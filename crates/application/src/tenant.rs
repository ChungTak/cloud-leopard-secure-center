//! Tenant application use cases.

use async_trait::async_trait;
use domain_audit::audit_record::ActionRisk;
use domain_organization::tenant::{Tenant, TenantStatus};
use foundation::{Clock, IdGenerator, PlatformError, RequestContext, Revision, TenantId};
use storage_api::{AuditWriter, Page, TenantRepository};

use crate::authorization::AuthorizationPort;
use crate::usecase::{self, WriteRequest, WriteResponse};

/// Stable DTO for a tenant.
#[derive(Debug, Clone)]
pub struct TenantDto {
    pub id: TenantId,
    pub code: String,
    pub name: String,
    pub locale: String,
    pub timezone: String,
    pub status: TenantStatus,
    pub revision: Revision,
}

impl From<&Tenant> for TenantDto {
    fn from(t: &Tenant) -> Self {
        Self {
            id: t.id,
            code: t.code.clone(),
            name: t.name.clone(),
            locale: t.locale.clone(),
            timezone: t.timezone.clone(),
            status: t.status,
            revision: t.revision,
        }
    }
}

/// Request to create a tenant.
#[derive(Debug, Clone)]
pub struct CreateTenantRequest {
    pub code: String,
    pub name: String,
    pub locale: Option<String>,
    pub timezone: Option<String>,
}

/// Request to update a tenant.
#[derive(Debug, Clone)]
pub struct UpdateTenantRequest {
    pub id: TenantId,
    pub name: String,
    pub locale: Option<String>,
    pub timezone: Option<String>,
}

/// Request to change a tenant's lifecycle status.
#[derive(Debug, Clone)]
pub struct ChangeTenantStatusRequest {
    pub id: TenantId,
    pub status: TenantStatus,
}

/// Port for tenant application use cases.
#[async_trait]
pub trait TenantUseCase: Send + Sync {
    /// Create a new tenant.
    async fn create(
        &self,
        request: WriteRequest<CreateTenantRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<TenantDto>, PlatformError>;

    /// Update an existing tenant.
    async fn update(
        &self,
        request: WriteRequest<UpdateTenantRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<TenantDto>, PlatformError>;

    /// Change a tenant's lifecycle status (suspend/close).
    async fn change_status(
        &self,
        request: WriteRequest<ChangeTenantStatusRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<TenantDto>, PlatformError>;

    /// Get a tenant by id.
    async fn get(&self, id: TenantId, ctx: &RequestContext) -> Result<TenantDto, PlatformError>;

    /// List tenants visible to the caller.
    async fn list(&self, ctx: &RequestContext) -> Result<Page<TenantDto>, PlatformError>;
}

/// Default tenant application service.
#[derive(Debug, Clone)]
pub struct TenantService<R, A, U, C, I> {
    repo: R,
    auth: A,
    audit: U,
    clock: C,
    id_gen: I,
}

impl<R, A, U, C, I> TenantService<R, A, U, C, I> {
    /// Create a new tenant service from its dependencies.
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
impl<R, A, U, C, I> TenantUseCase for TenantService<R, A, U, C, I>
where
    R: TenantRepository,
    A: AuthorizationPort,
    U: AuditWriter,
    C: Clock,
    I: IdGenerator,
{
    async fn create(
        &self,
        request: WriteRequest<CreateTenantRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<TenantDto>, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let req = usecase::platform_authorization(actor, "platform:tenant:write");
        usecase::authorize_or_fail(&self.auth, req, ctx).await?;

        let id = TenantId::generate(&self.id_gen)?;
        let tenant = Tenant::new(
            id,
            request.payload.code,
            request.payload.name,
            request.payload.locale,
            request.payload.timezone,
            &self.clock,
            Some(actor),
        )?;

        self.repo.create(&tenant, ctx).await?;

        usecase::audit_write(
            &self.audit,
            id,
            "user",
            actor.to_hyphenated(),
            "tenant.create",
            "tenant",
            id.to_hyphenated(),
            ActionRisk::High,
            serde_json::json!({"tenant_id": id.to_hyphenated()}),
            &self.clock,
            ctx,
        )
        .await?;

        Ok(WriteResponse::new(
            TenantDto::from(&tenant),
            tenant.revision,
        ))
    }

    async fn update(
        &self,
        request: WriteRequest<UpdateTenantRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<TenantDto>, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let expected = request.expected_revision.ok_or_else(|| {
            PlatformError::invalid("expected_revision", "revision is required for updates")
        })?;

        let mut tenant = self.repo.by_id(request.payload.id, ctx).await?;
        let tenant_id = tenant.id;

        let auth_req = usecase::platform_authorization(actor, "platform:tenant:write");
        usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        if let Some(locale) = request.payload.locale {
            tenant.set_locale(locale, &self.clock, Some(actor))?;
        }
        if let Some(timezone) = request.payload.timezone {
            tenant.set_timezone(timezone, &self.clock, Some(actor))?;
        }
        tenant.rename(request.payload.name, &self.clock, Some(actor))?;

        self.repo.update(&tenant, expected, ctx).await?;

        usecase::audit_write(
            &self.audit,
            tenant_id,
            "user",
            actor.to_hyphenated(),
            "tenant.update",
            "tenant",
            tenant_id.to_hyphenated(),
            ActionRisk::Normal,
            serde_json::json!({"tenant_id": tenant_id.to_hyphenated()}),
            &self.clock,
            ctx,
        )
        .await?;

        Ok(WriteResponse::new(
            TenantDto::from(&tenant),
            tenant.revision,
        ))
    }

    async fn change_status(
        &self,
        request: WriteRequest<ChangeTenantStatusRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<TenantDto>, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let expected = request.expected_revision.ok_or_else(|| {
            PlatformError::invalid(
                "expected_revision",
                "revision is required for status changes",
            )
        })?;

        let mut tenant = self.repo.by_id(request.payload.id, ctx).await?;
        let tenant_id = tenant.id;

        let auth_req = usecase::platform_authorization(actor, "platform:tenant:write");
        usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        match request.payload.status {
            TenantStatus::Suspended => tenant.suspend(&self.clock, Some(actor))?,
            TenantStatus::Closed => {
                tenant.close(&self.clock, Some(actor));
            }
            TenantStatus::Active => {
                return Err(PlatformError::invalid(
                    "tenant_status",
                    "cannot reactivate a tenant through this use case",
                ));
            }
        }

        self.repo.update(&tenant, expected, ctx).await?;

        usecase::audit_write(
            &self.audit,
            tenant_id,
            "user",
            actor.to_hyphenated(),
            "tenant.change_status",
            "tenant",
            tenant_id.to_hyphenated(),
            ActionRisk::Critical,
            serde_json::json!({"tenant_id": tenant_id.to_hyphenated(), "status": request.payload.status.as_str()}),
            &self.clock,
            ctx,
        )
        .await?;

        Ok(WriteResponse::new(
            TenantDto::from(&tenant),
            tenant.revision,
        ))
    }

    async fn get(&self, id: TenantId, ctx: &RequestContext) -> Result<TenantDto, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let auth_req = usecase::platform_authorization(actor, "platform:tenant:read");
        usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        let tenant = self.repo.by_id(id, ctx).await?;
        Ok(TenantDto::from(&tenant))
    }

    async fn list(&self, ctx: &RequestContext) -> Result<Page<TenantDto>, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let auth_req = usecase::platform_authorization(actor, "platform:tenant:read");
        usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        let page = self.repo.list(ctx).await?;
        Ok(Page {
            items: page.items.iter().map(TenantDto::from).collect(),
            next_cursor: page.next_cursor,
        })
    }
}
