use foundation::chrono::{Duration, Utc};
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
async fn audit_tables_are_partitioned_and_tenant_isolated(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_id = parse_uuid("018e1234-5678-7abc-8def-0123456789ab");

    sqlx::query(
        "INSERT INTO audit.events (tenant_id, event_type, created_at) VALUES ($1, 'login', $2)",
    )
    .bind(tenant_id)
    .bind(Utc::now())
    .execute(&pool)
    .await?;

    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");
    let mut tx = begin_tx(&pool, &ctx).await;
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit.events")
        .fetch_one(&mut *tx)
        .await?;
    assert_eq!(row.0, 1);
    tx.rollback().await?;

    let other = ctx_for("018f1234-5678-7abc-8def-0123456789ab");
    let mut tx = begin_tx(&pool, &other).await;
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit.events")
        .fetch_one(&mut *tx)
        .await?;
    assert_eq!(row.0, 0);
    tx.rollback().await?;

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn purge_partition_removes_old_rows_in_batches(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant_id = parse_uuid("018e1234-5678-7abc-8def-0123456789ab");
    let partition_name = format!("events_{}", Utc::now().format("%Y_%m"));
    let old = Utc::now() - Duration::seconds(86400);

    for _ in 0..5 {
        sqlx::query(
            "INSERT INTO audit.events (tenant_id, event_type, created_at) VALUES ($1, 'test', $2)",
        )
        .bind(tenant_id)
        .bind(old)
        .execute(&pool)
        .await?;
    }

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit.events")
        .fetch_one(&pool)
        .await?;
    assert_eq!(count.0, 5);

    let deleted: (i64,) = sqlx::query_as("SELECT audit.purge_partition('events', $1, $2, $3)")
        .bind(&partition_name)
        .bind(Utc::now())
        .bind(2_i64)
        .fetch_one(&pool)
        .await?;
    assert_eq!(deleted.0, 5);

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit.events")
        .fetch_one(&pool)
        .await?;
    assert_eq!(count.0, 0);

    Ok(())
}
