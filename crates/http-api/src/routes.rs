//! HTTP route handlers.

use axum::{
    Json, Router,
    extract::{Path, Query},
    routing::{get, post},
};

use crate::{
    dto::{
        AuditRecordDto, CameraDto, ConfigDefinitionDto, ConfigValueDto,
        CreateOrganizationUnitRequest, CreateSpatialNodeRequest, DeviceDto, HealthDto,
        MoveOrganizationUnitRequest, MoveSpatialNodeRequest, OrganizationUnitDto,
        ProblemDetailsDto, RoleBindingDto, RoleDto, SpatialNodeDto, TenantDto,
        UpdateOrganizationUnitRequest, UpdateSpatialNodeRequest, UserDto,
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
        .route("/users/{id}", get(get_user))
        .route("/roles/{id}", get(get_role))
        .route("/role-bindings/{id}", get(get_role_binding))
        .route("/devices/{id}", get(get_device))
        .route("/cameras/{id}", get(get_camera))
        .route("/audit-records/{id}", get(get_audit_record))
        .route("/config-values/{id}", get(get_config_value))
        .route("/config-definitions/{key}", get(get_config_definition))
}
