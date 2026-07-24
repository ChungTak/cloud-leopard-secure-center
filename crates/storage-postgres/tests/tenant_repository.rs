use domain_organization::tenant::{Tenant, TenantStatus};
use foundation::{Clock, RequestContext, Revision, SystemClock, TenantId, UserId, uuid::Uuid};
use storage_api::TenantRepository;
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

fn ctx_for(tenant: &str) -> RequestContext {
    RequestContext {
        tenant_id: Some(parse_tenant(tenant)),
        ..Default::default()
    }
}

fn new_tenant(id: Uuid, code: &str, name: &str, actor: Option<UserId>) -> Tenant {
    ok_or_panic(Tenant::new(
        parse_tenant(&id.to_string()),
        code,
        name,
        Option::<&str>::None,
        Option::<&str>::None,
        &SystemClock,
        actor,
    ))
}

fn ok_or_panic<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
    match result {
        Ok(v) => v,
        Err(e) => panic!("{e}"),
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_and_read_round_trip(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresTenantRepository::new(pool);
    let tenant_id = parse_uuid("018e1234-5678-7abc-8def-0123456789ab");
    let tenant = new_tenant(tenant_id, "acme", "Acme Corp", None);

    ok_or_panic(
        repo.create(&tenant, &ctx_for("018e1234-5678-7abc-8def-0123456789ab"))
            .await,
    );

    let loaded = ok_or_panic(
        repo.by_id(tenant.id, &ctx_for("018e1234-5678-7abc-8def-0123456789ab"))
            .await,
    );
    assert_eq!(loaded.code, "acme");
    assert_eq!(loaded.name, "Acme Corp");
    assert_eq!(loaded.locale, "en-US");
    assert_eq!(loaded.timezone, "UTC");
    assert_eq!(loaded.status, TenantStatus::Active);

    let not_found = repo
        .by_id(tenant.id, &ctx_for("018f1234-5678-7abc-8def-0123456789ab"))
        .await;
    assert!(not_found.is_err());

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn update_with_stale_revision_fails(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresTenantRepository::new(pool);
    let tenant_id = parse_uuid("018e1234-5678-7abc-8def-0123456789ab");
    let mut tenant = new_tenant(tenant_id, "acme", "Acme Corp", None);
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");

    ok_or_panic(repo.create(&tenant, &ctx).await);

    ok_or_panic(tenant.rename("Acme Updated", &SystemClock, None));
    let result = repo.update(&tenant, Revision::new(99), &ctx).await;
    assert!(result.is_err());

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn delete_with_expected_revision_succeeds(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresTenantRepository::new(pool);
    let tenant_id = parse_uuid("018e1234-5678-7abc-8def-0123456789ab");
    let tenant = new_tenant(tenant_id, "acme", "Acme Corp", None);
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");

    ok_or_panic(repo.create(&tenant, &ctx).await);
    ok_or_panic(
        repo.delete(tenant.id, Revision::initial(), SystemClock.now(), &ctx)
            .await,
    );

    let not_found = repo.by_id(tenant.id, &ctx).await;
    assert!(not_found.is_err());

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn duplicate_tenant_code_fails(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresTenantRepository::new(pool);
    let first = new_tenant(
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "acme",
        "Acme Corp",
        None,
    );
    let second = new_tenant(
        parse_uuid("018f1234-5678-7abc-8def-0123456789ab"),
        "acme",
        "Acme Two",
        None,
    );
    let first_ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");
    let second_ctx = ctx_for("018f1234-5678-7abc-8def-0123456789ab");

    ok_or_panic(repo.create(&first, &first_ctx).await);
    let result = repo.create(&second, &second_ctx).await;
    assert!(result.is_err());

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn suspended_tenant_blocks_new_sessions(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresTenantRepository::new(pool);
    let tenant_id = parse_uuid("018e1234-5678-7abc-8def-0123456789ab");
    let mut tenant = new_tenant(tenant_id, "acme", "Acme Corp", None);
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");

    ok_or_panic(repo.create(&tenant, &ctx).await);
    ok_or_panic(tenant.suspend(&SystemClock, None));
    ok_or_panic(repo.update(&tenant, tenant.revision.prev(), &ctx).await);

    let loaded = ok_or_panic(repo.by_id(tenant.id, &ctx).await);
    assert_eq!(loaded.status, TenantStatus::Suspended);
    assert!(!loaded.allows_new_sessions());

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn closed_tenant_is_persisted_and_terminal(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresTenantRepository::new(pool);
    let tenant_id = parse_uuid("018e1234-5678-7abc-8def-0123456789ab");
    let mut tenant = new_tenant(tenant_id, "acme", "Acme Corp", None);
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");

    ok_or_panic(repo.create(&tenant, &ctx).await);
    tenant.close(&SystemClock, None);
    ok_or_panic(repo.update(&tenant, tenant.revision.prev(), &ctx).await);

    let loaded = ok_or_panic(repo.by_id(tenant.id, &ctx).await);
    assert_eq!(loaded.status, TenantStatus::Closed);
    assert!(loaded.is_closed());

    Ok(())
}
