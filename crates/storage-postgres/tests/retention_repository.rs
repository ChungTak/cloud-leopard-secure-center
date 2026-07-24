use chrono::Duration as ChronoDuration;
use domain_audit::retention::{
    LegalHold, RetentionPolicy, RetentionTarget, TenantRetentionOverride,
};
use foundation::{Clock, SystemClock, TenantId, UtcTimestamp};
use storage_api::RetentionRepository;
use storage_postgres::retention_repository::PostgresRetentionRepository;

fn ok_or_panic<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(v) => v,
        Err(e) => panic!("{e:?}"),
    }
}

fn now() -> UtcTimestamp {
    SystemClock.now()
}

fn future(days: i64) -> UtcTimestamp {
    let dt: chrono::DateTime<chrono::Utc> = now().into();
    (dt + ChronoDuration::days(days)).into()
}

fn past(days: i64) -> UtcTimestamp {
    let dt: chrono::DateTime<chrono::Utc> = now().into();
    (dt - ChronoDuration::days(days)).into()
}

async fn insert_audit_records(
    pool: &sqlx::PgPool,
    count: i64,
    occurred_at: UtcTimestamp,
) -> sqlx::Result<()> {
    for i in 0..count {
        sqlx::query(
            "INSERT INTO audit.records_default
             (tenant_id, actor_type, actor_id, action, target_type, target_id, result, risk, occurred_at, details)
             VALUES ($1, 'user', $2, 'login', 'User', $3, 'success', 'normal', $4, $5)",
        )
        .bind(foundation::uuid::Uuid::parse_str("018e1234-5678-7abc-8def-0123456789ab").unwrap_or_else(|e| panic!("{e}")))
        .bind(format!("actor-{i}"))
        .bind(format!("user-{i}"))
        .bind(chrono::DateTime::<chrono::Utc>::from(occurred_at))
        .bind(serde_json::json!({"ip": "127.0.0.1"}))
        .execute(pool)
        .await?;
    }
    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn save_and_retrieve_default_policy(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresRetentionRepository::new(pool.clone());
    let policy = ok_or_panic(RetentionPolicy::new(RetentionTarget::AuditEvents, 14, 2500));
    ok_or_panic(repo.save_policy(&policy).await);

    let fetched = ok_or_panic(repo.get_policy(RetentionTarget::AuditEvents).await);
    assert_eq!(fetched.days, 14);
    assert_eq!(fetched.max_batch_size, 2500);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn tenant_override_wins(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant = ok_or_panic(TenantId::parse_str("018e1234-5678-7abc-8def-0123456789ab"));
    let repo = PostgresRetentionRepository::new(pool.clone());

    let default = ok_or_panic(RetentionPolicy::new(
        RetentionTarget::AuditRecords,
        365,
        1000,
    ));
    ok_or_panic(repo.save_policy(&default).await);

    let override_value = ok_or_panic(TenantRetentionOverride::new(
        tenant,
        RetentionTarget::AuditRecords,
        30,
    ));
    ok_or_panic(repo.set_tenant_override(&override_value).await);

    assert_eq!(
        ok_or_panic(
            repo.get_effective_days(RetentionTarget::AuditRecords, Some(tenant))
                .await
        ),
        30
    );
    assert_eq!(
        ok_or_panic(
            repo.get_effective_days(RetentionTarget::AuditRecords, None)
                .await
        ),
        365
    );

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn cleanup_batch_respects_batch_size(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresRetentionRepository::new(pool.clone());
    let occurred = past(30);
    insert_audit_records(&pool, 5, occurred).await?;

    let first = ok_or_panic(
        repo.cleanup_batch(
            RetentionTarget::AuditRecords,
            "records_default",
            future(1),
            2,
        )
        .await,
    );
    assert_eq!(first.rows_deleted, 2);
    assert!(!first.finished);

    let second = ok_or_panic(
        repo.cleanup_batch(
            RetentionTarget::AuditRecords,
            "records_default",
            future(1),
            2,
        )
        .await,
    );
    assert_eq!(second.rows_deleted, 2);
    assert!(!second.finished);

    let third = ok_or_panic(
        repo.cleanup_batch(
            RetentionTarget::AuditRecords,
            "records_default",
            future(1),
            2,
        )
        .await,
    );
    assert_eq!(third.rows_deleted, 1);
    assert!(third.finished);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn legal_hold_prevents_cleanup(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresRetentionRepository::new(pool.clone());
    let occurred = past(30);
    insert_audit_records(&pool, 1, occurred).await?;

    let hold = LegalHold::new("User", "user-0", future(10));
    ok_or_panic(repo.add_legal_hold(&hold).await);

    let first = ok_or_panic(
        repo.cleanup_batch(
            RetentionTarget::AuditRecords,
            "records_default",
            future(1),
            1000,
        )
        .await,
    );
    assert_eq!(first.rows_deleted, 0);
    assert!(first.finished);

    ok_or_panic(repo.remove_legal_hold("User", "user-0").await);

    let second = ok_or_panic(
        repo.cleanup_batch(
            RetentionTarget::AuditRecords,
            "records_default",
            future(1),
            1000,
        )
        .await,
    );
    assert_eq!(second.rows_deleted, 1);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn double_worker_lease_prevents_concurrent_cleanup(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresRetentionRepository::new(pool.clone());

    let acquired1 = ok_or_panic(
        repo.acquire_lease(
            RetentionTarget::AuditRecords,
            "records_default",
            "worker-1",
            future(1),
            now(),
        )
        .await,
    );
    assert!(acquired1);

    let acquired2 = ok_or_panic(
        repo.acquire_lease(
            RetentionTarget::AuditRecords,
            "records_default",
            "worker-2",
            future(1),
            now(),
        )
        .await,
    );
    assert!(!acquired2);

    ok_or_panic(
        repo.release_lease(RetentionTarget::AuditRecords, "records_default", "worker-1")
            .await,
    );

    let acquired3 = ok_or_panic(
        repo.acquire_lease(
            RetentionTarget::AuditRecords,
            "records_default",
            "worker-2",
            future(1),
            now(),
        )
        .await,
    );
    assert!(acquired3);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn expired_lease_allows_recovery(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresRetentionRepository::new(pool.clone());

    let acquired1 = ok_or_panic(
        repo.acquire_lease(
            RetentionTarget::AuditRecords,
            "records_default",
            "worker-1",
            future(1),
            now(),
        )
        .await,
    );
    assert!(acquired1);

    // Simulate a crashed worker by expiring the lease in the database.
    sqlx::query(
        "UPDATE audit.cleanup_checkpoint SET lease_until = now() - interval '1 minute' WHERE table_name = $1 AND partition_name = $2",
    )
    .bind(RetentionTarget::AuditRecords.as_str())
    .bind("records_default")
    .execute(&pool)
    .await?;

    let acquired2 = ok_or_panic(
        repo.acquire_lease(
            RetentionTarget::AuditRecords,
            "records_default",
            "worker-2",
            future(1),
            now(),
        )
        .await,
    );
    assert!(acquired2);

    Ok(())
}
