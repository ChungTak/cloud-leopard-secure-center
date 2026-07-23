use domain_organization::organization_unit::OrganizationUnit;
use domain_organization::spatial::Area;
use domain_organization::tenant::Tenant;
use domain_resource::camera::{Camera, Sensitivity};
use domain_resource::device::{DeviceLifecycle, ManagedDevice};
use foundation::{
    AreaId, CameraId, DeviceId, OrganizationId, RequestContext, SystemClock, TenantId, uuid::Uuid,
};
use storage_api::{
    CameraRepository, DeviceRepository, ListOptions, OrganizationUnitRepository, SpatialRepository,
    TenantRepository,
};
use storage_postgres::camera_repository::PostgresCameraRepository;
use storage_postgres::device_repository::PostgresDeviceRepository;
use storage_postgres::organization_unit_repository::PostgresOrganizationUnitRepository;
use storage_postgres::spatial_repository::PostgresSpatialRepository;
use storage_postgres::tenant_repository::PostgresTenantRepository;

fn parse_uuid(s: &str) -> Uuid {
    Uuid::parse_str(s).unwrap_or_else(|e| panic!("{e}"))
}

fn parse_tenant(s: &str) -> TenantId {
    TenantId::parse_str(s).unwrap_or_else(|e| panic!("{e}"))
}

fn parse_device(s: &str) -> DeviceId {
    DeviceId::parse_str(s).unwrap_or_else(|e| panic!("{e}"))
}

fn parse_camera(s: &str) -> CameraId {
    CameraId::parse_str(s).unwrap_or_else(|e| panic!("{e}"))
}

fn parse_area(s: &str) -> AreaId {
    AreaId::parse_str(s).unwrap_or_else(|e| panic!("{e}"))
}

fn parse_organization(s: &str) -> OrganizationId {
    OrganizationId::parse_str(s).unwrap_or_else(|e| panic!("{e}"))
}

fn ok_or_panic<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
    match result {
        Ok(v) => v,
        Err(e) => panic!("{e}"),
    }
}

fn tenant_ctx(tenant: &str) -> RequestContext {
    RequestContext {
        tenant_id: Some(parse_tenant(tenant)),
        ..Default::default()
    }
}

async fn create_tenant(pool: &sqlx::PgPool, id: Uuid, code: &str, name: &str) -> TenantId {
    let repo = PostgresTenantRepository::new(pool.clone());
    let tenant = ok_or_panic(Tenant::new(
        parse_tenant(&id.to_string()),
        code,
        name,
        Option::<&str>::None,
        Option::<&str>::None,
        &SystemClock,
        None,
    ));
    ok_or_panic(repo.create(&tenant, &tenant_ctx(&id.to_string())).await);
    parse_tenant(&id.to_string())
}

async fn create_organization(
    pool: &sqlx::PgPool,
    tenant: TenantId,
    id: Uuid,
    code: &str,
) -> OrganizationId {
    let repo = PostgresOrganizationUnitRepository::new(pool.clone());
    let ctx = tenant_ctx(&tenant.as_uuid().to_string());
    let unit = ok_or_panic(OrganizationUnit::new(
        parse_organization(&id.to_string()),
        tenant,
        None,
        code,
        "Org",
        &SystemClock,
        None,
    ));
    ok_or_panic(repo.create(&unit, &ctx).await);
    parse_organization(&id.to_string())
}

async fn create_area(pool: &sqlx::PgPool, tenant: TenantId, id: Uuid, code: &str) -> AreaId {
    let repo = PostgresSpatialRepository::new(pool.clone());
    let ctx = tenant_ctx(&tenant.as_uuid().to_string());
    let area = ok_or_panic(Area::new(
        parse_area(&id.to_string()),
        tenant,
        None,
        None,
        code,
        "Area",
        &SystemClock,
        None,
    ));
    ok_or_panic(repo.create_area(&area, &ctx).await);
    parse_area(&id.to_string())
}

#[sqlx::test(migrations = "../../migrations")]
async fn device_lifecycle_and_revision(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant = create_tenant(
        &pool,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "t1",
        "Tenant",
    )
    .await;
    let repo = PostgresDeviceRepository::new(pool);
    let ctx = tenant_ctx(&tenant.as_uuid().to_string());

    let mut device = ok_or_panic(ManagedDevice::new(
        parse_device("018e0000-0000-0000-0000-000000000001"),
        tenant,
        "dev-01",
        "Device 1",
        Some("SN123".to_string()),
        &SystemClock,
        None,
    ));

    ok_or_panic(repo.create(&device, &ctx).await);
    assert_eq!(device.lifecycle, DeviceLifecycle::Draft);

    ok_or_panic(device.activate(&SystemClock, None));
    ok_or_panic(repo.update(&device, device.revision.prev(), &ctx).await);

    let read = ok_or_panic(repo.by_id(device.id, &ctx).await);
    assert_eq!(read.lifecycle, DeviceLifecycle::Active);
    assert_eq!(read.serial, Some("SN123".to_string()));

    ok_or_panic(device.retire(&SystemClock, None));
    ok_or_panic(repo.update(&device, device.revision.prev(), &ctx).await);

    let read = ok_or_panic(repo.by_id(device.id, &ctx).await);
    assert_eq!(read.lifecycle, DeviceLifecycle::Retired);
    assert!(read.lifecycle.is_terminal());

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn camera_references_and_sensitivity(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant = create_tenant(
        &pool,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "t1",
        "Tenant",
    )
    .await;
    let org = create_organization(
        &pool,
        tenant,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ac"),
        "org-1",
    )
    .await;
    let area = create_area(
        &pool,
        tenant,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ad"),
        "area-1",
    )
    .await;

    let device_repo = PostgresDeviceRepository::new(pool.clone());
    let ctx = tenant_ctx(&tenant.as_uuid().to_string());

    let mut device = ok_or_panic(ManagedDevice::new(
        parse_device("018e0000-0000-0000-0000-000000000001"),
        tenant,
        "dev-01",
        "Device 1",
        None,
        &SystemClock,
        None,
    ));
    device.set_location(Some(org), Some(area), &SystemClock, None);
    ok_or_panic(device.activate(&SystemClock, None));
    ok_or_panic(device_repo.create(&device, &ctx).await);

    let camera_repo = PostgresCameraRepository::new(pool.clone());
    let mut camera = ok_or_panic(Camera::new(
        parse_camera("018e0000-0000-0000-0000-000000000002"),
        tenant,
        device.id,
        "cam-01",
        "Camera 1",
        Sensitivity::Medium,
        &SystemClock,
        None,
    ));
    camera.set_area(Some(area), &SystemClock, None);
    ok_or_panic(camera_repo.create(&camera, &ctx).await);

    let read = ok_or_panic(camera_repo.by_id(camera.id, &ctx).await);
    assert_eq!(read.sensitivity, Sensitivity::Medium);
    assert_eq!(read.area_id, Some(area));

    camera.set_sensitivity(Sensitivity::Critical, &SystemClock, None);
    ok_or_panic(camera_repo.update(&camera, read.revision, &ctx).await);

    let read = ok_or_panic(camera_repo.by_id(camera.id, &ctx).await);
    assert_eq!(read.sensitivity, Sensitivity::Critical);

    let cameras = ok_or_panic(
        camera_repo
            .list_by_device(device.id, &ctx, ListOptions::default())
            .await,
    );
    assert_eq!(cameras.items.len(), 1);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn retired_device_is_persisted_not_deleted(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant = create_tenant(
        &pool,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "t1",
        "Tenant",
    )
    .await;
    let repo = PostgresDeviceRepository::new(pool);
    let ctx = tenant_ctx(&tenant.as_uuid().to_string());

    let mut device = ok_or_panic(ManagedDevice::new(
        parse_device("018e0000-0000-0000-0000-000000000001"),
        tenant,
        "dev-01",
        "Device 1",
        None,
        &SystemClock,
        None,
    ));
    ok_or_panic(device.activate(&SystemClock, None));
    ok_or_panic(repo.create(&device, &ctx).await);

    ok_or_panic(device.retire(&SystemClock, None));
    ok_or_panic(repo.update(&device, device.revision.prev(), &ctx).await);

    let read = ok_or_panic(repo.by_id(device.id, &ctx).await);
    assert_eq!(read.lifecycle, DeviceLifecycle::Retired);

    let list = ok_or_panic(repo.list(&ctx, ListOptions::default()).await);
    assert!(list.items.iter().any(|d| d.id == device.id));

    Ok(())
}
