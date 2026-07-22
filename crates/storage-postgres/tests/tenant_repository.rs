use domain_identity::tenant::{Tenant, TenantStatus};
use foundation::{RequestContext, Revision, SystemClock, TenantId, UserId, uuid::Uuid};
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

fn new_tenant(id: Uuid, name: &str, actor: Option<UserId>) -> Tenant {
    Tenant::new(parse_tenant(&id.to_string()), name, &SystemClock, actor)
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
    let tenant = new_tenant(tenant_id, "acme", None);

    ok_or_panic(
        repo.create(&tenant, &ctx_for("018e1234-5678-7abc-8def-0123456789ab"))
            .await,
    );

    let loaded = ok_or_panic(
        repo.by_id(tenant.id, &ctx_for("018e1234-5678-7abc-8def-0123456789ab"))
            .await,
    );
    assert_eq!(loaded.name, "acme");
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
    let mut tenant = new_tenant(tenant_id, "acme", None);
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");

    ok_or_panic(repo.create(&tenant, &ctx).await);

    tenant.rename("acme updated", &SystemClock, None);
    let result = repo.update(&tenant, Revision::new(99), &ctx).await;
    assert!(result.is_err());

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn delete_with_expected_revision_succeeds(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresTenantRepository::new(pool);
    let tenant_id = parse_uuid("018e1234-5678-7abc-8def-0123456789ab");
    let tenant = new_tenant(tenant_id, "acme", None);
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");

    ok_or_panic(repo.create(&tenant, &ctx).await);
    ok_or_panic(repo.delete(tenant.id, Revision::initial(), &ctx).await);

    let not_found = repo.by_id(tenant.id, &ctx).await;
    assert!(not_found.is_err());

    Ok(())
}
