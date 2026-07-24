use domain_authorization::permission::Permission;
use domain_authorization::role::Role;
use domain_organization::tenant::Tenant;
use foundation::{Clock, RequestContext, Revision, RoleId, SystemClock, TenantId, uuid::Uuid};
use storage_api::{ListOptions, RoleRepository, TenantRepository};
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

fn parse_role(s: &str) -> RoleId {
    match RoleId::parse_str(s) {
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

fn new_role(tenant: &Tenant, id: Uuid, name: &str, perms: Vec<Permission>) -> Role {
    ok_or_panic(Role::new(
        parse_role(&id.to_string()),
        Some(tenant.id),
        name,
        false,
        perms,
        &SystemClock,
        None,
    ))
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_and_read_role_with_permissions(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_repo = PostgresTenantRepository::new(pool.clone());
    let repo = PostgresRoleRepository::new(pool.clone());
    let tenant = create_tenant(
        &tenant_repo,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "acme",
        "Acme",
    )
    .await;
    let ctx = tenant_ctx("018e1234-5678-7abc-8def-0123456789ab");

    let role_id = parse_uuid("018e0000-0000-0000-0000-000000000001");
    let perms = vec![Permission::parse("tenant:user:read").unwrap_or_else(|e| panic!("{e}"))];
    let role = new_role(&tenant, role_id, "User Reader", perms);
    ok_or_panic(repo.create(&role, &ctx).await);

    let read = ok_or_panic(repo.by_id(parse_role(&role_id.to_string()), &ctx).await);
    assert_eq!(read.name, "User Reader");
    assert_eq!(read.permissions, vec!["tenant:user:read"]);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn duplicate_role_name_fails(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_repo = PostgresTenantRepository::new(pool.clone());
    let repo = PostgresRoleRepository::new(pool.clone());
    let tenant = create_tenant(
        &tenant_repo,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "acme",
        "Acme",
    )
    .await;
    let ctx = tenant_ctx("018e1234-5678-7abc-8def-0123456789ab");

    ok_or_panic(
        repo.create(
            &new_role(
                &tenant,
                parse_uuid("018e0000-0000-0000-0000-000000000001"),
                "Admin",
                vec![],
            ),
            &ctx,
        )
        .await,
    );
    let result = repo
        .create(
            &new_role(
                &tenant,
                parse_uuid("018e0000-0000-0000-0000-000000000002"),
                "Admin",
                vec![],
            ),
            &ctx,
        )
        .await;
    assert!(result.is_err());

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn tenant_role_cannot_hold_platform_permission(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_repo = PostgresTenantRepository::new(pool.clone());
    let tenant = create_tenant(
        &tenant_repo,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "acme",
        "Acme",
    )
    .await;

    let role_id = parse_uuid("018e0000-0000-0000-0000-000000000001");
    let perms = vec![Permission::parse("platform:tenant:read").unwrap_or_else(|e| panic!("{e}"))];
    let role_result = Role::new(
        parse_role(&role_id.to_string()),
        Some(tenant.id),
        "Admin",
        false,
        perms,
        &SystemClock,
        None,
    );
    assert!(role_result.is_err());

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn unknown_permission_is_rejected(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_repo = PostgresTenantRepository::new(pool.clone());
    let _tenant = create_tenant(
        &tenant_repo,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "acme",
        "Acme",
    )
    .await;

    assert!(Permission::parse("tenant:missing").is_err());

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn builtin_role_cannot_be_modified_or_deleted(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_repo = PostgresTenantRepository::new(pool.clone());
    let repo = PostgresRoleRepository::new(pool.clone());
    let tenant = create_tenant(
        &tenant_repo,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "acme",
        "Acme",
    )
    .await;
    let ctx = tenant_ctx("018e1234-5678-7abc-8def-0123456789ab");

    let role_id = parse_uuid("018e0000-0000-0000-0000-000000000001");
    let role = ok_or_panic(Role::new(
        parse_role(&role_id.to_string()),
        Some(tenant.id),
        "Builtin Admin",
        true,
        vec![],
        &SystemClock,
        None,
    ));
    ok_or_panic(repo.create(&role, &ctx).await);

    let mut read = ok_or_panic(repo.by_id(parse_role(&role_id.to_string()), &ctx).await);
    read.name = "Updated".to_string();
    read.revision = read.revision.next();
    let update_result = repo.update(&read, Revision::initial(), &ctx).await;
    assert!(update_result.is_err());

    let delete_result = repo
        .delete(
            parse_role(&role_id.to_string()),
            Revision::initial(),
            SystemClock.now(),
            &ctx,
        )
        .await;
    assert!(delete_result.is_err());

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn grant_and_revoke_bump_revision(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_repo = PostgresTenantRepository::new(pool.clone());
    let repo = PostgresRoleRepository::new(pool.clone());
    let tenant = create_tenant(
        &tenant_repo,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "acme",
        "Acme",
    )
    .await;
    let ctx = tenant_ctx("018e1234-5678-7abc-8def-0123456789ab");

    let role_id = parse_uuid("018e0000-0000-0000-0000-000000000001");
    let role = new_role(&tenant, role_id, "Admin", vec![]);
    ok_or_panic(repo.create(&role, &ctx).await);

    let mut read = ok_or_panic(repo.by_id(parse_role(&role_id.to_string()), &ctx).await);
    ok_or_panic(read.grant_permission(
        Permission::parse("tenant:user:read").unwrap_or_else(|e| panic!("{e}")),
        &SystemClock,
        None,
    ));
    ok_or_panic(repo.update(&read, Revision::initial(), &ctx).await);

    let after_grant = ok_or_panic(repo.by_id(parse_role(&role_id.to_string()), &ctx).await);
    assert_eq!(after_grant.permissions, vec!["tenant:user:read"]);
    assert_eq!(after_grant.revision.value(), 2);

    let mut to_revoke = after_grant;
    ok_or_panic(to_revoke.revoke_permission("tenant:user:read", &SystemClock, None));
    ok_or_panic(repo.update(&to_revoke, Revision::new(2), &ctx).await);

    let after_revoke = ok_or_panic(repo.by_id(parse_role(&role_id.to_string()), &ctx).await);
    assert!(after_revoke.permissions.is_empty());
    assert_eq!(after_revoke.revision.value(), 3);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn roles_are_isolated_by_tenant(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_repo = PostgresTenantRepository::new(pool.clone());
    let repo = PostgresRoleRepository::new(pool.clone());
    let tenant_a = create_tenant(
        &tenant_repo,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "acme",
        "Acme",
    )
    .await;
    let _tenant_b = create_tenant(
        &tenant_repo,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ac"),
        "bravo",
        "Bravo",
    )
    .await;

    let ctx_a = tenant_ctx("018e1234-5678-7abc-8def-0123456789ab");
    let ctx_b = tenant_ctx("018e1234-5678-7abc-8def-0123456789ac");

    let role_id = parse_uuid("018e0000-0000-0000-0000-000000000001");
    ok_or_panic(
        repo.create(&new_role(&tenant_a, role_id, "Admin", vec![]), &ctx_a)
            .await,
    );

    let page_a = ok_or_panic(repo.list(&ctx_a, ListOptions::default()).await);
    assert_eq!(page_a.items.len(), 1);

    let page_b = ok_or_panic(repo.list(&ctx_b, ListOptions::default()).await);
    assert!(page_b.items.is_empty());

    Ok(())
}
