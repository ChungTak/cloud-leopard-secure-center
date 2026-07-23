//! Storage port traits (repository contract, unit of work).

use async_trait::async_trait;
use domain_audit::audit_record::{AuditRecord, AuditRecordId};
use domain_authorization::role::Role;
use domain_authorization::role_binding::RoleBinding;
use domain_identity::api_key::ApiKey;
use domain_identity::credential::Credential;
use domain_identity::mfa::MfaFactor;
use domain_identity::session::RefreshToken;
use domain_identity::user::User;
use domain_organization::organization_unit::OrganizationUnit;
use domain_organization::spatial::{Area, Building, Floor, Site};
use domain_organization::tenant::Tenant;
use domain_resource::camera::Camera;
use domain_resource::device::ManagedDevice;
use domain_resource::external_binding::ExternalBinding;
use domain_resource::projection::{
    ChannelEvent, ChannelProjection, DeviceEvent, DeviceProjection, ProjectionFailure,
};
use domain_resource::tag::{ResourceType, Tag};
use foundation::{
    AreaId, BindingId, BuildingId, CameraId, DeviceId, ExternalBindingId, FloorId, OrganizationId,
    PlatformError, RequestContext, Revision, RoleId, SiteId, TagId, TenantId, UserId, UtcTimestamp,
    uuid::Uuid,
};

/// Page of results returned by a repository list query.
#[derive(Debug, Clone)]
pub struct Page<T> {
    /// Items in the page.
    pub items: Vec<T>,
    /// Opaque cursor for the next page, if any.
    pub next_cursor: Option<String>,
}

/// Unit of work boundary. Implementations wrap a transaction.
#[async_trait]
pub trait UnitOfWork: Send + Sync {
    /// Commit the transaction.
    async fn commit(self) -> Result<(), PlatformError>;
    /// Rollback the transaction.
    async fn rollback(self) -> Result<(), PlatformError>;
}

/// Repository contract for the `Tenant` aggregate.
///
/// All mutating methods take an `expected` revision and return
/// `REVISION_CONFLICT` or `NOT_FOUND` when the row is missing or stale.
#[async_trait]
pub trait TenantRepository: Send + Sync {
    /// Find a tenant by id, honoring the tenant context in `ctx`.
    async fn by_id(&self, id: TenantId, ctx: &RequestContext) -> Result<Tenant, PlatformError>;

    /// Persist a new tenant.
    async fn create(&self, tenant: &Tenant, ctx: &RequestContext) -> Result<(), PlatformError>;

    /// Update an existing tenant, failing if `expected` does not match.
    async fn update(
        &self,
        tenant: &Tenant,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Soft-delete a tenant by id when the current revision matches `expected`.
    async fn delete(
        &self,
        id: TenantId,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// List tenants visible in the current tenant context.
    async fn list(&self, ctx: &RequestContext) -> Result<Page<Tenant>, PlatformError>;
}

/// Repository contract for the `OrganizationUnit` aggregate.
///
/// All mutating methods take an `expected` revision and return
/// `REVISION_CONFLICT` or `NOT_FOUND` when the row is missing or stale.
#[async_trait]
pub trait OrganizationUnitRepository: Send + Sync {
    /// Find an organization unit by id.
    async fn by_id(
        &self,
        id: OrganizationId,
        ctx: &RequestContext,
    ) -> Result<OrganizationUnit, PlatformError>;

    /// Persist a new organization unit and its closure entries.
    async fn create(
        &self,
        unit: &OrganizationUnit,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Update an organization unit, including closure changes when the parent moves.
    async fn update(
        &self,
        unit: &OrganizationUnit,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Soft-delete an organization unit when it has no children.
    async fn delete(
        &self,
        id: OrganizationId,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// List organization units in the current tenant context, ordered by code.
    async fn list(&self, ctx: &RequestContext) -> Result<Page<OrganizationUnit>, PlatformError>;

    /// Returns `true` if `descendant` is `ancestor` itself or a descendant in the closure table.
    async fn is_descendant_of(
        &self,
        ancestor: OrganizationId,
        descendant: OrganizationId,
        ctx: &RequestContext,
    ) -> Result<bool, PlatformError>;
}

/// Repository contract for the `Site`, `Building`, `Floor`, and `Area` aggregates.
///
/// All mutating methods take an `expected` revision and return
/// `REVISION_CONFLICT` or `NOT_FOUND` when the row is missing or stale.
#[async_trait]
pub trait SpatialRepository: Send + Sync {
    // Site
    async fn site_by_id(&self, id: SiteId, ctx: &RequestContext) -> Result<Site, PlatformError>;
    async fn create_site(&self, site: &Site, ctx: &RequestContext) -> Result<(), PlatformError>;
    async fn update_site(
        &self,
        site: &Site,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;
    async fn delete_site(
        &self,
        id: SiteId,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;
    async fn list_sites(&self, ctx: &RequestContext) -> Result<Page<Site>, PlatformError>;

    // Building
    async fn building_by_id(
        &self,
        id: BuildingId,
        ctx: &RequestContext,
    ) -> Result<Building, PlatformError>;
    async fn create_building(
        &self,
        building: &Building,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;
    async fn update_building(
        &self,
        building: &Building,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;
    async fn delete_building(
        &self,
        id: BuildingId,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;
    async fn list_buildings(&self, ctx: &RequestContext) -> Result<Page<Building>, PlatformError>;

    // Floor
    async fn floor_by_id(&self, id: FloorId, ctx: &RequestContext) -> Result<Floor, PlatformError>;
    async fn create_floor(&self, floor: &Floor, ctx: &RequestContext) -> Result<(), PlatformError>;
    async fn update_floor(
        &self,
        floor: &Floor,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;
    async fn delete_floor(
        &self,
        id: FloorId,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;
    async fn list_floors(&self, ctx: &RequestContext) -> Result<Page<Floor>, PlatformError>;

    // Area
    async fn area_by_id(&self, id: AreaId, ctx: &RequestContext) -> Result<Area, PlatformError>;
    async fn create_area(&self, area: &Area, ctx: &RequestContext) -> Result<(), PlatformError>;
    async fn update_area(
        &self,
        area: &Area,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;
    async fn delete_area(
        &self,
        id: AreaId,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;
    async fn list_areas(&self, ctx: &RequestContext) -> Result<Page<Area>, PlatformError>;
    async fn areas_within_radius(
        &self,
        latitude: f64,
        longitude: f64,
        radius_meters: f64,
        ctx: &RequestContext,
    ) -> Result<Page<Area>, PlatformError>;

    /// Returns `true` if `descendant` is `ancestor` itself or a descendant in the area closure table.
    async fn is_area_descendant_of(
        &self,
        ancestor: AreaId,
        descendant: AreaId,
        ctx: &RequestContext,
    ) -> Result<bool, PlatformError>;
}

/// Repository contract for the `User` aggregate.
///
/// All mutating methods take an `expected` revision and return
/// `REVISION_CONFLICT` or `NOT_FOUND` when the row is missing or stale.
#[async_trait]
pub trait UserRepository: Send + Sync {
    /// Find a user by id, honoring the tenant context in `ctx`.
    async fn by_id(&self, id: UserId, ctx: &RequestContext) -> Result<User, PlatformError>;

    /// Find a user by normalized username.
    async fn by_username(
        &self,
        username: &str,
        ctx: &RequestContext,
    ) -> Result<User, PlatformError>;

    /// Persist a new user.
    async fn create(&self, user: &User, ctx: &RequestContext) -> Result<(), PlatformError>;

    /// Update an existing user, failing if `expected` does not match.
    async fn update(
        &self,
        user: &User,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Soft-delete a user by id when the current revision matches `expected`.
    async fn delete(
        &self,
        id: UserId,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// List users visible in the current tenant context.
    async fn list(&self, ctx: &RequestContext) -> Result<Page<User>, PlatformError>;
}

/// Repository contract for a user's `Credential`.
#[async_trait]
pub trait CredentialRepository: Send + Sync {
    /// Find the credential for a user by type.
    async fn by_user_and_type(
        &self,
        user_id: UserId,
        credential_type: &str,
        ctx: &RequestContext,
    ) -> Result<Credential, PlatformError>;

    /// Persist a new credential.
    async fn create(
        &self,
        credential: &Credential,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Update an existing credential.
    async fn update(
        &self,
        credential: &Credential,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Remove a user's credential.
    async fn delete(
        &self,
        user_id: UserId,
        credential_type: &str,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;
}

/// Repository contract for recording and querying login attempts.
#[async_trait]
pub trait LoginAttemptRepository: Send + Sync {
    /// Record a login attempt for audit and rate-limiting.
    async fn record(
        &self,
        tenant_id: TenantId,
        identity: &str,
        ip: Option<String>,
        success: bool,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Count recent failed attempts for the given identity.
    async fn count_failures_by_identity(
        &self,
        tenant_id: TenantId,
        identity: &str,
        window_seconds: i64,
        ctx: &RequestContext,
    ) -> Result<i64, PlatformError>;

    /// Count recent failed attempts from the given source IP.
    async fn count_failures_by_source(
        &self,
        tenant_id: TenantId,
        ip: String,
        window_seconds: i64,
        ctx: &RequestContext,
    ) -> Result<i64, PlatformError>;
}

/// Repository contract for sessions and refresh token families.
#[async_trait]
pub trait SessionRepository: Send + Sync {
    /// Persist a new refresh token.
    async fn save_refresh_token(
        &self,
        token: &RefreshToken,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Find a refresh token by its hash.
    async fn find_refresh_token_by_hash(
        &self,
        token_hash: &str,
        ctx: &RequestContext,
    ) -> Result<Option<RefreshToken>, PlatformError>;

    /// Mark a refresh token as used. Returns `Conflict` if already used.
    async fn mark_refresh_token_used(
        &self,
        token: &RefreshToken,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Revoke every token in the given family.
    async fn revoke_family(
        &self,
        family_id: Uuid,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Revoke all sessions for a user.
    async fn revoke_user_sessions(
        &self,
        user_id: UserId,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;
}

/// Repository contract for API keys.
#[async_trait]
pub trait ApiKeyRepository: Send + Sync {
    /// Persist a new API key.
    async fn create(&self, api_key: &ApiKey, ctx: &RequestContext) -> Result<(), PlatformError>;

    /// Find an API key by id.
    async fn by_id(&self, id: Uuid, ctx: &RequestContext) -> Result<ApiKey, PlatformError>;

    /// Find an API key by its token hash.
    async fn by_token_hash(
        &self,
        token_hash: &str,
        ctx: &RequestContext,
    ) -> Result<Option<ApiKey>, PlatformError>;

    /// Update an existing API key.
    async fn update(&self, api_key: &ApiKey, ctx: &RequestContext) -> Result<(), PlatformError>;

    /// List API keys for an owner.
    async fn list_by_owner(
        &self,
        owner_id: UserId,
        ctx: &RequestContext,
    ) -> Result<Page<ApiKey>, PlatformError>;
}

/// Repository contract for MFA factors.
#[async_trait]
pub trait MfaRepository: Send + Sync {
    /// Persist a new factor and its recovery codes.
    async fn save_factor(
        &self,
        factor: &MfaFactor,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Update an existing factor.
    async fn update_factor(
        &self,
        factor: &MfaFactor,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Find the active factor for a user, if any.
    async fn find_active_factor_by_user(
        &self,
        user_id: UserId,
        ctx: &RequestContext,
    ) -> Result<Option<MfaFactor>, PlatformError>;
}

/// Repository contract for the `Role` aggregate.
#[async_trait]
pub trait RoleRepository: Send + Sync {
    /// Find a role by id, including its permissions.
    async fn by_id(&self, id: RoleId, ctx: &RequestContext) -> Result<Role, PlatformError>;

    /// Persist a new role and its permissions.
    async fn create(&self, role: &Role, ctx: &RequestContext) -> Result<(), PlatformError>;

    /// Update an existing role and its permissions, failing if `expected` does not match.
    async fn update(
        &self,
        role: &Role,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Soft-delete a role by id.
    async fn delete(
        &self,
        id: RoleId,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// List roles in the current tenant context.
    async fn list(&self, ctx: &RequestContext) -> Result<Page<Role>, PlatformError>;
}

/// Repository contract for the `RoleBinding` aggregate.
#[async_trait]
pub trait RoleBindingRepository: Send + Sync {
    /// Find a role binding by id, including its resource set members.
    async fn by_id(
        &self,
        id: BindingId,
        ctx: &RequestContext,
    ) -> Result<RoleBinding, PlatformError>;

    /// Persist a new role binding.
    async fn create(
        &self,
        binding: &RoleBinding,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Update an existing role binding and its resource set.
    async fn update(
        &self,
        binding: &RoleBinding,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Soft-delete a role binding.
    async fn delete(
        &self,
        id: BindingId,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// List active bindings for a principal in the current tenant context.
    async fn list_by_principal(
        &self,
        principal_id: UserId,
        ctx: &RequestContext,
    ) -> Result<Page<RoleBinding>, PlatformError>;
}

/// Repository contract for the `ManagedDevice` aggregate.
#[async_trait]
pub trait DeviceRepository: Send + Sync {
    async fn by_id(
        &self,
        id: DeviceId,
        ctx: &RequestContext,
    ) -> Result<ManagedDevice, PlatformError>;

    async fn create(
        &self,
        device: &ManagedDevice,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    async fn update(
        &self,
        device: &ManagedDevice,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    async fn delete(
        &self,
        id: DeviceId,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    async fn list(&self, ctx: &RequestContext) -> Result<Page<ManagedDevice>, PlatformError>;
}

/// Repository contract for the `Camera` aggregate.
#[async_trait]
pub trait CameraRepository: Send + Sync {
    async fn by_id(&self, id: CameraId, ctx: &RequestContext) -> Result<Camera, PlatformError>;

    async fn create(&self, camera: &Camera, ctx: &RequestContext) -> Result<(), PlatformError>;

    async fn update(
        &self,
        camera: &Camera,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    async fn delete(
        &self,
        id: CameraId,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    async fn list_by_device(
        &self,
        device_id: DeviceId,
        ctx: &RequestContext,
    ) -> Result<Page<Camera>, PlatformError>;
}

/// Repository contract for the `Tag` aggregate.
#[async_trait]
pub trait TagRepository: Send + Sync {
    async fn by_id(&self, id: TagId, ctx: &RequestContext) -> Result<Tag, PlatformError>;

    async fn create(&self, tag: &Tag, ctx: &RequestContext) -> Result<(), PlatformError>;

    async fn update(
        &self,
        tag: &Tag,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    async fn delete(
        &self,
        id: TagId,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    async fn list_by_resource(
        &self,
        resource_type: ResourceType,
        resource_id: Uuid,
        ctx: &RequestContext,
    ) -> Result<Page<Tag>, PlatformError>;
}

/// Repository contract for the `ExternalBinding` aggregate.
#[async_trait]
pub trait ExternalBindingRepository: Send + Sync {
    async fn by_id(
        &self,
        id: ExternalBindingId,
        ctx: &RequestContext,
    ) -> Result<ExternalBinding, PlatformError>;

    async fn create(
        &self,
        binding: &ExternalBinding,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    async fn update(
        &self,
        binding: &ExternalBinding,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Activate a pending binding, or mark it conflict when another active binding
    /// already owns the same external reference.
    async fn activate(
        &self,
        id: ExternalBindingId,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<ExternalBinding, PlatformError>;

    /// Disable a binding without deleting it.
    async fn disable(
        &self,
        id: ExternalBindingId,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    async fn list_by_resource(
        &self,
        resource_type: ResourceType,
        resource_id: Uuid,
        ctx: &RequestContext,
    ) -> Result<Page<ExternalBinding>, PlatformError>;

    async fn list_by_external_ref(
        &self,
        external_kind: &str,
        external_ref: &str,
        ctx: &RequestContext,
    ) -> Result<Page<ExternalBinding>, PlatformError>;
}

/// Repository contract for signaling projection read models.
#[async_trait]
pub trait ProjectionRepository: Send + Sync {
    /// Apply a device event, handling duplicates, out-of-order, and mismatched payloads.
    async fn apply_device_event(
        &self,
        event: DeviceEvent,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Get the current device projection from the active read view.
    async fn get_device(
        &self,
        external_ref: &str,
        ctx: &RequestContext,
    ) -> Result<DeviceProjection, PlatformError>;

    /// Apply a channel event.
    async fn apply_channel_event(
        &self,
        event: ChannelEvent,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Get the current channel projection from the active read view.
    async fn get_channel(
        &self,
        external_ref: &str,
        ctx: &RequestContext,
    ) -> Result<ChannelProjection, PlatformError>;

    /// Rebuild the shadow tables from a complete ordered event stream and atomically
    /// switch the read view to the rebuilt shadow.
    async fn rebuild_shadow(
        &self,
        device_events: Vec<DeviceEvent>,
        channel_events: Vec<ChannelEvent>,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Persist a worker checkpoint.
    async fn checkpoint(
        &self,
        worker_id: &str,
        last_event_id: &str,
        observed_at: UtcTimestamp,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;

    /// Record a projection processing failure for later review.
    async fn record_failure(
        &self,
        failure: ProjectionFailure,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError>;
}

/// Append-only audit writer port. Implementations must guarantee that written
/// records cannot be updated or deleted by the application role.
#[async_trait]
pub trait AuditWriter: Send + Sync {
    /// Persist an audit record and return the generated database identifier.
    async fn write(
        &self,
        record: &AuditRecord,
        ctx: &RequestContext,
    ) -> Result<AuditRecordId, PlatformError>;
}

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[allow(dead_code)]
fn touch_dependencies() {
    let _v_foundation = foundation::version();
    let _v_domain_identity = domain_identity::version();
}
