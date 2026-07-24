//! User application use cases.

use async_trait::async_trait;
use domain_audit::audit_record::ActionRisk;
use domain_authorization::role_binding::ResourceRef;
use domain_identity::user::{User, UserStatus};
use foundation::{Clock, IdGenerator, PlatformError, RequestContext, Revision, TenantId, UserId};
use storage_api::{AuditWriter, ListOptions, Page, UserRepository};

use crate::authorization::AuthorizationPort;
use crate::usecase::{self, WriteRequest, WriteResponse};

/// Stable DTO for a user.
#[derive(Debug, Clone)]
pub struct UserDto {
    pub id: UserId,
    pub tenant_id: TenantId,
    pub username: String,
    pub display_name: String,
    pub status: UserStatus,
    pub revision: Revision,
}

impl From<&User> for UserDto {
    fn from(u: &User) -> Self {
        Self {
            id: u.id,
            tenant_id: u.tenant_id,
            username: u.username.clone(),
            display_name: u.display_name.clone(),
            status: u.status,
            revision: u.revision,
        }
    }
}

/// Request to create a user.
#[derive(Debug, Clone)]
pub struct CreateUserRequest {
    pub username: String,
    pub display_name: String,
}

/// Request to update a user.
#[derive(Debug, Clone)]
pub struct UpdateUserRequest {
    pub id: UserId,
    pub display_name: String,
}

/// Request to change a user's lifecycle status.
#[derive(Debug, Clone)]
pub struct ChangeUserStatusRequest {
    pub id: UserId,
    pub status: UserStatus,
}

/// Port for user application use cases.
#[async_trait]
pub trait UserUseCase: Send + Sync {
    async fn create(
        &self,
        request: WriteRequest<CreateUserRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<UserDto>, PlatformError>;

    async fn update(
        &self,
        request: WriteRequest<UpdateUserRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<UserDto>, PlatformError>;

    async fn change_status(
        &self,
        request: WriteRequest<ChangeUserStatusRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<UserDto>, PlatformError>;

    async fn get(&self, id: UserId, ctx: &RequestContext) -> Result<UserDto, PlatformError>;

    async fn list(
        &self,
        ctx: &RequestContext,
        options: ListOptions,
    ) -> Result<Page<UserDto>, PlatformError>;
}

/// Default user application service.
#[derive(Debug, Clone)]
pub struct UserService<R, A, U, C, I> {
    repo: R,
    auth: A,
    audit: U,
    clock: C,
    id_gen: I,
}

impl<R, A, U, C, I> UserService<R, A, U, C, I> {
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
impl<R, A, U, C, I> UserUseCase for UserService<R, A, U, C, I>
where
    R: UserRepository,
    A: AuthorizationPort,
    U: AuditWriter,
    C: Clock,
    I: IdGenerator,
{
    async fn create(
        &self,
        request: WriteRequest<CreateUserRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<UserDto>, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let tenant_id = usecase::require_tenant(ctx)?;

        let auth_req = usecase::tenant_authorization(
            actor,
            tenant_id,
            "tenant:user:write",
            ResourceRef::User(actor),
        );
        usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        let id = UserId::generate(&self.id_gen)?;
        let user = User::new(
            id,
            tenant_id,
            request.payload.username,
            request.payload.display_name,
            &self.clock,
            Some(actor),
        )?;

        self.repo.create(&user, ctx).await?;

        usecase::audit_write(
            &self.audit,
            tenant_id,
            "user",
            actor.to_hyphenated(),
            "user.create",
            "user",
            id.to_hyphenated(),
            ActionRisk::High,
            serde_json::json!({"user_id": id.to_hyphenated(), "tenant_id": tenant_id.to_hyphenated()}),
            &self.clock,
            ctx,
        ).await?;

        Ok(WriteResponse::new(UserDto::from(&user), user.revision))
    }

    async fn update(
        &self,
        request: WriteRequest<UpdateUserRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<UserDto>, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let tenant_id = usecase::require_tenant(ctx)?;
        let expected = request.expected_revision.ok_or_else(|| {
            PlatformError::invalid("expected_revision", "revision is required for updates")
        })?;

        let auth_req = usecase::tenant_authorization(
            actor,
            tenant_id,
            "tenant:user:write",
            ResourceRef::User(request.payload.id),
        );
        usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        let mut user = self.repo.by_id(request.payload.id, ctx).await?;
        let user_id = user.id;

        user.set_display_name(request.payload.display_name, &self.clock, Some(actor))?;

        self.repo.update(&user, expected, ctx).await?;

        usecase::audit_write(
            &self.audit,
            tenant_id,
            "user",
            actor.to_hyphenated(),
            "user.update",
            "user",
            user_id.to_hyphenated(),
            ActionRisk::Normal,
            serde_json::json!({"user_id": user_id.to_hyphenated()}),
            &self.clock,
            ctx,
        )
        .await?;

        Ok(WriteResponse::new(UserDto::from(&user), user.revision))
    }

    async fn change_status(
        &self,
        request: WriteRequest<ChangeUserStatusRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<UserDto>, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let tenant_id = usecase::require_tenant(ctx)?;
        let expected = request.expected_revision.ok_or_else(|| {
            PlatformError::invalid(
                "expected_revision",
                "revision is required for status changes",
            )
        })?;

        let auth_req = usecase::tenant_authorization(
            actor,
            tenant_id,
            "tenant:user:write",
            ResourceRef::User(request.payload.id),
        );
        usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        let mut user = self.repo.by_id(request.payload.id, ctx).await?;
        let user_id = user.id;
        let previous_status = user.status;

        match request.payload.status {
            UserStatus::Active => user.activate(&self.clock, Some(actor))?,
            UserStatus::Locked => user.lock(&self.clock, Some(actor))?,
            UserStatus::Disabled => user.disable(&self.clock, Some(actor))?,
            UserStatus::Pending => {
                if previous_status == UserStatus::Disabled {
                    user.enable(&self.clock, Some(actor))?;
                } else if previous_status != UserStatus::Pending {
                    return Err(PlatformError::invalid(
                        "status",
                        "user can only be set to pending when re-enabling a disabled user",
                    ));
                }
            }
        }

        if user.status == previous_status {
            return Err(PlatformError::invalid(
                "status",
                format!("user status is already {}", request.payload.status.as_str()),
            ));
        }

        if request.payload.status == UserStatus::Locked
            || request.payload.status == UserStatus::Disabled
        {
            user.bump_session_version(&self.clock, Some(actor))?;
        }

        self.repo.update(&user, expected, ctx).await?;

        usecase::audit_write(
            &self.audit,
            tenant_id,
            "user",
            actor.to_hyphenated(),
            "user.change_status",
            "user",
            user_id.to_hyphenated(),
            ActionRisk::Critical,
            serde_json::json!({"user_id": user_id.to_hyphenated(), "status": request.payload.status.as_str()}),
            &self.clock,
            ctx,
        ).await?;

        Ok(WriteResponse::new(UserDto::from(&user), user.revision))
    }

    async fn get(&self, id: UserId, ctx: &RequestContext) -> Result<UserDto, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let tenant_id = usecase::require_tenant(ctx)?;

        let auth_req = usecase::tenant_authorization(
            actor,
            tenant_id,
            "tenant:user:read",
            ResourceRef::User(id),
        );
        usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        let user = self.repo.by_id(id, ctx).await?;
        Ok(UserDto::from(&user))
    }

    async fn list(
        &self,
        ctx: &RequestContext,
        options: ListOptions,
    ) -> Result<Page<UserDto>, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let tenant_id = usecase::require_tenant(ctx)?;

        let auth_req = usecase::tenant_authorization(
            actor,
            tenant_id,
            "tenant:user:read",
            ResourceRef::User(actor),
        );
        usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        let page = self.repo.list(ctx, options).await?;
        Ok(Page {
            items: page.items.iter().map(UserDto::from).collect(),
            next_cursor: page.next_cursor,
        })
    }
}
