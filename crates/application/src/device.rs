//! Device application use cases.

use async_trait::async_trait;
use domain_audit::audit_record::ActionRisk;
use domain_authorization::role_binding::ResourceRef;
use domain_resource::device::{DeviceLifecycle, ManagedDevice};
use foundation::{
    AreaId, Clock, DeviceId, IdGenerator, OrganizationId, PlatformError, RequestContext, Revision,
    TenantId,
};
use storage_api::{AuditWriter, DeviceRepository, ListOptions, Page};

use crate::authorization::AuthorizationPort;
use crate::usecase::{self, WriteRequest, WriteResponse};

/// Stable DTO for a managed device.
#[derive(Debug, Clone)]
pub struct DeviceDto {
    pub id: DeviceId,
    pub tenant_id: TenantId,
    pub organization_id: Option<OrganizationId>,
    pub area_id: Option<AreaId>,
    pub code: String,
    pub name: String,
    pub lifecycle: DeviceLifecycle,
    pub revision: Revision,
}

impl From<&ManagedDevice> for DeviceDto {
    fn from(d: &ManagedDevice) -> Self {
        Self {
            id: d.id,
            tenant_id: d.tenant_id,
            organization_id: d.organization_id,
            area_id: d.area_id,
            code: d.code.clone(),
            name: d.name.clone(),
            lifecycle: d.lifecycle,
            revision: d.revision,
        }
    }
}

/// Request to create a managed device.
#[derive(Debug, Clone)]
pub struct CreateDeviceRequest {
    pub code: String,
    pub name: String,
    pub serial: Option<String>,
    pub organization_id: Option<OrganizationId>,
    pub area_id: Option<AreaId>,
}

/// Request to update a managed device.
#[derive(Debug, Clone)]
pub struct UpdateDeviceRequest {
    pub id: DeviceId,
    pub name: String,
    pub organization_id: Option<OrganizationId>,
    pub area_id: Option<AreaId>,
}

/// Request to change a device lifecycle state.
#[derive(Debug, Clone)]
pub struct ChangeDeviceLifecycleRequest {
    pub id: DeviceId,
    pub lifecycle: DeviceLifecycle,
}

/// Port for device application use cases.
#[async_trait]
pub trait DeviceUseCase: Send + Sync {
    async fn create(
        &self,
        request: WriteRequest<CreateDeviceRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<DeviceDto>, PlatformError>;

    async fn update(
        &self,
        request: WriteRequest<UpdateDeviceRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<DeviceDto>, PlatformError>;

    async fn change_lifecycle(
        &self,
        request: WriteRequest<ChangeDeviceLifecycleRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<DeviceDto>, PlatformError>;

    async fn get(&self, id: DeviceId, ctx: &RequestContext) -> Result<DeviceDto, PlatformError>;

    async fn list(
        &self,
        ctx: &RequestContext,
        options: ListOptions,
    ) -> Result<Page<DeviceDto>, PlatformError>;
}

/// Default device application service.
#[derive(Debug, Clone)]
pub struct DeviceService<R, A, U, C, I> {
    repo: R,
    auth: A,
    audit: U,
    clock: C,
    id_gen: I,
}

impl<R, A, U, C, I> DeviceService<R, A, U, C, I> {
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
impl<R, A, U, C, I> DeviceUseCase for DeviceService<R, A, U, C, I>
where
    R: DeviceRepository,
    A: AuthorizationPort,
    U: AuditWriter,
    C: Clock,
    I: IdGenerator,
{
    async fn create(
        &self,
        request: WriteRequest<CreateDeviceRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<DeviceDto>, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let tenant_id = usecase::require_tenant(ctx)?;

        let auth_req = usecase::tenant_authorization(
            actor,
            tenant_id,
            "tenant:device:write",
            ResourceRef::User(actor),
        );
        usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        let id = DeviceId::generate(&self.id_gen)?;
        let mut device = ManagedDevice::new(
            id,
            tenant_id,
            request.payload.code,
            request.payload.name,
            request.payload.serial,
            &self.clock,
            Some(actor),
        )?;

        if request.payload.organization_id.is_some() || request.payload.area_id.is_some() {
            device.set_location(
                request.payload.organization_id,
                request.payload.area_id,
                &self.clock,
                Some(actor),
            );
        }

        self.repo.create(&device, ctx).await?;

        usecase::audit_write(
            &self.audit,
            tenant_id,
            "user",
            actor.to_hyphenated(),
            "device.create",
            "device",
            id.to_hyphenated(),
            ActionRisk::High,
            serde_json::json!({"device_id": id.to_hyphenated(), "tenant_id": tenant_id.to_hyphenated()}),
            &self.clock,
            ctx,
        ).await?;

        Ok(WriteResponse::new(
            DeviceDto::from(&device),
            device.revision,
        ))
    }

    async fn update(
        &self,
        request: WriteRequest<UpdateDeviceRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<DeviceDto>, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let tenant_id = usecase::require_tenant(ctx)?;
        let expected = request.expected_revision.ok_or_else(|| {
            PlatformError::invalid("expected_revision", "revision is required for updates")
        })?;

        let mut device = self.repo.by_id(request.payload.id, ctx).await?;
        let device_id = device.id;

        let auth_req = usecase::tenant_authorization(
            actor,
            tenant_id,
            "tenant:device:write",
            ResourceRef::Device(device_id),
        );
        usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        device.rename(request.payload.name, &self.clock, Some(actor));
        if request.payload.organization_id != device.organization_id
            || request.payload.area_id != device.area_id
        {
            device.set_location(
                request.payload.organization_id,
                request.payload.area_id,
                &self.clock,
                Some(actor),
            );
        }

        self.repo.update(&device, expected, ctx).await?;

        usecase::audit_write(
            &self.audit,
            tenant_id,
            "user",
            actor.to_hyphenated(),
            "device.update",
            "device",
            device_id.to_hyphenated(),
            ActionRisk::Normal,
            serde_json::json!({"device_id": device_id.to_hyphenated()}),
            &self.clock,
            ctx,
        )
        .await?;

        Ok(WriteResponse::new(
            DeviceDto::from(&device),
            device.revision,
        ))
    }

    async fn change_lifecycle(
        &self,
        request: WriteRequest<ChangeDeviceLifecycleRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<DeviceDto>, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let tenant_id = usecase::require_tenant(ctx)?;
        let expected = request.expected_revision.ok_or_else(|| {
            PlatformError::invalid(
                "expected_revision",
                "revision is required for lifecycle changes",
            )
        })?;

        let mut device = self.repo.by_id(request.payload.id, ctx).await?;
        let device_id = device.id;

        let auth_req = usecase::tenant_authorization(
            actor,
            tenant_id,
            "tenant:device:write",
            ResourceRef::Device(device_id),
        );
        usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        match request.payload.lifecycle {
            DeviceLifecycle::Active => device.activate(&self.clock, Some(actor))?,
            DeviceLifecycle::Disabled => device.disable(&self.clock, Some(actor))?,
            DeviceLifecycle::Retired => device.retire(&self.clock, Some(actor))?,
            DeviceLifecycle::Draft => {}
        }

        self.repo.update(&device, expected, ctx).await?;

        usecase::audit_write(
            &self.audit,
            tenant_id,
            "user",
            actor.to_hyphenated(),
            "device.change_lifecycle",
            "device",
            device_id.to_hyphenated(),
            ActionRisk::Critical,
            serde_json::json!({"device_id": device_id.to_hyphenated(), "lifecycle": request.payload.lifecycle.as_str()}),
            &self.clock,
            ctx,
        ).await?;

        Ok(WriteResponse::new(
            DeviceDto::from(&device),
            device.revision,
        ))
    }

    async fn get(&self, id: DeviceId, ctx: &RequestContext) -> Result<DeviceDto, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let tenant_id = usecase::require_tenant(ctx)?;

        let auth_req = usecase::tenant_authorization(
            actor,
            tenant_id,
            "tenant:device:read",
            ResourceRef::Device(id),
        );
        usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        let device = self.repo.by_id(id, ctx).await?;
        Ok(DeviceDto::from(&device))
    }

    async fn list(
        &self,
        ctx: &RequestContext,
        options: ListOptions,
    ) -> Result<Page<DeviceDto>, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let tenant_id = usecase::require_tenant(ctx)?;

        let auth_req = usecase::tenant_authorization(
            actor,
            tenant_id,
            "tenant:device:read",
            ResourceRef::User(actor),
        );
        usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        let page = self.repo.list(ctx, options).await?;
        Ok(Page {
            items: page.items.iter().map(DeviceDto::from).collect(),
            next_cursor: page.next_cursor,
        })
    }
}
