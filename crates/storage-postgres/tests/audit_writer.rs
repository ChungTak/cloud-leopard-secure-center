use domain_audit::audit_record::{ActionRisk, AuditDetails, AuditRecord, AuditResult};
use domain_organization::tenant::Tenant;
use foundation::{FakeClock, RequestContext, TenantId, uuid::Uuid};
use storage_api::{AuditWriter, TenantRepository};
use storage_postgres::audit_writer::PostgresAuditWriter;
use storage_postgres::begin_tenant_transaction;
use storage_postgres::tenant_repository::PostgresTenantRepository;

fn ok_or_panic<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(v) => v,
        Err(e) => panic!("{e:?}"),
    }
}

fn tenant_ctx(tenant: &str) -> RequestContext {
    RequestContext {
        tenant_id: Some(TenantId::parse_str(tenant).unwrap_or_else(|e| panic!("{e}"))),
        ..Default::default()
    }
}

async fn create_tenant(pool: &sqlx::PgPool, id: Uuid, code: &str, name: &str) -> TenantId {
    let repo = PostgresTenantRepository::new(pool.clone());
    let tenant = ok_or_panic(Tenant::new(
        TenantId::parse_str(&id.to_string()).unwrap_or_else(|e| panic!("{e}")),
        code,
        name,
        Option::<&str>::None,
        Option::<&str>::None,
        &FakeClock::from_millis(1_000_000_000_000),
        None,
    ));
    ok_or_panic(repo.create(&tenant, &tenant_ctx(&id.to_string())).await);
    TenantId::parse_str(&id.to_string()).unwrap_or_else(|e| panic!("{e}"))
}

fn sample_record(tenant: TenantId, clock: &FakeClock) -> AuditRecord {
    let details = ok_or_panic(AuditDetails::new("user.update", "{\"field\":\"name\"}"));
    ok_or_panic(AuditRecord::new(
        tenant,
        "user",
        "018e0000-0000-0000-0000-000000000001",
        "tenant.user.update",
        "user",
        "018e0000-0000-0000-0000-000000000001",
        AuditResult::Success,
        ActionRisk::High,
        details,
        clock,
    ))
}

#[sqlx::test(migrations = "../../migrations")]
async fn writes_audit_record_and_returns_id(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant = create_tenant(
        &pool,
        Uuid::parse_str("018e1234-5678-7abc-8def-0123456789ab").unwrap_or_else(|e| panic!("{e}")),
        "t1",
        "Tenant",
    )
    .await;
    let ctx = tenant_ctx(&tenant.as_uuid().to_string());
    let writer = PostgresAuditWriter::new(pool.clone());
    let clock = FakeClock::from_millis(1_000_000_000_000);

    let record = sample_record(tenant, &clock);
    let id = ok_or_panic(writer.write(&record, &ctx).await);
    assert!(id.value() > 0);

    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit.records")
        .fetch_one(&pool)
        .await?;
    assert_eq!(count.0, 1);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn append_only_denies_update_and_delete(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant = create_tenant(
        &pool,
        Uuid::parse_str("018e1234-5678-7abc-8def-0123456789ab").unwrap_or_else(|e| panic!("{e}")),
        "t1",
        "Tenant",
    )
    .await;
    let ctx = tenant_ctx(&tenant.as_uuid().to_string());
    let writer = PostgresAuditWriter::new(pool.clone());
    let clock = FakeClock::from_millis(1_000_000_000_000);

    let record = sample_record(tenant, &clock);
    let id = ok_or_panic(writer.write(&record, &ctx).await);

    let tx_managed = begin_tenant_transaction(&pool, &ctx)
        .await
        .unwrap_or_else(|e| panic!("{e:?}"));
    let mut tx = tx_managed.lock().await;
    let update = sqlx::query("UPDATE audit.records SET action = 'x' WHERE id = $1")
        .bind(id.value())
        .execute(&mut *tx)
        .await;
    assert!(update.is_err());
    drop(tx);
    tx_managed
        .rollback()
        .await
        .unwrap_or_else(|e| panic!("{e:?}"));

    let tx_managed = begin_tenant_transaction(&pool, &ctx)
        .await
        .unwrap_or_else(|e| panic!("{e:?}"));
    let mut tx = tx_managed.lock().await;
    let delete = sqlx::query("DELETE FROM audit.records WHERE id = $1")
        .bind(id.value())
        .execute(&mut *tx)
        .await;
    assert!(delete.is_err());
    drop(tx);
    tx_managed
        .rollback()
        .await
        .unwrap_or_else(|e| panic!("{e:?}"));

    // Record must still be present.
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit.records WHERE id = $1")
        .bind(id.value())
        .fetch_one(&pool)
        .await?;
    assert_eq!(count.0, 1);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn out_of_range_occurred_at_goes_to_default_partition(
    pool: sqlx::PgPool,
) -> sqlx::Result<()> {
    let tenant = create_tenant(
        &pool,
        Uuid::parse_str("018e1234-5678-7abc-8def-0123456789ab").unwrap_or_else(|e| panic!("{e}")),
        "t1",
        "Tenant",
    )
    .await;
    let ctx = tenant_ctx(&tenant.as_uuid().to_string());
    let writer = PostgresAuditWriter::new(pool.clone());
    let clock = FakeClock::from_millis(1_000_000_000_000);

    let mut record = sample_record(tenant, &clock);
    // occurred_at defaults to clock.now(); force an out-of-range timestamp two years ago.
    let old_dt = foundation::chrono::DateTime::from_timestamp_millis(
        1_000_000_000_000 - 2 * 365 * 86_400_000,
    )
    .unwrap_or_else(|| panic!("invalid timestamp"));
    record.occurred_at = old_dt.into();

    let id = ok_or_panic(writer.write(&record, &ctx).await);

    let default_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM audit.records_default WHERE id = $1")
            .bind(id.value())
            .fetch_one(&pool)
            .await?;
    assert_eq!(default_count.0, 1);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn high_risk_write_failure_is_not_silent(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant = create_tenant(
        &pool,
        Uuid::parse_str("018e1234-5678-7abc-8def-0123456789ab").unwrap_or_else(|e| panic!("{e}")),
        "t1",
        "Tenant",
    )
    .await;
    let writer = PostgresAuditWriter::new(pool.clone());
    let clock = FakeClock::from_millis(1_000_000_000_000);

    let mut record = sample_record(tenant, &clock);
    record.details = ok_or_panic(AuditDetails::new("x", "{\"x\":\"y\"}"));
    // Wrong tenant context should cause a tenant mismatch error before DB write.
    let wrong_ctx = tenant_ctx("018f1234-5678-7abc-8def-0123456789ab");
    assert!(writer.write(&record, &wrong_ctx).await.is_err());

    Ok(())
}
