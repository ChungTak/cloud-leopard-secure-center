//! HTTP route handlers.

use axum::{
    Json, Router,
    extract::{Path, Query},
    routing::{delete, get, patch, post},
};

use crate::{
    dto::{
        ApiKeyCreatedDto, AuditRecordDto, AuthExplainRequest, AuthExplainResponse, CameraDto,
        ChangeDeviceLifecycleRequest, ChangeUserStatusRequest, ConfigDefinitionDto, ConfigValueDto,
        CreateCameraRequest, CreateDeviceRequest, CreateExternalBindingRequest,
        CreateOrganizationUnitRequest, CreateRoleBindingRequest, CreateRoleRequest,
        CreateServiceAccountRequest, CreateSpatialNodeRequest, CreateTagRequest, CreateUserRequest,
        DeviceDto, ExternalBindingDto, HealthDto, ManageMfaRequest, MoveOrganizationUnitRequest,
        MoveSpatialNodeRequest, OrganizationUnitDto, ProblemDetailsDto, ProjectionStateDto,
        ResolveExternalBindingConflictRequest, RoleBindingDto, RoleDto, SetPasswordRequest,
        SpatialNodeDto, TagDto, TenantDto, UpdateCameraRequest, UpdateConfigValueRequest,
        UpdateDeviceRequest, UpdateOrganizationUnitRequest, UpdateRoleBindingRequest,
        UpdateRoleRequest, UpdateSpatialNodeRequest, UpdateTagRequest, UpdateUserRequest, UserDto,
    },
    error::AppError,
};

/// Health check handler.
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy", body = HealthDto)
    )
)]
pub(crate) async fn health() -> Json<HealthDto> {
    Json(HealthDto {
        status: "healthy".to_string(),
        component: "http-api".to_string(),
    })
}

/// Get a tenant by id.
#[utoipa::path(
    get,
    path = "/tenants/{id}",
    responses(
        (status = 200, description = "OK", body = TenantDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn get_tenant(Path(_id): Path<String>) -> Result<Json<TenantDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Get an organization unit by id.
#[utoipa::path(
    get,
    path = "/organization-units/{id}",
    responses(
        (status = 200, description = "OK", body = OrganizationUnitDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn get_organization_unit(
    Path(_id): Path<String>,
) -> Result<Json<OrganizationUnitDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Get a user by id.
#[utoipa::path(
    get,
    path = "/users/{id}",
    responses(
        (status = 200, description = "OK", body = UserDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn get_user(Path(_id): Path<String>) -> Result<Json<UserDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Get a role by id.
#[utoipa::path(
    get,
    path = "/roles/{id}",
    responses(
        (status = 200, description = "OK", body = RoleDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn get_role(Path(_id): Path<String>) -> Result<Json<RoleDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Get a role binding by id.
#[utoipa::path(
    get,
    path = "/role-bindings/{id}",
    responses(
        (status = 200, description = "OK", body = RoleBindingDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn get_role_binding(
    Path(_id): Path<String>,
) -> Result<Json<RoleBindingDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Get a managed device by id.
#[utoipa::path(
    get,
    path = "/devices/{id}",
    responses(
        (status = 200, description = "OK", body = DeviceDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn get_device(Path(_id): Path<String>) -> Result<Json<DeviceDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Get a camera by id.
#[utoipa::path(
    get,
    path = "/cameras/{id}",
    responses(
        (status = 200, description = "OK", body = CameraDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn get_camera(Path(_id): Path<String>) -> Result<Json<CameraDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// List audit records.
#[utoipa::path(
    get,
    path = "/audit-records",
    params(
        ("targetType" = Option<String>, Query, description = "Target type"),
        ("targetId" = Option<String>, Query, description = "Target ID"),
        ("action" = Option<String>, Query, description = "Action filter"),
        ("from" = Option<String>, Query, description = "Start time (RFC 3339)"),
        ("to" = Option<String>, Query, description = "End time (RFC 3339)"),
        ("search" = Option<String>, Query, description = "Search term")
    ),
    responses(
        (status = 200, description = "OK", body = Vec<AuditRecordDto>),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn list_audit_records(
    Query(_q): Query<AuditListQuery>,
) -> Result<Json<Vec<AuditRecordDto>>, AppError> {
    Err(AppError::NotImplemented)
}

/// Get an audit record by id.
#[utoipa::path(
    get,
    path = "/audit-records/{id}",
    responses(
        (status = 200, description = "OK", body = AuditRecordDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn get_audit_record(
    Path(_id): Path<String>,
) -> Result<Json<AuditRecordDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Get a configuration value by id.
#[utoipa::path(
    get,
    path = "/config-values/{id}",
    responses(
        (status = 200, description = "OK", body = ConfigValueDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn get_config_value(
    Path(_id): Path<String>,
) -> Result<Json<ConfigValueDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Update a configuration value.
#[utoipa::path(
    patch,
    path = "/config-values/{id}",
    request_body = UpdateConfigValueRequest,
    responses(
        (status = 200, description = "OK", body = ConfigValueDto),
        (status = 412, description = "Precondition failed", body = ProblemDetailsDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn update_config_value(
    Path(_id): Path<String>,
    Json(_body): Json<UpdateConfigValueRequest>,
) -> Result<Json<ConfigValueDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// List configuration values.
#[utoipa::path(
    get,
    path = "/config-values",
    params(
        ("module" = Option<String>, Query, description = "Module filter"),
        ("search" = Option<String>, Query, description = "Search term")
    ),
    responses(
        (status = 200, description = "OK", body = Vec<ConfigValueDto>),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn list_config_values(
    Query(_q): Query<ConfigValueListQuery>,
) -> Result<Json<Vec<ConfigValueDto>>, AppError> {
    Err(AppError::NotImplemented)
}

/// Get a configuration definition by key.
#[utoipa::path(
    get,
    path = "/config-definitions/{key}",
    responses(
        (status = 200, description = "OK", body = ConfigDefinitionDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn get_config_definition(
    Path(_id): Path<String>,
) -> Result<Json<ConfigDefinitionDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// List configuration definitions.
#[utoipa::path(
    get,
    path = "/config-definitions",
    params(
        ("module" = Option<String>, Query, description = "Module filter"),
        ("search" = Option<String>, Query, description = "Search term")
    ),
    responses(
        (status = 200, description = "OK", body = Vec<ConfigDefinitionDto>),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn list_config_definitions(
    Query(_q): Query<ConfigDefinitionListQuery>,
) -> Result<Json<Vec<ConfigDefinitionDto>>, AppError> {
    Err(AppError::NotImplemented)
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub(crate) struct ListQuery {
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub node_type: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub(crate) struct DeviceListQuery {
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub organization_id: Option<String>,
    #[serde(default)]
    pub area_id: Option<String>,
    #[serde(default)]
    pub lifecycle: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub(crate) struct CameraListQuery {
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub device_id: Option<String>,
    #[serde(default)]
    pub area_id: Option<String>,
    #[serde(default)]
    pub sensitivity: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub(crate) struct TagListQuery {
    #[serde(default)]
    pub resource_type: Option<String>,
    #[serde(default)]
    pub resource_id: Option<String>,
    #[serde(default)]
    pub search: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub(crate) struct ExternalBindingListQuery {
    #[serde(default)]
    pub resource_type: Option<String>,
    #[serde(default)]
    pub resource_id: Option<String>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub(crate) struct ProjectionListQuery {
    #[serde(default)]
    pub device_id: Option<String>,
    #[serde(default)]
    pub is_stale: Option<bool>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub(crate) struct AuditListQuery {
    #[serde(default)]
    pub target_type: Option<String>,
    #[serde(default)]
    pub target_id: Option<String>,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub search: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub(crate) struct ConfigDefinitionListQuery {
    #[serde(default)]
    pub module: Option<String>,
    #[serde(default)]
    pub search: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub(crate) struct ConfigValueListQuery {
    #[serde(default)]
    pub module: Option<String>,
    #[serde(default)]
    pub search: Option<String>,
}

/// List organization units.
#[utoipa::path(
    get,
    path = "/organization-units",
    params(
        ("parentId" = Option<String>, Query, description = "Filter by parent id"),
        ("search" = Option<String>, Query, description = "Search term"),
    ),
    responses(
        (status = 200, description = "OK", body = Vec<OrganizationUnitDto>),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn list_organization_units(
    Query(_q): Query<ListQuery>,
) -> Result<Json<Vec<OrganizationUnitDto>>, AppError> {
    Err(AppError::NotImplemented)
}

/// Create an organization unit.
#[utoipa::path(
    post,
    path = "/organization-units",
    request_body = CreateOrganizationUnitRequest,
    responses(
        (status = 201, description = "Created", body = OrganizationUnitDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn create_organization_unit(
    Json(_body): Json<CreateOrganizationUnitRequest>,
) -> Result<Json<OrganizationUnitDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Update an organization unit.
#[utoipa::path(
    patch,
    path = "/organization-units/{id}",
    request_body = UpdateOrganizationUnitRequest,
    responses(
        (status = 200, description = "OK", body = OrganizationUnitDto),
        (status = 409, description = "Conflict", body = ProblemDetailsDto),
        (status = 412, description = "Precondition failed", body = ProblemDetailsDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn update_organization_unit(
    Path(_id): Path<String>,
    Json(_body): Json<UpdateOrganizationUnitRequest>,
) -> Result<Json<OrganizationUnitDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Move an organization unit.
#[utoipa::path(
    post,
    path = "/organization-units/{id}/move",
    request_body = MoveOrganizationUnitRequest,
    responses(
        (status = 200, description = "OK", body = OrganizationUnitDto),
        (status = 409, description = "Conflict", body = ProblemDetailsDto),
        (status = 412, description = "Precondition failed", body = ProblemDetailsDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn move_organization_unit(
    Path(_id): Path<String>,
    Json(_body): Json<MoveOrganizationUnitRequest>,
) -> Result<Json<OrganizationUnitDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Delete an organization unit.
#[utoipa::path(
    delete,
    path = "/organization-units/{id}",
    responses(
        (status = 204, description = "No content"),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn delete_organization_unit(Path(_id): Path<String>) -> Result<(), AppError> {
    Err(AppError::NotImplemented)
}

/// List spatial nodes.
#[utoipa::path(
    get,
    path = "/spatial-nodes",
    params(
        ("parentId" = Option<String>, Query, description = "Filter by parent id"),
        ("search" = Option<String>, Query, description = "Search term"),
        ("nodeType" = Option<String>, Query, description = "Filter by node type"),
    ),
    responses(
        (status = 200, description = "OK", body = Vec<SpatialNodeDto>),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn list_spatial_nodes(
    Query(_q): Query<ListQuery>,
) -> Result<Json<Vec<SpatialNodeDto>>, AppError> {
    Err(AppError::NotImplemented)
}

/// Create a spatial node.
#[utoipa::path(
    post,
    path = "/spatial-nodes",
    request_body = CreateSpatialNodeRequest,
    responses(
        (status = 201, description = "Created", body = SpatialNodeDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn create_spatial_node(
    Json(_body): Json<CreateSpatialNodeRequest>,
) -> Result<Json<SpatialNodeDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Get a spatial node by id.
#[utoipa::path(
    get,
    path = "/spatial-nodes/{id}",
    responses(
        (status = 200, description = "OK", body = SpatialNodeDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn get_spatial_node(
    Path(_id): Path<String>,
) -> Result<Json<SpatialNodeDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Update a spatial node.
#[utoipa::path(
    patch,
    path = "/spatial-nodes/{id}",
    request_body = UpdateSpatialNodeRequest,
    responses(
        (status = 200, description = "OK", body = SpatialNodeDto),
        (status = 409, description = "Conflict", body = ProblemDetailsDto),
        (status = 412, description = "Precondition failed", body = ProblemDetailsDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn update_spatial_node(
    Path(_id): Path<String>,
    Json(_body): Json<UpdateSpatialNodeRequest>,
) -> Result<Json<SpatialNodeDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Move a spatial node.
#[utoipa::path(
    post,
    path = "/spatial-nodes/{id}/move",
    request_body = MoveSpatialNodeRequest,
    responses(
        (status = 200, description = "OK", body = SpatialNodeDto),
        (status = 409, description = "Conflict", body = ProblemDetailsDto),
        (status = 412, description = "Precondition failed", body = ProblemDetailsDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn move_spatial_node(
    Path(_id): Path<String>,
    Json(_body): Json<MoveSpatialNodeRequest>,
) -> Result<Json<SpatialNodeDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Delete a spatial node.
#[utoipa::path(
    delete,
    path = "/spatial-nodes/{id}",
    responses(
        (status = 204, description = "No content"),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn delete_spatial_node(Path(_id): Path<String>) -> Result<(), AppError> {
    Err(AppError::NotImplemented)
}

/// List users.
#[utoipa::path(
    get,
    path = "/users",
    params(
        ("search" = Option<String>, Query, description = "Search term"),
        ("status" = Option<String>, Query, description = "User status")
    ),
    responses(
        (status = 200, description = "OK", body = Vec<UserDto>),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn list_users(
    Query(_q): Query<ListQuery>,
) -> Result<Json<Vec<UserDto>>, AppError> {
    Err(AppError::NotImplemented)
}

/// Create a user.
#[utoipa::path(
    post,
    path = "/users",
    request_body = CreateUserRequest,
    responses(
        (status = 201, description = "Created", body = UserDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn create_user(
    Json(_body): Json<CreateUserRequest>,
) -> Result<Json<UserDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Update a user.
#[utoipa::path(
    patch,
    path = "/users/{id}",
    request_body = UpdateUserRequest,
    responses(
        (status = 200, description = "OK", body = UserDto),
        (status = 409, description = "Conflict", body = ProblemDetailsDto),
        (status = 412, description = "Precondition failed", body = ProblemDetailsDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn update_user(
    Path(_id): Path<String>,
    Json(_body): Json<UpdateUserRequest>,
) -> Result<Json<UserDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Change a user's status.
#[utoipa::path(
    post,
    path = "/users/{id}/status",
    request_body = ChangeUserStatusRequest,
    responses(
        (status = 200, description = "OK", body = UserDto),
        (status = 409, description = "Conflict", body = ProblemDetailsDto),
        (status = 412, description = "Precondition failed", body = ProblemDetailsDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn change_user_status(
    Path(_id): Path<String>,
    Json(_body): Json<ChangeUserStatusRequest>,
) -> Result<Json<UserDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Set a user's password.
#[utoipa::path(
    post,
    path = "/users/{id}/password",
    request_body = SetPasswordRequest,
    responses(
        (status = 204, description = "No content"),
        (status = 412, description = "Precondition failed", body = ProblemDetailsDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn set_user_password(
    Path(_id): Path<String>,
    Json(_body): Json<SetPasswordRequest>,
) -> Result<(), AppError> {
    Err(AppError::NotImplemented)
}

/// Manage a user's MFA.
#[utoipa::path(
    post,
    path = "/users/{id}/mfa",
    request_body = ManageMfaRequest,
    responses(
        (status = 200, description = "OK", body = UserDto),
        (status = 409, description = "Conflict", body = ProblemDetailsDto),
        (status = 412, description = "Precondition failed", body = ProblemDetailsDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn manage_user_mfa(
    Path(_id): Path<String>,
    Json(_body): Json<ManageMfaRequest>,
) -> Result<Json<UserDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Create a service account.
#[utoipa::path(
    post,
    path = "/service-accounts",
    request_body = CreateServiceAccountRequest,
    responses(
        (status = 201, description = "Created", body = ApiKeyCreatedDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn create_service_account(
    Json(_body): Json<CreateServiceAccountRequest>,
) -> Result<Json<ApiKeyCreatedDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// List roles.
#[utoipa::path(
    get,
    path = "/roles",
    params(
        ("search" = Option<String>, Query, description = "Search term")
    ),
    responses(
        (status = 200, description = "OK", body = Vec<RoleDto>),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn list_roles(
    Query(_q): Query<ListQuery>,
) -> Result<Json<Vec<RoleDto>>, AppError> {
    Err(AppError::NotImplemented)
}

/// Create a role.
#[utoipa::path(
    post,
    path = "/roles",
    request_body = CreateRoleRequest,
    responses(
        (status = 201, description = "Created", body = RoleDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn create_role(
    Json(_body): Json<CreateRoleRequest>,
) -> Result<Json<RoleDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Update a role.
#[utoipa::path(
    patch,
    path = "/roles/{id}",
    request_body = UpdateRoleRequest,
    responses(
        (status = 200, description = "OK", body = RoleDto),
        (status = 409, description = "Conflict", body = ProblemDetailsDto),
        (status = 412, description = "Precondition failed", body = ProblemDetailsDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn update_role(
    Path(_id): Path<String>,
    Json(_body): Json<UpdateRoleRequest>,
) -> Result<Json<RoleDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Delete a role.
#[utoipa::path(
    delete,
    path = "/roles/{id}",
    responses(
        (status = 204, description = "No content"),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn delete_role(Path(_id): Path<String>) -> Result<(), AppError> {
    Err(AppError::NotImplemented)
}

/// List role bindings.
#[utoipa::path(
    get,
    path = "/role-bindings",
    params(
        ("search" = Option<String>, Query, description = "Search term"),
        ("principalId" = Option<String>, Query, description = "Principal filter"),
        ("roleId" = Option<String>, Query, description = "Role filter")
    ),
    responses(
        (status = 200, description = "OK", body = Vec<RoleBindingDto>),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn list_role_bindings(
    Query(_q): Query<ListQuery>,
) -> Result<Json<Vec<RoleBindingDto>>, AppError> {
    Err(AppError::NotImplemented)
}

/// Create a role binding.
#[utoipa::path(
    post,
    path = "/role-bindings",
    request_body = CreateRoleBindingRequest,
    responses(
        (status = 201, description = "Created", body = RoleBindingDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn create_role_binding(
    Json(_body): Json<CreateRoleBindingRequest>,
) -> Result<Json<RoleBindingDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Update a role binding.
#[utoipa::path(
    patch,
    path = "/role-bindings/{id}",
    request_body = UpdateRoleBindingRequest,
    responses(
        (status = 200, description = "OK", body = RoleBindingDto),
        (status = 409, description = "Conflict", body = ProblemDetailsDto),
        (status = 412, description = "Precondition failed", body = ProblemDetailsDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn update_role_binding(
    Path(_id): Path<String>,
    Json(_body): Json<UpdateRoleBindingRequest>,
) -> Result<Json<RoleBindingDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Delete a role binding.
#[utoipa::path(
    delete,
    path = "/role-bindings/{id}",
    responses(
        (status = 204, description = "No content"),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn delete_role_binding(Path(_id): Path<String>) -> Result<(), AppError> {
    Err(AppError::NotImplemented)
}

/// Explain an authorization decision.
#[utoipa::path(
    post,
    path = "/auth/explain",
    request_body = AuthExplainRequest,
    responses(
        (status = 200, description = "OK", body = AuthExplainResponse),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn explain_auth(
    Json(_body): Json<AuthExplainRequest>,
) -> Result<Json<AuthExplainResponse>, AppError> {
    Err(AppError::NotImplemented)
}

/// List managed devices.
#[utoipa::path(
    get,
    path = "/devices",
    params(
        ("search" = Option<String>, Query, description = "Search term"),
        ("organizationId" = Option<String>, Query, description = "Organization filter"),
        ("areaId" = Option<String>, Query, description = "Area filter"),
        ("lifecycle" = Option<String>, Query, description = "Lifecycle filter")
    ),
    responses(
        (status = 200, description = "OK", body = Vec<DeviceDto>),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn list_devices(
    Query(_q): Query<DeviceListQuery>,
) -> Result<Json<Vec<DeviceDto>>, AppError> {
    Err(AppError::NotImplemented)
}

/// Create a managed device.
#[utoipa::path(
    post,
    path = "/devices",
    request_body = CreateDeviceRequest,
    responses(
        (status = 201, description = "Created", body = DeviceDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn create_device(
    Json(_body): Json<CreateDeviceRequest>,
) -> Result<Json<DeviceDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Update a managed device.
#[utoipa::path(
    patch,
    path = "/devices/{id}",
    request_body = UpdateDeviceRequest,
    responses(
        (status = 200, description = "OK", body = DeviceDto),
        (status = 409, description = "Conflict", body = ProblemDetailsDto),
        (status = 412, description = "Precondition failed", body = ProblemDetailsDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn update_device(
    Path(_id): Path<String>,
    Json(_body): Json<UpdateDeviceRequest>,
) -> Result<Json<DeviceDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Change a device lifecycle.
#[utoipa::path(
    post,
    path = "/devices/{id}/lifecycle",
    request_body = ChangeDeviceLifecycleRequest,
    responses(
        (status = 200, description = "OK", body = DeviceDto),
        (status = 412, description = "Precondition failed", body = ProblemDetailsDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn change_device_lifecycle(
    Path(_id): Path<String>,
    Json(_body): Json<ChangeDeviceLifecycleRequest>,
) -> Result<Json<DeviceDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Delete a managed device.
#[utoipa::path(
    delete,
    path = "/devices/{id}",
    responses(
        (status = 204, description = "No content"),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn delete_device(Path(_id): Path<String>) -> Result<(), AppError> {
    Err(AppError::NotImplemented)
}

/// List cameras.
#[utoipa::path(
    get,
    path = "/cameras",
    params(
        ("search" = Option<String>, Query, description = "Search term"),
        ("deviceId" = Option<String>, Query, description = "Device filter"),
        ("areaId" = Option<String>, Query, description = "Area filter"),
        ("sensitivity" = Option<String>, Query, description = "Sensitivity filter")
    ),
    responses(
        (status = 200, description = "OK", body = Vec<CameraDto>),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn list_cameras(
    Query(_q): Query<CameraListQuery>,
) -> Result<Json<Vec<CameraDto>>, AppError> {
    Err(AppError::NotImplemented)
}

/// Create a camera.
#[utoipa::path(
    post,
    path = "/cameras",
    request_body = CreateCameraRequest,
    responses(
        (status = 201, description = "Created", body = CameraDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn create_camera(
    Json(_body): Json<CreateCameraRequest>,
) -> Result<Json<CameraDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Update a camera.
#[utoipa::path(
    patch,
    path = "/cameras/{id}",
    request_body = UpdateCameraRequest,
    responses(
        (status = 200, description = "OK", body = CameraDto),
        (status = 409, description = "Conflict", body = ProblemDetailsDto),
        (status = 412, description = "Precondition failed", body = ProblemDetailsDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn update_camera(
    Path(_id): Path<String>,
    Json(_body): Json<UpdateCameraRequest>,
) -> Result<Json<CameraDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Delete a camera.
#[utoipa::path(
    delete,
    path = "/cameras/{id}",
    responses(
        (status = 204, description = "No content"),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn delete_camera(Path(_id): Path<String>) -> Result<(), AppError> {
    Err(AppError::NotImplemented)
}

/// List tags for a resource.
#[utoipa::path(
    get,
    path = "/tags",
    params(
        ("resourceType" = Option<String>, Query, description = "Resource type"),
        ("resourceId" = Option<String>, Query, description = "Resource ID"),
        ("search" = Option<String>, Query, description = "Search term")
    ),
    responses(
        (status = 200, description = "OK", body = Vec<TagDto>),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn list_tags(
    Query(_q): Query<TagListQuery>,
) -> Result<Json<Vec<TagDto>>, AppError> {
    Err(AppError::NotImplemented)
}

/// Create a tag.
#[utoipa::path(
    post,
    path = "/tags",
    request_body = CreateTagRequest,
    responses(
        (status = 201, description = "Created", body = TagDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn create_tag(
    Json(_body): Json<CreateTagRequest>,
) -> Result<Json<TagDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Update a tag.
#[utoipa::path(
    patch,
    path = "/tags/{id}",
    request_body = UpdateTagRequest,
    responses(
        (status = 200, description = "OK", body = TagDto),
        (status = 412, description = "Precondition failed", body = ProblemDetailsDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn update_tag(
    Path(_id): Path<String>,
    Json(_body): Json<UpdateTagRequest>,
) -> Result<Json<TagDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Delete a tag.
#[utoipa::path(
    delete,
    path = "/tags/{id}",
    responses(
        (status = 204, description = "No content"),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn delete_tag(Path(_id): Path<String>) -> Result<(), AppError> {
    Err(AppError::NotImplemented)
}

/// List external bindings.
#[utoipa::path(
    get,
    path = "/external-bindings",
    params(
        ("resourceType" = Option<String>, Query, description = "Resource type"),
        ("resourceId" = Option<String>, Query, description = "Resource ID"),
        ("search" = Option<String>, Query, description = "Search term"),
        ("state" = Option<String>, Query, description = "State filter")
    ),
    responses(
        (status = 200, description = "OK", body = Vec<ExternalBindingDto>),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn list_external_bindings(
    Query(_q): Query<ExternalBindingListQuery>,
) -> Result<Json<Vec<ExternalBindingDto>>, AppError> {
    Err(AppError::NotImplemented)
}

/// Create an external binding.
#[utoipa::path(
    post,
    path = "/external-bindings",
    request_body = CreateExternalBindingRequest,
    responses(
        (status = 201, description = "Created", body = ExternalBindingDto),
        (status = 409, description = "Conflict", body = ProblemDetailsDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn create_external_binding(
    Json(_body): Json<CreateExternalBindingRequest>,
) -> Result<Json<ExternalBindingDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Resolve an external binding conflict.
#[utoipa::path(
    post,
    path = "/external-bindings/{id}/resolve",
    request_body = ResolveExternalBindingConflictRequest,
    responses(
        (status = 200, description = "OK", body = ExternalBindingDto),
        (status = 409, description = "Conflict", body = ProblemDetailsDto),
        (status = 412, description = "Precondition failed", body = ProblemDetailsDto),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn resolve_external_binding_conflict(
    Path(_id): Path<String>,
    Json(_body): Json<ResolveExternalBindingConflictRequest>,
) -> Result<Json<ExternalBindingDto>, AppError> {
    Err(AppError::NotImplemented)
}

/// Delete an external binding.
#[utoipa::path(
    delete,
    path = "/external-bindings/{id}",
    responses(
        (status = 204, description = "No content"),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn delete_external_binding(Path(_id): Path<String>) -> Result<(), AppError> {
    Err(AppError::NotImplemented)
}

/// List projection states.
#[utoipa::path(
    get,
    path = "/projections",
    params(
        ("deviceId" = Option<String>, Query, description = "Device filter"),
        ("isStale" = Option<bool>, Query, description = "Stale filter")
    ),
    responses(
        (status = 200, description = "OK", body = Vec<ProjectionStateDto>),
        (status = 501, description = "Not implemented", body = ProblemDetailsDto)
    )
)]
pub(crate) async fn list_projections(
    Query(_q): Query<ProjectionListQuery>,
) -> Result<Json<Vec<ProjectionStateDto>>, AppError> {
    Err(AppError::NotImplemented)
}

/// Public router exposing health and resource definition stubs.
pub fn router() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/tenants/{id}", get(get_tenant))
        .route(
            "/organization-units",
            post(create_organization_unit).get(list_organization_units),
        )
        .route(
            "/organization-units/{id}",
            get(get_organization_unit)
                .patch(update_organization_unit)
                .delete(delete_organization_unit),
        )
        .route(
            "/organization-units/{id}/move",
            post(move_organization_unit),
        )
        .route(
            "/spatial-nodes",
            post(create_spatial_node).get(list_spatial_nodes),
        )
        .route(
            "/spatial-nodes/{id}",
            get(get_spatial_node)
                .patch(update_spatial_node)
                .delete(delete_spatial_node),
        )
        .route("/spatial-nodes/{id}/move", post(move_spatial_node))
        .route("/service-accounts", post(create_service_account))
        .route("/users", post(create_user).get(list_users))
        .route("/users/{id}", get(get_user).patch(update_user))
        .route("/users/{id}/status", post(change_user_status))
        .route("/users/{id}/password", post(set_user_password))
        .route("/users/{id}/mfa", post(manage_user_mfa))
        .route("/roles", post(create_role).get(list_roles))
        .route(
            "/roles/{id}",
            get(get_role).patch(update_role).delete(delete_role),
        )
        .route(
            "/role-bindings",
            post(create_role_binding).get(list_role_bindings),
        )
        .route(
            "/role-bindings/{id}",
            get(get_role_binding)
                .patch(update_role_binding)
                .delete(delete_role_binding),
        )
        .route("/auth/explain", post(explain_auth))
        .route("/devices", post(create_device).get(list_devices))
        .route(
            "/devices/{id}",
            get(get_device).patch(update_device).delete(delete_device),
        )
        .route("/devices/{id}/lifecycle", post(change_device_lifecycle))
        .route("/cameras", post(create_camera).get(list_cameras))
        .route(
            "/cameras/{id}",
            get(get_camera).patch(update_camera).delete(delete_camera),
        )
        .route("/tags", post(create_tag).get(list_tags))
        .route("/tags/{id}", patch(update_tag).delete(delete_tag))
        .route(
            "/external-bindings",
            post(create_external_binding).get(list_external_bindings),
        )
        .route("/external-bindings/{id}", delete(delete_external_binding))
        .route(
            "/external-bindings/{id}/resolve",
            post(resolve_external_binding_conflict),
        )
        .route("/projections", get(list_projections))
        .route("/audit-records", get(list_audit_records))
        .route("/audit-records/{id}", get(get_audit_record))
        .route("/config-values", get(list_config_values))
        .route(
            "/config-values/{id}",
            get(get_config_value).patch(update_config_value),
        )
        .route("/config-definitions", get(list_config_definitions))
        .route("/config-definitions/{key}", get(get_config_definition))
}
