use application::authorization::{
    AuthorizationPort, AuthorizationRequest, AuthorizationService, Decision, Reason,
};
use domain_authorization::permission::Permission;
use domain_authorization::role::Role;
use domain_authorization::role_binding::{ResourceRef, RoleBinding, Scope};
use domain_organization::organization_unit::OrganizationUnit;
use domain_organization::spatial::Area;
use domain_organization::tenant::Tenant;
use foundation::{
    AreaId, BindingId, Clock, FakeClock, OrganizationId, RequestContext, RoleId, TenantId, UserId,
    UtcTimestamp, uuid::Uuid,
};
use storage_api::{
    OrganizationUnitRepository, RoleBindingRepository, RoleRepository, SpatialRepository,
    TenantRepository,
};
use storage_postgres::organization_unit_repository::PostgresOrganizationUnitRepository;
use storage_postgres::role_binding_repository::PostgresRoleBindingRepository;
use storage_postgres::role_repository::PostgresRoleRepository;
use storage_postgres::spatial_repository::PostgresSpatialRepository;
use storage_postgres::tenant_repository::PostgresTenantRepository;

fn ok_or_panic<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
    match result {
        Ok(v) => v,
        Err(e) => panic!("{e}"),
    }
}

fn parse_uuid(s: &str) -> Uuid {
    match Uuid::parse_str(s) {
        Ok(v) => v,
        Err(e) => panic!("{e}"),
    }
}

fn parse_tenant(s: &str) -> TenantId {
    match TenantId::parse_str(s) {
        Ok(v) => v,
        Err(e) => panic!("{e}"),
    }
}

fn parse_user(s: &str) -> UserId {
    match UserId::parse_str(s) {
        Ok(v) => v,
        Err(e) => panic!("{e}"),
    }
}

fn parse_role(s: &str) -> RoleId {
    match RoleId::parse_str(s) {
        Ok(v) => v,
        Err(e) => panic!("{e}"),
    }
}

fn parse_org(s: &str) -> OrganizationId {
    match OrganizationId::parse_str(s) {
        Ok(v) => v,
        Err(e) => panic!("{e}"),
    }
}

fn parse_area(s: &str) -> AreaId {
    match AreaId::parse_str(s) {
        Ok(v) => v,
        Err(e) => panic!("{e}"),
    }
}

fn parse_binding(s: &str) -> BindingId {
    match BindingId::parse_str(s) {
        Ok(v) => v,
        Err(e) => panic!("{e}"),
    }
}

fn parse_permission(s: &str) -> Permission {
    match Permission::parse(s) {
        Ok(v) => v,
        Err(e) => panic!("{e}"),
    }
}

fn ctx(tenant: TenantId) -> RequestContext {
    RequestContext {
        tenant_id: Some(tenant),
        ..Default::default()
    }
}

async fn create_tenant(pool: &sqlx::PgPool, id: Uuid) -> TenantId {
    let repo = PostgresTenantRepository::new(pool.clone());
    let tenant_id = parse_tenant(&id.to_string());
    let tenant = ok_or_panic(Tenant::new(
        tenant_id,
        "acme",
        "Acme",
        Option::<&str>::None,
        Option::<&str>::None,
        &foundation::SystemClock,
        None,
    ));
    ok_or_panic(repo.create(&tenant, &ctx(tenant_id)).await);
    tenant_id
}

async fn create_org(
    pool: &sqlx::PgPool,
    tenant: TenantId,
    id: Uuid,
    parent: Option<OrganizationId>,
    code: &str,
) -> OrganizationId {
    let repo = PostgresOrganizationUnitRepository::new(pool.clone());
    let org_id = parse_org(&id.to_string());
    let unit = ok_or_panic(OrganizationUnit::new(
        org_id,
        tenant,
        parent,
        code,
        code,
        &FakeClock::from_millis(1_000_000_000_000),
        None,
    ));
    ok_or_panic(repo.create(&unit, &ctx(tenant)).await);
    org_id
}

async fn create_area(
    pool: &sqlx::PgPool,
    tenant: TenantId,
    id: Uuid,
    parent: Option<AreaId>,
    code: &str,
) -> AreaId {
    let repo = PostgresSpatialRepository::new(pool.clone());
    let area_id = parse_area(&id.to_string());
    let mut area = ok_or_panic(Area::new(
        area_id,
        tenant,
        None,
        None,
        code,
        code,
        &FakeClock::from_millis(1_000_000_000_000),
        None,
    ));
    if let Some(parent_id) = parent {
        area.set_parent(
            Some(parent_id),
            &[],
            &FakeClock::from_millis(1_000_000_000_000),
            None,
        )
        .unwrap_or_else(|e| panic!("{e}"));
    }
    ok_or_panic(repo.create_area(&area, &ctx(tenant)).await);
    area_id
}

async fn create_role(
    pool: &sqlx::PgPool,
    tenant: TenantId,
    id: Uuid,
    permissions: Vec<Permission>,
) -> RoleId {
    let repo = PostgresRoleRepository::new(pool.clone());
    let role_id = parse_role(&id.to_string());
    let role = ok_or_panic(Role::new(
        role_id,
        Some(tenant),
        "test-role",
        false,
        permissions,
        &FakeClock::from_millis(1_000_000_000_000),
        None,
    ));
    ok_or_panic(repo.create(&role, &ctx(tenant)).await);
    role_id
}

async fn create_binding(
    pool: &sqlx::PgPool,
    tenant: TenantId,
    principal: UserId,
    role: RoleId,
    id: Uuid,
    scope: Scope,
    clock: &FakeClock,
) -> BindingId {
    let repo = PostgresRoleBindingRepository::new(pool.clone());
    let binding_id = parse_binding(&id.to_string());
    let binding = ok_or_panic(RoleBinding::new(
        binding_id,
        tenant,
        principal,
        role,
        scope,
        clock.now(),
        None,
        clock,
        None,
    ));
    ok_or_panic(repo.create(&binding, &ctx(tenant)).await);
    binding_id
}

fn service(
    pool: &sqlx::PgPool,
    clock: FakeClock,
) -> AuthorizationService<
    PostgresRoleRepository,
    PostgresRoleBindingRepository,
    PostgresOrganizationUnitRepository,
    PostgresSpatialRepository,
    FakeClock,
> {
    AuthorizationService::new(
        PostgresRoleRepository::new(pool.clone()),
        PostgresRoleBindingRepository::new(pool.clone()),
        PostgresOrganizationUnitRepository::new(pool.clone()),
        PostgresSpatialRepository::new(pool.clone()),
        clock,
    )
}

#[sqlx::test(migrations = "../../migrations")]
async fn no_binding_denies(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant = create_tenant(&pool, parse_uuid("018e1234-5678-7abc-8def-0123456789ab")).await;
    let principal = parse_user("018e1234-5678-7abc-8def-0123456789ac");
    let svc = service(&pool, FakeClock::from_millis(1_000_000_000_000));
    let res = ok_or_panic(
        svc.authorize(
            AuthorizationRequest {
                principal,
                tenant,
                action: "tenant:user:read".to_string(),
                resource: ResourceRef::User(principal),
                context: None,
            },
            &ctx(tenant),
        )
        .await,
    );
    assert_eq!(res.decision, Decision::Deny);
    assert_eq!(res.reason, Reason::NoBinding);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn unknown_action_denies(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant = create_tenant(&pool, parse_uuid("018e1234-5678-7abc-8def-0123456789ab")).await;
    let principal = parse_user("018e1234-5678-7abc-8def-0123456789ac");
    let svc = service(&pool, FakeClock::from_millis(1_000_000_000_000));
    let res = ok_or_panic(
        svc.authorize(
            AuthorizationRequest {
                principal,
                tenant,
                action: "tenant:unknown:action".to_string(),
                resource: ResourceRef::User(principal),
                context: None,
            },
            &ctx(tenant),
        )
        .await,
    );
    assert_eq!(res.decision, Decision::Deny);
    assert_eq!(res.reason, Reason::PermissionNotGranted);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn tenant_scope_allows(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant = create_tenant(&pool, parse_uuid("018e1234-5678-7abc-8def-0123456789ab")).await;
    let principal = parse_user("018e1234-5678-7abc-8def-0123456789ac");
    let role = create_role(
        &pool,
        tenant,
        parse_uuid("018e0000-0000-0000-0000-000000000001"),
        vec![parse_permission("tenant:user:read")],
    )
    .await;
    let _binding = create_binding(
        &pool,
        tenant,
        principal,
        role,
        parse_uuid("018e0000-0000-0000-0000-000000000002"),
        Scope::Tenant,
        &FakeClock::from_millis(1_000_000_000_000),
    )
    .await;

    let svc = service(&pool, FakeClock::from_millis(1_000_000_000_000));
    let res = ok_or_panic(
        svc.authorize(
            AuthorizationRequest {
                principal,
                tenant,
                action: "tenant:user:read".to_string(),
                resource: ResourceRef::User(principal),
                context: None,
            },
            &ctx(tenant),
        )
        .await,
    );
    assert_eq!(res.decision, Decision::Allow);
    assert_eq!(res.reason, Reason::Allowed);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn organization_subtree_allows(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant = create_tenant(&pool, parse_uuid("018e1234-5678-7abc-8def-0123456789ab")).await;
    let principal = parse_user("018e1234-5678-7abc-8def-0123456789ac");
    let root = create_org(
        &pool,
        tenant,
        parse_uuid("018e0000-0000-0000-0000-000000000001"),
        None,
        "root",
    )
    .await;
    let child = create_org(
        &pool,
        tenant,
        parse_uuid("018e0000-0000-0000-0000-000000000002"),
        Some(root),
        "child",
    )
    .await;
    let role = create_role(
        &pool,
        tenant,
        parse_uuid("018e0000-0000-0000-0000-000000000003"),
        vec![parse_permission("tenant:organization:read")],
    )
    .await;
    let _binding = create_binding(
        &pool,
        tenant,
        principal,
        role,
        parse_uuid("018e0000-0000-0000-0000-000000000004"),
        Scope::OrganizationSubtree(root),
        &FakeClock::from_millis(1_000_000_000_000),
    )
    .await;

    let svc = service(&pool, FakeClock::from_millis(1_000_000_000_000));
    let res = ok_or_panic(
        svc.authorize(
            AuthorizationRequest {
                principal,
                tenant,
                action: "tenant:organization:read".to_string(),
                resource: ResourceRef::Organization(child),
                context: None,
            },
            &ctx(tenant),
        )
        .await,
    );
    assert_eq!(res.decision, Decision::Allow);
    assert_eq!(res.reason, Reason::Allowed);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn organization_subtree_excludes_other_branch(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant = create_tenant(&pool, parse_uuid("018e1234-5678-7abc-8def-0123456789ab")).await;
    let principal = parse_user("018e1234-5678-7abc-8def-0123456789ac");
    let root = create_org(
        &pool,
        tenant,
        parse_uuid("018e0000-0000-0000-0000-000000000001"),
        None,
        "root",
    )
    .await;
    let other = create_org(
        &pool,
        tenant,
        parse_uuid("018e0000-0000-0000-0000-000000000002"),
        None,
        "other",
    )
    .await;
    let role = create_role(
        &pool,
        tenant,
        parse_uuid("018e0000-0000-0000-0000-000000000003"),
        vec![parse_permission("tenant:organization:read")],
    )
    .await;
    let _binding = create_binding(
        &pool,
        tenant,
        principal,
        role,
        parse_uuid("018e0000-0000-0000-0000-000000000004"),
        Scope::OrganizationSubtree(root),
        &FakeClock::from_millis(1_000_000_000_000),
    )
    .await;

    let svc = service(&pool, FakeClock::from_millis(1_000_000_000_000));
    let res = ok_or_panic(
        svc.authorize(
            AuthorizationRequest {
                principal,
                tenant,
                action: "tenant:organization:read".to_string(),
                resource: ResourceRef::Organization(other),
                context: None,
            },
            &ctx(tenant),
        )
        .await,
    );
    assert_eq!(res.decision, Decision::Deny);
    assert_eq!(res.reason, Reason::ScopeMismatch);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn area_subtree_allows(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant = create_tenant(&pool, parse_uuid("018e1234-5678-7abc-8def-0123456789ab")).await;
    let principal = parse_user("018e1234-5678-7abc-8def-0123456789ac");
    let root = create_area(
        &pool,
        tenant,
        parse_uuid("018e0000-0000-0000-0000-000000000001"),
        None,
        "root-area",
    )
    .await;
    let child = create_area(
        &pool,
        tenant,
        parse_uuid("018e0000-0000-0000-0000-000000000002"),
        Some(root),
        "child-area",
    )
    .await;
    let role = create_role(
        &pool,
        tenant,
        parse_uuid("018e0000-0000-0000-0000-000000000003"),
        vec![parse_permission("tenant:area:read")],
    )
    .await;
    let _binding = create_binding(
        &pool,
        tenant,
        principal,
        role,
        parse_uuid("018e0000-0000-0000-0000-000000000004"),
        Scope::AreaSubtree(root),
        &FakeClock::from_millis(1_000_000_000_000),
    )
    .await;

    let svc = service(&pool, FakeClock::from_millis(1_000_000_000_000));
    let res = ok_or_panic(
        svc.authorize(
            AuthorizationRequest {
                principal,
                tenant,
                action: "tenant:area:read".to_string(),
                resource: ResourceRef::Area(child),
                context: None,
            },
            &ctx(tenant),
        )
        .await,
    );
    assert_eq!(res.decision, Decision::Allow);
    assert_eq!(res.reason, Reason::Allowed);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn resource_set_allows(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant = create_tenant(&pool, parse_uuid("018e1234-5678-7abc-8def-0123456789ab")).await;
    let principal = parse_user("018e1234-5678-7abc-8def-0123456789ac");
    let target = parse_user("018e0000-0000-0000-0000-000000000005");
    let role = create_role(
        &pool,
        tenant,
        parse_uuid("018e0000-0000-0000-0000-000000000001"),
        vec![parse_permission("tenant:user:read")],
    )
    .await;
    let _binding = create_binding(
        &pool,
        tenant,
        principal,
        role,
        parse_uuid("018e0000-0000-0000-0000-000000000002"),
        Scope::ResourceSet(vec![ResourceRef::User(target)]),
        &FakeClock::from_millis(1_000_000_000_000),
    )
    .await;

    let svc = service(&pool, FakeClock::from_millis(1_000_000_000_000));
    let res = ok_or_panic(
        svc.authorize(
            AuthorizationRequest {
                principal,
                tenant,
                action: "tenant:user:read".to_string(),
                resource: ResourceRef::User(target),
                context: None,
            },
            &ctx(tenant),
        )
        .await,
    );
    assert_eq!(res.decision, Decision::Allow);
    assert_eq!(res.reason, Reason::Allowed);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn expired_binding_denies(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant = create_tenant(&pool, parse_uuid("018e1234-5678-7abc-8def-0123456789ab")).await;
    let principal = parse_user("018e1234-5678-7abc-8def-0123456789ac");
    let role = create_role(
        &pool,
        tenant,
        parse_uuid("018e0000-0000-0000-0000-000000000001"),
        vec![parse_permission("tenant:user:read")],
    )
    .await;

    let clock = FakeClock::from_millis(1_000_000_000_000);
    let binding_id = parse_binding("018e0000-0000-0000-0000-000000000002");
    let valid_until =
        UtcTimestamp::parse_rfc3339("2001-09-09T01:46:41Z").unwrap_or_else(|e| panic!("{e}"));
    let binding = ok_or_panic(RoleBinding::new(
        binding_id,
        tenant,
        principal,
        role,
        Scope::Tenant,
        clock.now(),
        Some(valid_until),
        &clock,
        None,
    ));
    ok_or_panic(
        PostgresRoleBindingRepository::new(pool.clone())
            .create(&binding, &ctx(tenant))
            .await,
    );

    let later = FakeClock::from_millis(1_000_000_000_000_000);
    let svc = service(&pool, later);
    let res = ok_or_panic(
        svc.authorize(
            AuthorizationRequest {
                principal,
                tenant,
                action: "tenant:user:read".to_string(),
                resource: ResourceRef::User(principal),
                context: None,
            },
            &ctx(tenant),
        )
        .await,
    );
    assert_eq!(res.decision, Decision::Deny);
    assert_eq!(res.reason, Reason::Expired);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn missing_permission_denies(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant = create_tenant(&pool, parse_uuid("018e1234-5678-7abc-8def-0123456789ab")).await;
    let principal = parse_user("018e1234-5678-7abc-8def-0123456789ac");
    let role = create_role(
        &pool,
        tenant,
        parse_uuid("018e0000-0000-0000-0000-000000000001"),
        vec![parse_permission("tenant:user:read")],
    )
    .await;
    let _binding = create_binding(
        &pool,
        tenant,
        principal,
        role,
        parse_uuid("018e0000-0000-0000-0000-000000000002"),
        Scope::Tenant,
        &FakeClock::from_millis(1_000_000_000_000),
    )
    .await;

    let svc = service(&pool, FakeClock::from_millis(1_000_000_000_000));
    let res = ok_or_panic(
        svc.authorize(
            AuthorizationRequest {
                principal,
                tenant,
                action: "tenant:site:read".to_string(),
                resource: ResourceRef::User(principal),
                context: None,
            },
            &ctx(tenant),
        )
        .await,
    );
    assert_eq!(res.decision, Decision::Deny);
    assert_eq!(res.reason, Reason::PermissionNotGranted);

    Ok(())
}
