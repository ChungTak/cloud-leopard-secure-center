//! OpenAPI 3.1 document and snapshot helpers.

use utoipa::OpenApi;

use crate::{
    dto::{
        ApiKeyCreatedDto, AuditDetailsDto, AuditRecordDto, AuthExplainRequest, AuthExplainResponse,
        CameraDto, ChangeUserStatusRequest, ConfigDefinitionDto, ConfigScopeDto, ConfigValueDto,
        CreateOrganizationUnitRequest, CreateRoleBindingRequest, CreateRoleRequest,
        CreateServiceAccountRequest, CreateSpatialNodeRequest, CreateUserRequest, DeviceDto,
        HealthDto, ManageMfaRequest, MoveOrganizationUnitRequest, MoveSpatialNodeRequest,
        OrganizationUnitDto, ProblemDetailsDto, ResourceRefDto, RoleBindingDto,
        RoleBindingScopeDto, RoleDto, SetPasswordRequest, SpatialNodeDto, SpatialNodeType,
        TenantDto, UpdateOrganizationUnitRequest, UpdateRoleBindingRequest, UpdateRoleRequest,
        UpdateSpatialNodeRequest, UpdateUserRequest, UserDto,
    },
    routes,
};

/// Cloud Leopard Secure Center HTTP API.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Cloud Leopard Secure Center",
        description = "OpenAPI 3.1 surface for tenants, organization units, spatial nodes, users, service accounts, roles, role bindings, authorization explain, devices, cameras, audit records, and configuration.",
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
        routes::list_users,
        routes::create_user,
        routes::get_user,
        routes::update_user,
        routes::change_user_status,
        routes::set_user_password,
        routes::manage_user_mfa,
        routes::create_service_account,
        routes::list_roles,
        routes::create_role,
        routes::get_role,
        routes::update_role,
        routes::delete_role,
        routes::list_role_bindings,
        routes::create_role_binding,
        routes::get_role_binding,
        routes::update_role_binding,
        routes::delete_role_binding,
        routes::explain_auth,
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
        CreateUserRequest,
        UpdateUserRequest,
        ChangeUserStatusRequest,
        SetPasswordRequest,
        ManageMfaRequest,
        ApiKeyCreatedDto,
        CreateServiceAccountRequest,
        RoleDto,
        CreateRoleRequest,
        UpdateRoleRequest,
        RoleBindingDto,
        RoleBindingScopeDto,
        CreateRoleBindingRequest,
        UpdateRoleBindingRequest,
        ResourceRefDto,
        AuthExplainRequest,
        AuthExplainResponse,
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
