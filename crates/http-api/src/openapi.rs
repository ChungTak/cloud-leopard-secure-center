//! OpenAPI 3.1 document and snapshot helpers.

use utoipa::OpenApi;

use crate::{
    dto::{
        AuditDetailsDto, AuditRecordDto, CameraDto, ConfigDefinitionDto, ConfigScopeDto,
        ConfigValueDto, CreateOrganizationUnitRequest, CreateSpatialNodeRequest, DeviceDto,
        HealthDto, MoveOrganizationUnitRequest, MoveSpatialNodeRequest, OrganizationUnitDto,
        ProblemDetailsDto, ResourceRefDto, RoleBindingDto, RoleBindingScopeDto, RoleDto,
        SpatialNodeDto, SpatialNodeType, TenantDto, UpdateOrganizationUnitRequest,
        UpdateSpatialNodeRequest, UserDto,
    },
    routes,
};

/// Cloud Leopard Secure Center HTTP API.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Cloud Leopard Secure Center",
        description = "OpenAPI 3.1 surface for tenants, organization units, spatial nodes, users, roles, role bindings, devices, cameras, audit records, and configuration.",
        version = "0.1.0"
    ),
    paths(
        routes::health,
        routes::get_tenant,
        routes::get_organization_unit,
        routes::list_organization_units,
        routes::create_organization_unit,
        routes::update_organization_unit,
        routes::move_organization_unit,
        routes::delete_organization_unit,
        routes::get_spatial_node,
        routes::list_spatial_nodes,
        routes::create_spatial_node,
        routes::update_spatial_node,
        routes::move_spatial_node,
        routes::delete_spatial_node,
        routes::get_user,
        routes::get_role,
        routes::get_role_binding,
        routes::get_device,
        routes::get_camera,
        routes::get_audit_record,
        routes::get_config_value,
        routes::get_config_definition
    ),
    components(schemas(
        HealthDto,
        ProblemDetailsDto,
        TenantDto,
        OrganizationUnitDto,
        CreateOrganizationUnitRequest,
        UpdateOrganizationUnitRequest,
        MoveOrganizationUnitRequest,
        SpatialNodeDto,
        SpatialNodeType,
        CreateSpatialNodeRequest,
        UpdateSpatialNodeRequest,
        MoveSpatialNodeRequest,
        UserDto,
        RoleDto,
        RoleBindingDto,
        RoleBindingScopeDto,
        ResourceRefDto,
        DeviceDto,
        CameraDto,
        AuditRecordDto,
        AuditDetailsDto,
        ConfigValueDto,
        ConfigScopeDto,
        ConfigDefinitionDto
    ))
)]
pub struct ApiDoc;

impl ApiDoc {
    /// Return the OpenAPI document as a pretty-printed JSON string.
    pub fn json() -> String {
        ApiDoc::openapi().to_pretty_json().unwrap_or_default()
    }
}
