use foundation::{RequestContext, TenantId, uuid::Uuid};
use storage_postgres::begin_tenant_transaction;

fn parse_tenant(s: &str) -> TenantId {
    match TenantId::parse_str(s) {
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

fn ctx_for(tenant: &str) -> RequestContext {
    RequestContext {
        tenant_id: Some(parse_tenant(tenant)),
        ..Default::default()
    }
}

async fn begin_tx<'a>(
    pool: &'a sqlx::PgPool,
    context: &'a RequestContext,
) -> sqlx::Transaction<'a, sqlx::Postgres> {
    match begin_tenant_transaction(pool, context).await {
        Ok(tx) => tx,
        Err(e) => panic!("{e}"),
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn tenants_table_is_isolated_by_tenant(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_id = parse_uuid("018e1234-5678-7abc-8def-0123456789ab");
    sqlx::query(
        "INSERT INTO iam.tenants (id, name, status, revision) VALUES ($1, 'test', 'active', 1)",
    )
    .bind(tenant_id)
    .execute(&pool)
    .await?;

    let context = ctx_for("018e1234-5678-7abc-8def-0123456789ab");
    let mut tx = begin_tx(&pool, &context).await;
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM iam.tenants")
        .fetch_one(&mut *tx)
        .await?;
    assert_eq!(row.0, 1);
    tx.rollback().await?;

    let other = ctx_for("018f1234-5678-7abc-8def-0123456789ab");
    let mut tx = begin_tx(&pool, &other).await;
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM iam.tenants")
        .fetch_one(&mut *tx)
        .await?;
    assert_eq!(row.0, 0);
    tx.rollback().await?;

    let empty = RequestContext::default();
    let mut tx = begin_tx(&pool, &empty).await;
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM iam.tenants")
        .fetch_one(&mut *tx)
        .await?;
    assert_eq!(row.0, 0);
    tx.rollback().await?;

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn event_trigger_enables_rls_on_new_tenant_tables(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_id = parse_uuid("018e1234-5678-7abc-8def-0123456789ab");
    let device_id = parse_uuid("018e1111-1111-1111-1111-111111111111");

    sqlx::query(
        "CREATE TABLE resource.devices (
            id UUID PRIMARY KEY,
            tenant_id UUID NOT NULL,
            name TEXT NOT NULL
        )"
    )
    .execute(&pool)
    .await?;

    sqlx::query("INSERT INTO resource.devices (id, tenant_id, name) VALUES ($1, $2, 'cam')")
        .bind(device_id)
        .bind(tenant_id)
        .execute(&pool)
        .await?;

    let context = ctx_for("018e1234-5678-7abc-8def-0123456789ab");
    let mut tx = begin_tx(&pool, &context).await;
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM resource.devices")
        .fetch_one(&mut *tx)
        .await?;
    assert_eq!(row.0, 1);
    tx.rollback().await?;

    let empty = RequestContext::default();
    let mut tx = begin_tx(&pool, &empty).await;
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM resource.devices")
        .fetch_one(&mut *tx)
        .await?;
    assert_eq!(row.0, 0);
    tx.rollback().await?;

    Ok(())
}
