use domain_authorization::permission::Permission;
use domain_authorization::role::Role;
use domain_authorization::role_binding::{ResourceRef, RoleBinding, Scope};
use domain_organization::tenant::Tenant;
use foundation::{
    BindingId, Clock, FakeClock, OrganizationId, RequestContext, Revision, RoleId, SiteId,
    SystemClock, TenantId, UserId, uuid::Uuid,
};
use storage_api::{ListOptions, RoleBindingRepository, RoleRepository, TenantRepository};
use storage_postgres::role_binding_repository::PostgresRoleBindingRepository;
use storage_postgres::role_repository::PostgresRoleRepository;
use storage_postgres::tenant_repository::PostgresTenantRepository;

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

fn parse_binding(s: &str) -> BindingId {
    match BindingId::parse_str(s) {
        Ok(v) => v,
        Err(e) => panic!("{e}"),
    }
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

async fn create_tenant(
    repo: &PostgresTenantRepository,
    id: Uuid,
    code: &str,
    name: &str,
) -> Tenant {
    let tenant = ok_or_panic(Tenant::new(
        parse_tenant(&id.to_string()),
        code,
        name,
        Option::<&str>::None,
        Option::<&str>::None,
        &SystemClock,
        None,
    ));
    let ctx = tenant_ctx(&id.to_string());
    ok_or_panic(repo.create(&tenant, &ctx).await);
    tenant
}

async fn create_role(
    repo: &PostgresRoleRepository,
    tenant: &Tenant,
    id: Uuid,
    name: &str,
    perms: Vec<Permission>,
) -> Role {
    let role = ok_or_panic(Role::new(
        parse_role(&id.to_string()),
        Some(tenant.id),
        name,
        false,
        perms,
        &SystemClock,
        None,
    ));
    let ctx = tenant_ctx(&tenant.id.to_string());
    ok_or_panic(repo.create(&role, &ctx).await);
    role
}

fn new_binding(
    tenant: &Tenant,
    role: &Role,
    id: Uuid,
    scope: Scope,
    clock: &FakeClock,
) -> RoleBinding {
    ok_or_panic(RoleBinding::new(
        parse_binding(&id.to_string()),
        tenant.id,
        parse_user("018e1234-5678-7abc-8def-0123456789ac"),
        role.id,
        scope,
        clock.now(),
        None,
        clock,
        None,
    ))
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_and_read_tenant_scope_binding(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_repo = PostgresTenantRepository::new(pool.clone());
    let role_repo = PostgresRoleRepository::new(pool.clone());
    let repo = PostgresRoleBindingRepository::new(pool.clone());

    let tenant = create_tenant(
        &tenant_repo,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "acme",
        "Acme",
    )
    .await;
    let role = create_role(
        &role_repo,
        &tenant,
        parse_uuid("018e0000-0000-0000-0000-000000000001"),
        "Admin",
        vec![],
    )
    .await;
    let ctx = tenant_ctx("018e1234-5678-7abc-8def-0123456789ab");

    let clock = FakeClock::from_millis(1_000_000_000_000);
    let binding = new_binding(
        &tenant,
        &role,
        parse_uuid("018e0000-0000-0000-0000-000000000002"),
        Scope::Tenant,
        &clock,
    );
    ok_or_panic(repo.create(&binding, &ctx).await);

    let read = ok_or_panic(repo.by_id(binding.id, &ctx).await);
    assert_eq!(read.scope, Scope::Tenant);
    assert_eq!(read.role_id, role.id);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn resource_set_scope_round_trip(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_repo = PostgresTenantRepository::new(pool.clone());
    let role_repo = PostgresRoleRepository::new(pool.clone());
    let repo = PostgresRoleBindingRepository::new(pool.clone());

    let tenant = create_tenant(
        &tenant_repo,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "acme",
        "Acme",
    )
    .await;
    let role = create_role(
        &role_repo,
        &tenant,
        parse_uuid("018e0000-0000-0000-0000-000000000001"),
        "Admin",
        vec![],
    )
    .await;
    let ctx = tenant_ctx("018e1234-5678-7abc-8def-0123456789ab");

    let clock = FakeClock::from_millis(1_000_000_000_000);
    let org_id = OrganizationId::parse_str("018e0000-0000-0000-0000-000000000003")
        .unwrap_or_else(|e| panic!("{e}"));
    let site_id =
        SiteId::parse_str("018e0000-0000-0000-0000-000000000004").unwrap_or_else(|e| panic!("{e}"));
    let scope = Scope::ResourceSet(vec![
        ResourceRef::Organization(org_id),
        ResourceRef::Site(site_id),
    ]);
    let binding = new_binding(
        &tenant,
        &role,
        parse_uuid("018e0000-0000-0000-0000-000000000002"),
        scope,
        &clock,
    );
    ok_or_panic(repo.create(&binding, &ctx).await);

    let read = ok_or_panic(repo.by_id(binding.id, &ctx).await);
    if let Scope::ResourceSet(resources) = read.scope {
        assert_eq!(resources.len(), 2);
    } else {
        panic!("expected resource set scope");
    }

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn list_bindings_by_principal(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_repo = PostgresTenantRepository::new(pool.clone());
    let role_repo = PostgresRoleRepository::new(pool.clone());
    let repo = PostgresRoleBindingRepository::new(pool.clone());

    let tenant = create_tenant(
        &tenant_repo,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "acme",
        "Acme",
    )
    .await;
    let role = create_role(
        &role_repo,
        &tenant,
        parse_uuid("018e0000-0000-0000-0000-000000000001"),
        "Admin",
        vec![],
    )
    .await;
    let ctx = tenant_ctx("018e1234-5678-7abc-8def-0123456789ab");

    let clock = FakeClock::from_millis(1_000_000_000_000);
    let principal = parse_user("018e1234-5678-7abc-8def-0123456789ac");
    ok_or_panic(
        repo.create(
            &new_binding(
                &tenant,
                &role,
                parse_uuid("018e0000-0000-0000-0000-000000000002"),
                Scope::Tenant,
                &clock,
            ),
            &ctx,
        )
        .await,
    );
    ok_or_panic(
        repo.create(
            &new_binding(
                &tenant,
                &role,
                parse_uuid("018e0000-0000-0000-0000-000000000003"),
                Scope::Tenant,
                &clock,
            ),
            &ctx,
        )
        .await,
    );

    let page = ok_or_panic(
        repo.list_by_principal(principal, &ctx, ListOptions::default())
            .await,
    );
    assert_eq!(page.items.len(), 2);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn update_scope_bumps_revision(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_repo = PostgresTenantRepository::new(pool.clone());
    let role_repo = PostgresRoleRepository::new(pool.clone());
    let repo = PostgresRoleBindingRepository::new(pool.clone());

    let tenant = create_tenant(
        &tenant_repo,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "acme",
        "Acme",
    )
    .await;
    let role = create_role(
        &role_repo,
        &tenant,
        parse_uuid("018e0000-0000-0000-0000-000000000001"),
        "Admin",
        vec![],
    )
    .await;
    let ctx = tenant_ctx("018e1234-5678-7abc-8def-0123456789ab");

    let clock = FakeClock::from_millis(1_000_000_000_000);
    let binding = new_binding(
        &tenant,
        &role,
        parse_uuid("018e0000-0000-0000-0000-000000000002"),
        Scope::Tenant,
        &clock,
    );
    ok_or_panic(repo.create(&binding, &ctx).await);

    let mut read = ok_or_panic(repo.by_id(binding.id, &ctx).await);
    let org_id = OrganizationId::parse_str("018e0000-0000-0000-0000-000000000003")
        .unwrap_or_else(|e| panic!("{e}"));
    ok_or_panic(read.set_scope(Scope::OrganizationSubtree(org_id), &clock, None));
    ok_or_panic(repo.update(&read, Revision::initial(), &ctx).await);

    let after = ok_or_panic(repo.by_id(binding.id, &ctx).await);
    assert_eq!(after.revision.value(), 2);
    assert_eq!(after.scope, Scope::OrganizationSubtree(org_id));

    Ok(())
}
