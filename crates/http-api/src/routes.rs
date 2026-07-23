//! HTTP route handlers.

use axum::{Json, Router, extract::Path, routing::get};

use crate::{
    dto::{
        AuditRecordDto, CameraDto, ConfigDefinitionDto, ConfigValueDto, DeviceDto, HealthDto,
        OrganizationUnitDto, ProblemDetailsDto, RoleBindingDto, RoleDto, TenantDto, UserDto,
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

/// Public router exposing health and resource definition stubs.
pub fn router() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/tenants/{id}", get(get_tenant))
        .route("/organization-units/{id}", get(get_organization_unit))
        .route("/users/{id}", get(get_user))
        .route("/roles/{id}", get(get_role))
        .route("/role-bindings/{id}", get(get_role_binding))
        .route("/devices/{id}", get(get_device))
        .route("/cameras/{id}", get(get_camera))
        .route("/audit-records/{id}", get(get_audit_record))
        .route("/config-values/{id}", get(get_config_value))
        .route("/config-definitions/{key}", get(get_config_definition))
}
