//! Configuration application use cases.

use async_trait::async_trait;
use domain_audit::audit_record::ActionRisk;
use domain_authorization::role_binding::ResourceRef;
use domain_configuration::{ConfigDefinition, ConfigScope, ConfigValue, ConfigValueId};
use foundation::{Clock, PlatformError, RequestContext, Revision, TenantId, uuid::Uuid};
use storage_api::{AuditWriter, ConfigurationRepository};

use crate::authorization::AuthorizationPort;
use crate::usecase::{self, WriteRequest, WriteResponse};

/// Stable DTO for a configuration value.
#[derive(Debug, Clone)]
pub struct ConfigValueDto {
    pub id: Option<ConfigValueId>,
    pub scope: ConfigScope,
    pub config_key: String,
    pub value: String,
    pub revision: Revision,
}

impl From<&ConfigValue> for ConfigValueDto {
    fn from(v: &ConfigValue) -> Self {
        Self {
            id: v.id,
            scope: v.scope.clone(),
            config_key: v.config_key.clone(),
            value: v.effective_value().to_string(),
            revision: v.revision,
        }
    }
}

/// Request to save a configuration definition.
#[derive(Debug, Clone)]
pub struct SaveDefinitionRequest {
    pub config_key: String,
    pub value_type: domain_configuration::ConfigValueType,
    pub schema: Option<String>,
    pub default_value: String,
    pub sensitive: bool,
    pub dynamic: bool,
}

/// Request to save a configuration value.
#[derive(Debug, Clone)]
pub struct SaveValueRequest {
    pub id: Option<ConfigValueId>,
    pub scope: ConfigScope,
    pub config_key: String,
    pub raw_value: String,
    pub secret_ref: Option<String>,
}

/// Request to resolve an effective configuration value.
#[derive(Debug, Clone)]
pub struct ResolveConfigRequest {
    pub config_key: String,
    pub tenant_id: Option<TenantId>,
    pub module: Option<String>,
}

/// Port for configuration application use cases.
#[async_trait]
pub trait ConfigUseCase: Send + Sync {
    /// Save or update a configuration definition.
    async fn save_definition(
        &self,
        request: SaveDefinitionRequest,
        ctx: &RequestContext,
    ) -> Result<ConfigDefinition, PlatformError>;

    /// Save or update a configuration value.
    async fn save_value(
        &self,
        request: WriteRequest<SaveValueRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<ConfigValueDto>, PlatformError>;

    /// Resolve the effective value of a configuration key.
    async fn resolve(
        &self,
        request: ResolveConfigRequest,
        ctx: &RequestContext,
    ) -> Result<Option<String>, PlatformError>;

    /// Get a configuration definition.
    async fn get_definition(
        &self,
        config_key: String,
        ctx: &RequestContext,
    ) -> Result<Option<ConfigDefinition>, PlatformError>;
}

/// Default configuration application service.
#[derive(Debug, Clone)]
pub struct ConfigService<R, A, U, C> {
    repo: R,
    auth: A,
    audit: U,
    clock: C,
}

impl<R, A, U, C> ConfigService<R, A, U, C> {
    pub fn new(repo: R, auth: A, audit: U, clock: C) -> Self {
        Self {
            repo,
            auth,
            audit,
            clock,
        }
    }
}

#[async_trait]
impl<R, A, U, C> ConfigUseCase for ConfigService<R, A, U, C>
where
    R: ConfigurationRepository,
    A: AuthorizationPort,
    U: AuditWriter,
    C: Clock,
{
    async fn save_definition(
        &self,
        request: SaveDefinitionRequest,
        ctx: &RequestContext,
    ) -> Result<ConfigDefinition, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let auth_req = usecase::platform_authorization(actor, "platform:tenant:write");
        usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        let definition = ConfigDefinition::new(
            request.config_key,
            request.value_type,
            request.schema,
            request.default_value,
            request.sensitive,
            request.dynamic,
        )?;

        self.repo.save_definition(&definition).await?;

        usecase::audit_write(
            &self.audit,
            TenantId::from_uuid(Uuid::nil()),
            "user",
            actor.to_hyphenated(),
            "config.definition.save",
            "config_definition",
            definition.config_key.clone(),
            ActionRisk::High,
            serde_json::json!({"config_key": definition.config_key}),
            &self.clock,
            ctx,
        )
        .await?;

        Ok(definition)
    }

    async fn save_value(
        &self,
        request: WriteRequest<SaveValueRequest>,
        ctx: &RequestContext,
    ) -> Result<WriteResponse<ConfigValueDto>, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let tenant_id = request
            .payload
            .scope
            .tenant_id()
            .or(ctx.tenant_id)
            .ok_or_else(|| {
                PlatformError::invalid("tenant_id", "tenant scope is required for config values")
            })?;

        let auth_req = usecase::tenant_authorization(
            actor,
            tenant_id,
            "tenant:config:write",
            ResourceRef::User(actor),
        );
        usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        let definition = self
            .repo
            .get_definition(&request.payload.config_key)
            .await?
            .ok_or_else(|| {
                PlatformError::invalid(
                    "config_key",
                    format!("definition not found: {}", request.payload.config_key),
                )
            })?;

        let (id, revision) = if let Some(id) = request.payload.id {
            let expected = request.expected_revision.ok_or_else(|| {
                PlatformError::invalid(
                    "expected_revision",
                    "revision is required for value updates",
                )
            })?;
            (Some(id), expected)
        } else {
            (None, Revision::initial())
        };

        let value = ConfigValue::new(
            id,
            request.payload.scope,
            &definition,
            request.payload.raw_value,
            request.payload.secret_ref,
            revision,
        )?;

        let saved = self.repo.save_value(&value, ctx).await?;

        usecase::audit_write(
            &self.audit,
            tenant_id,
            "user",
            actor.to_hyphenated(),
            "config.value.save",
            "config_value",
            saved.config_key.clone(),
            if definition.sensitive {
                ActionRisk::Critical
            } else {
                ActionRisk::Normal
            },
            serde_json::json!({"config_key": saved.config_key, "scope": saved.scope.scope_type()}),
            &self.clock,
            ctx,
        )
        .await?;

        Ok(WriteResponse::new(
            ConfigValueDto::from(&saved),
            saved.revision,
        ))
    }

    async fn resolve(
        &self,
        request: ResolveConfigRequest,
        ctx: &RequestContext,
    ) -> Result<Option<String>, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let tenant_id = request.tenant_id.or(ctx.tenant_id).ok_or_else(|| {
            PlatformError::invalid(
                "tenant_id",
                "tenant scope is required for config resolution",
            )
        })?;

        let auth_req = usecase::tenant_authorization(
            actor,
            tenant_id,
            "tenant:config:read",
            ResourceRef::User(actor),
        );
        usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        self.repo
            .resolve(
                &request.config_key,
                Some(tenant_id),
                request.module.as_deref(),
                ctx,
            )
            .await
    }

    async fn get_definition(
        &self,
        config_key: String,
        ctx: &RequestContext,
    ) -> Result<Option<ConfigDefinition>, PlatformError> {
        usecase::check_deadline(ctx, &self.clock)?;
        let actor = usecase::require_actor(ctx)?;
        let auth_req = usecase::platform_authorization(actor, "platform:tenant:read");
        usecase::authorize_or_fail(&self.auth, auth_req, ctx).await?;

        self.repo.get_definition(&config_key).await
    }
}
