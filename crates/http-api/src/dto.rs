//! HTTP request/response DTOs and explicit domain mappers.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Generic API problem details sent by this service.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProblemDetailsDto {
    #[schema(rename = "type")]
    #[serde(rename = "type")]
    pub problem_type: String,
    pub title: String,
    pub status: u16,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
}

/// Health status returned by the root endpoint.
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HealthDto {
    pub status: String,
    pub component: String,
}

/// Stable API representation of a tenant.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TenantDto {
    pub id: String,
    pub code: String,
    pub name: String,
    pub locale: String,
    pub timezone: String,
    pub status: String,
    pub revision: u64,
    pub created_at: String,
    pub updated_at: String,
    pub actor: Option<String>,
}

impl From<&domain_organization::tenant::Tenant> for TenantDto {
    fn from(t: &domain_organization::tenant::Tenant) -> Self {
        Self {
            id: t.id.as_uuid().to_string(),
            code: t.code.clone(),
            name: t.name.clone(),
            locale: t.locale.clone(),
            timezone: t.timezone.clone(),
            status: t.status.as_str().to_string(),
            revision: t.revision.value(),
            created_at: t.created_at.to_rfc3339(),
            updated_at: t.updated_at.to_rfc3339(),
            actor: t.actor.map(|a| a.as_uuid().to_string()),
        }
    }
}

/// Stable API representation of an organization unit.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationUnitDto {
    pub id: String,
    pub tenant_id: String,
    pub parent_id: Option<String>,
    pub code: String,
    pub name: String,
    pub revision: u64,
    pub created_at: String,
    pub updated_at: String,
    pub actor: Option<String>,
}

impl From<&domain_organization::organization_unit::OrganizationUnit> for OrganizationUnitDto {
    fn from(u: &domain_organization::organization_unit::OrganizationUnit) -> Self {
        Self {
            id: u.id.as_uuid().to_string(),
            tenant_id: u.tenant_id.as_uuid().to_string(),
            parent_id: u.parent_id.map(|id| id.as_uuid().to_string()),
            code: u.code.clone(),
            name: u.name.clone(),
            revision: u.revision.value(),
            created_at: u.created_at.to_rfc3339(),
            updated_at: u.updated_at.to_rfc3339(),
            actor: u.actor.map(|a| a.as_uuid().to_string()),
        }
    }
}

/// Request to create an organization unit.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateOrganizationUnitRequest {
    pub parent_id: Option<String>,
    pub code: String,
    pub name: String,
}

/// Request to update an organization unit.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOrganizationUnitRequest {
    pub name: String,
    pub expected_revision: u64,
}

/// Request to move an organization unit.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MoveOrganizationUnitRequest {
    pub parent_id: Option<String>,
    pub expected_revision: u64,
}

/// Stable API representation of a spatial node.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SpatialNodeDto {
    pub id: String,
    pub tenant_id: String,
    pub parent_id: Option<String>,
    #[schema(rename = "nodeType")]
    pub node_type: SpatialNodeType,
    pub code: String,
    pub name: String,
    pub revision: u64,
    pub created_at: String,
    pub updated_at: String,
    pub actor: Option<String>,
}

/// Spatial node kind.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SpatialNodeType {
    Site,
    Building,
    Floor,
    Area,
}

/// Request to create a spatial node.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateSpatialNodeRequest {
    pub parent_id: Option<String>,
    pub node_type: SpatialNodeType,
    pub code: String,
    pub name: String,
}

/// Request to update a spatial node.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSpatialNodeRequest {
    pub name: String,
    pub expected_revision: u64,
}

/// Request to move a spatial node.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MoveSpatialNodeRequest {
    pub parent_id: Option<String>,
    pub expected_revision: u64,
}

/// Request to create a user.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateUserRequest {
    pub username: String,
    pub display_name: String,
}

/// Request to update a user.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUserRequest {
    pub display_name: String,
    pub expected_revision: u64,
}

/// Request to change a user status.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChangeUserStatusRequest {
    pub status: String,
    pub expected_revision: u64,
}

/// Request to set a user's password.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetPasswordRequest {
    pub password: String,
    pub expected_revision: u64,
}

/// Request to enable or disable MFA.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManageMfaRequest {
    pub enabled: bool,
    pub expected_revision: u64,
}

/// Request to create a service account.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateServiceAccountRequest {
    pub name: String,
}

/// Response containing a newly created API key (shown once).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyCreatedDto {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    pub key: String,
    pub scopes: Vec<String>,
    pub expires_at: Option<String>,
}

/// Request to create a role.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateRoleRequest {
    pub name: String,
    pub permissions: Vec<String>,
}

/// Request to update a role.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRoleRequest {
    pub name: String,
    pub permissions: Vec<String>,
    pub expected_revision: u64,
}

/// Request to create a role binding.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateRoleBindingRequest {
    pub principal_id: String,
    pub role_id: String,
    pub scope: RoleBindingScopeDto,
    pub valid_from: String,
    pub valid_until: Option<String>,
}

/// Request to update a role binding.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRoleBindingRequest {
    pub role_id: String,
    pub scope: RoleBindingScopeDto,
    pub valid_from: String,
    pub valid_until: Option<String>,
    pub expected_revision: u64,
}

/// Request to preview an authorization decision.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuthExplainRequest {
    pub principal_id: String,
    pub action: String,
    pub resource_type: String,
    pub resource_id: String,
}

/// Authorization explanation.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuthExplainResponse {
    pub decision: String,
    pub reason: String,
}

/// Stable API representation of a user.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserDto {
    pub id: String,
    pub tenant_id: String,
    pub username: String,
    pub display_name: String,
    pub status: String,
    pub session_version: u64,
    pub revision: u64,
    pub created_at: String,
    pub updated_at: String,
    pub actor: Option<String>,
    pub deleted_at: Option<String>,
}

impl From<&domain_identity::user::User> for UserDto {
    fn from(u: &domain_identity::user::User) -> Self {
        Self {
            id: u.id.as_uuid().to_string(),
            tenant_id: u.tenant_id.as_uuid().to_string(),
            username: u.username.clone(),
            display_name: u.display_name.clone(),
            status: u.status.as_str().to_string(),
            session_version: u.session_version,
            revision: u.revision.value(),
            created_at: u.created_at.to_rfc3339(),
            updated_at: u.updated_at.to_rfc3339(),
            actor: u.actor.map(|a| a.as_uuid().to_string()),
            deleted_at: u.deleted_at.map(|ts| ts.to_rfc3339()),
        }
    }
}

/// Stable API representation of a role.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoleDto {
    pub id: String,
    pub tenant_id: Option<String>,
    pub name: String,
    pub is_builtin: bool,
    pub permissions: Vec<String>,
    pub revision: u64,
    pub created_at: String,
    pub updated_at: String,
    pub actor: Option<String>,
}

impl From<&domain_authorization::role::Role> for RoleDto {
    fn from(r: &domain_authorization::role::Role) -> Self {
        Self {
            id: r.id.as_uuid().to_string(),
            tenant_id: r.tenant_id.map(|id| id.as_uuid().to_string()),
            name: r.name.clone(),
            is_builtin: r.is_builtin,
            permissions: r.permissions.clone(),
            revision: r.revision.value(),
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
            actor: r.actor.map(|a| a.as_uuid().to_string()),
        }
    }
}

/// Stable API representation of an explicit resource reference.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResourceRefDto {
    pub resource_type: String,
    pub resource_id: String,
}

impl From<&domain_authorization::role_binding::ResourceRef> for ResourceRefDto {
    fn from(r: &domain_authorization::role_binding::ResourceRef) -> Self {
        Self {
            resource_type: r.resource_type().to_string(),
            resource_id: r.as_uuid().to_string(),
        }
    }
}

/// Stable API representation of a role-binding scope.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RoleBindingScopeDto {
    Tenant,
    OrganizationSubtree { resource_id: String },
    AreaSubtree { resource_id: String },
    ResourceSet { resources: Vec<ResourceRefDto> },
}

impl From<&domain_authorization::role_binding::Scope> for RoleBindingScopeDto {
    fn from(scope: &domain_authorization::role_binding::Scope) -> Self {
        use domain_authorization::role_binding::Scope as S;
        match scope {
            S::Tenant => Self::Tenant,
            S::OrganizationSubtree(id) => Self::OrganizationSubtree {
                resource_id: id.as_uuid().to_string(),
            },
            S::AreaSubtree(id) => Self::AreaSubtree {
                resource_id: id.as_uuid().to_string(),
            },
            S::ResourceSet(refs) => Self::ResourceSet {
                resources: refs.iter().map(Into::into).collect(),
            },
        }
    }
}

/// Stable API representation of a role binding.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoleBindingDto {
    pub id: String,
    pub tenant_id: String,
    pub principal_id: String,
    pub role_id: String,
    pub scope: RoleBindingScopeDto,
    pub valid_from: String,
    pub valid_until: Option<String>,
    pub revision: u64,
    pub created_at: String,
    pub updated_at: String,
    pub actor: Option<String>,
}

impl From<&domain_authorization::role_binding::RoleBinding> for RoleBindingDto {
    fn from(b: &domain_authorization::role_binding::RoleBinding) -> Self {
        Self {
            id: b.id.as_uuid().to_string(),
            tenant_id: b.tenant_id.as_uuid().to_string(),
            principal_id: b.principal_id.as_uuid().to_string(),
            role_id: b.role_id.as_uuid().to_string(),
            scope: (&b.scope).into(),
            valid_from: b.valid_from.to_rfc3339(),
            valid_until: b.valid_until.map(|ts| ts.to_rfc3339()),
            revision: b.revision.value(),
            created_at: b.created_at.to_rfc3339(),
            updated_at: b.updated_at.to_rfc3339(),
            actor: b.actor.map(|a| a.as_uuid().to_string()),
        }
    }
}

/// Stable API representation of a managed device.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeviceDto {
    pub id: String,
    pub tenant_id: String,
    pub organization_id: Option<String>,
    pub area_id: Option<String>,
    pub code: String,
    pub name: String,
    pub serial: Option<String>,
    pub lifecycle: String,
    pub online_state: String,
    pub revision: u64,
    pub created_at: String,
    pub updated_at: String,
    pub actor: Option<String>,
}

impl From<&domain_resource::device::ManagedDevice> for DeviceDto {
    fn from(d: &domain_resource::device::ManagedDevice) -> Self {
        Self {
            id: d.id.as_uuid().to_string(),
            tenant_id: d.tenant_id.as_uuid().to_string(),
            organization_id: d.organization_id.map(|id| id.as_uuid().to_string()),
            area_id: d.area_id.map(|id| id.as_uuid().to_string()),
            code: d.code.clone(),
            name: d.name.clone(),
            serial: d.serial.clone(),
            lifecycle: d.lifecycle.as_str().to_string(),
            online_state: d.online_state.as_str().to_string(),
            revision: d.revision.value(),
            created_at: d.created_at.to_rfc3339(),
            updated_at: d.updated_at.to_rfc3339(),
            actor: d.actor.map(|a| a.as_uuid().to_string()),
        }
    }
}

/// Stable API representation of a camera.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CameraDto {
    pub id: String,
    pub tenant_id: String,
    pub device_id: String,
    pub area_id: Option<String>,
    pub code: String,
    pub name: String,
    pub sensitivity: String,
    pub is_enabled: bool,
    pub revision: u64,
    pub created_at: String,
    pub updated_at: String,
    pub actor: Option<String>,
}

impl From<&domain_resource::camera::Camera> for CameraDto {
    fn from(c: &domain_resource::camera::Camera) -> Self {
        Self {
            id: c.id.as_uuid().to_string(),
            tenant_id: c.tenant_id.as_uuid().to_string(),
            device_id: c.device_id.as_uuid().to_string(),
            area_id: c.area_id.map(|id| id.as_uuid().to_string()),
            code: c.code.clone(),
            name: c.name.clone(),
            sensitivity: c.sensitivity.as_str().to_string(),
            is_enabled: c.is_enabled,
            revision: c.revision.value(),
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
            actor: c.actor.map(|a| a.as_uuid().to_string()),
        }
    }
}

/// Request to create a managed device.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateDeviceRequest {
    pub organization_id: Option<String>,
    pub area_id: Option<String>,
    pub code: String,
    pub name: String,
    pub serial: Option<String>,
}

/// Request to update a managed device.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDeviceRequest {
    pub organization_id: Option<String>,
    pub area_id: Option<String>,
    pub name: String,
    pub serial: Option<String>,
    pub expected_revision: u64,
}

/// Request to change a device lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChangeDeviceLifecycleRequest {
    pub lifecycle: String,
    pub expected_revision: u64,
}

/// Request to create a camera.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateCameraRequest {
    pub device_id: String,
    pub area_id: Option<String>,
    pub code: String,
    pub name: String,
    pub sensitivity: String,
}

/// Request to update a camera.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCameraRequest {
    pub area_id: Option<String>,
    pub name: String,
    pub sensitivity: String,
    pub is_enabled: bool,
    pub expected_revision: u64,
}

/// Stable API representation of a tag.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TagDto {
    pub id: String,
    pub tenant_id: String,
    pub resource_type: String,
    pub resource_id: String,
    pub key: String,
    pub value: String,
    pub revision: u64,
    pub created_at: String,
    pub updated_at: String,
    pub actor: Option<String>,
}

/// Request to create a tag.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateTagRequest {
    pub resource_type: String,
    pub resource_id: String,
    pub key: String,
    pub value: String,
}

/// Request to update a tag.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTagRequest {
    pub value: String,
    pub expected_revision: u64,
}

/// Stable API representation of an external binding.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExternalBindingDto {
    pub id: String,
    pub tenant_id: String,
    pub resource_type: String,
    pub resource_id: String,
    pub external_ref: String,
    pub external_kind: String,
    pub state: String,
    pub activated_at: Option<String>,
    pub revision: u64,
    pub created_at: String,
    pub updated_at: String,
    pub actor: Option<String>,
}

/// Request to create an external binding.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateExternalBindingRequest {
    pub resource_type: String,
    pub resource_id: String,
    pub external_ref: String,
    pub external_kind: String,
}

/// Request to resolve an external binding conflict.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResolveExternalBindingConflictRequest {
    pub action: String,
    pub expected_revision: u64,
}

/// Stable API representation of a projection node.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectionStateDto {
    pub channel_id: String,
    pub device_id: String,
    pub tenant_id: String,
    pub is_online: bool,
    pub observed_at: String,
    pub is_stale: bool,
}

/// Stable API representation of an audit record.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuditRecordDto {
    pub id: Option<String>,
    pub tenant_id: String,
    pub actor_type: String,
    pub actor_id: String,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub result: String,
    pub risk: String,
    pub request_id: Option<String>,
    pub trace_id: Option<String>,
    pub source_ip: Option<String>,
    pub before_digest: Option<String>,
    pub after_digest: Option<String>,
    pub occurred_at: String,
    pub details: AuditDetailsDto,
}

/// Stable API representation of audit record details.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuditDetailsDto {
    pub schema: String,
    pub value: serde_json::Value,
}

impl From<&domain_audit::audit_record::AuditRecord> for AuditRecordDto {
    fn from(r: &domain_audit::audit_record::AuditRecord) -> Self {
        Self {
            id: r.id.map(|i| i.value().to_string()),
            tenant_id: r.tenant_id.as_uuid().to_string(),
            actor_type: r.actor_type.clone(),
            actor_id: r.actor_id.clone(),
            action: r.action.clone(),
            target_type: r.target_type.clone(),
            target_id: r.target_id.clone(),
            result: r.result.as_str().to_string(),
            risk: r.risk.as_str().to_string(),
            request_id: r.request_id.clone(),
            trace_id: r.trace_id.clone(),
            source_ip: r.source_ip.clone(),
            before_digest: r.before_digest.clone(),
            after_digest: r.after_digest.clone(),
            occurred_at: r.occurred_at.to_rfc3339(),
            details: (&r.details).into(),
        }
    }
}

impl From<&domain_audit::audit_record::AuditDetails> for AuditDetailsDto {
    fn from(d: &domain_audit::audit_record::AuditDetails) -> Self {
        let value = match serde_json::from_str(&d.value) {
            Ok(v) => v,
            Err(_) => serde_json::Value::Null,
        };
        Self {
            schema: d.schema.clone(),
            value,
        }
    }
}

/// Stable API representation of a configuration scope.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConfigScopeDto {
    Platform,
    Tenant { tenant_id: String },
    Module { tenant_id: String, module: String },
}

impl From<&domain_configuration::ConfigScope> for ConfigScopeDto {
    fn from(scope: &domain_configuration::ConfigScope) -> Self {
        match scope {
            domain_configuration::ConfigScope::Platform => Self::Platform,
            domain_configuration::ConfigScope::Tenant(id) => Self::Tenant {
                tenant_id: id.as_uuid().to_string(),
            },
            domain_configuration::ConfigScope::Module { tenant_id, module } => Self::Module {
                tenant_id: tenant_id.as_uuid().to_string(),
                module: module.clone(),
            },
        }
    }
}

/// Stable API representation of a configuration value.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigValueDto {
    pub id: Option<String>,
    pub scope: ConfigScopeDto,
    pub config_key: String,
    pub value: String,
    pub secret_ref: Option<String>,
    pub revision: u64,
}

impl From<&domain_configuration::ConfigValue> for ConfigValueDto {
    fn from(v: &domain_configuration::ConfigValue) -> Self {
        Self {
            id: v.id.map(|i| i.0.to_string()),
            scope: (&v.scope).into(),
            config_key: v.config_key.clone(),
            value: v.effective_value().to_string(),
            secret_ref: v.secret_ref.clone(),
            revision: v.revision.value(),
        }
    }
}

/// Request to update a configuration value.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateConfigValueRequest {
    pub value: Option<String>,
    pub clear_secret: bool,
    pub expected_revision: u64,
}

/// Stable API representation of a configuration definition.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigDefinitionDto {
    pub config_key: String,
    pub value_type: String,
    pub schema: Option<String>,
    pub default_value: String,
    pub sensitive: bool,
    pub dynamic: bool,
}

impl From<&domain_configuration::ConfigDefinition> for ConfigDefinitionDto {
    fn from(d: &domain_configuration::ConfigDefinition) -> Self {
        Self {
            config_key: d.config_key.clone(),
            value_type: d.value_type.as_str().to_string(),
            schema: d.schema.clone(),
            default_value: d.default_value.clone(),
            sensitive: d.sensitive,
            dynamic: d.dynamic,
        }
    }
}
