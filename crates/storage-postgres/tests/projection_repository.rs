use domain_organization::tenant::Tenant;
use domain_resource::projection::{ChannelEvent, DeviceEvent, ProjectionFailure};
use foundation::{
    Clock, FakeClock, RequestContext, SystemClock, TenantId, UtcTimestamp, uuid::Uuid,
};
use storage_api::{ProjectionRepository, TenantRepository};
use storage_postgres::projection_repository::PostgresProjectionRepository;
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

fn device_event(external_ref: &str, sequence: i64, source: &str, payload: &str) -> DeviceEvent {
    DeviceEvent {
        external_ref: external_ref.to_string(),
        sequence,
        source_event_id: source.to_string(),
        observed_at: UtcTimestamp::now(),
        payload: payload.to_string(),
    }
}

fn channel_event(external_ref: &str, sequence: i64, source: &str, payload: &str) -> ChannelEvent {
    ChannelEvent {
        external_ref: external_ref.to_string(),
        sequence,
        source_event_id: source.to_string(),
        observed_at: UtcTimestamp::now(),
        payload: payload.to_string(),
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn device_projection_returns_observed_fields(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant = create_tenant(
        &pool,
        Uuid::parse_str("018e1234-5678-7abc-8def-0123456789ab").unwrap_or_else(|e| panic!("{e}")),
        "t1",
        "Tenant",
    )
    .await;
    let ctx = tenant_ctx(&tenant.as_uuid().to_string());
    let repo = PostgresProjectionRepository::new(pool);

    let event = device_event("dev-1", 1, "evt-1", "{\"online\":true}");
    ok_or_panic(repo.apply_device_event(event, &ctx).await);

    let projection = ok_or_panic(repo.get_device("dev-1", SystemClock.now(), &ctx).await);
    assert_eq!(projection.external_ref, "dev-1");
    assert_eq!(projection.sequence, 1);
    assert_eq!(projection.source_event_id, "evt-1");
    assert!(!projection.stale);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn out_of_order_and_duplicate_events_are_ignored(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant = create_tenant(
        &pool,
        Uuid::parse_str("018e1234-5678-7abc-8def-0123456789ab").unwrap_or_else(|e| panic!("{e}")),
        "t1",
        "Tenant",
    )
    .await;
    let ctx = tenant_ctx(&tenant.as_uuid().to_string());
    let repo = PostgresProjectionRepository::new(pool);

    let first = device_event("dev-1", 1, "evt-1", "{\"online\":true}");
    let duplicate = device_event("dev-1", 1, "evt-1", "{\"online\":true}");
    let out_of_order = device_event("dev-1", 0, "evt-0", "{\"online\":false}");

    ok_or_panic(repo.apply_device_event(first, &ctx).await);
    ok_or_panic(repo.apply_device_event(duplicate, &ctx).await);
    ok_or_panic(repo.apply_device_event(out_of_order, &ctx).await);

    let projection = ok_or_panic(repo.get_device("dev-1", SystemClock.now(), &ctx).await);
    assert_eq!(projection.sequence, 1);
    assert_eq!(projection.source_event_id, "evt-1");

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn gap_marks_projection_stale(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant = create_tenant(
        &pool,
        Uuid::parse_str("018e1234-5678-7abc-8def-0123456789ab").unwrap_or_else(|e| panic!("{e}")),
        "t1",
        "Tenant",
    )
    .await;
    let ctx = tenant_ctx(&tenant.as_uuid().to_string());
    let repo = PostgresProjectionRepository::new(pool);

    let first = device_event("dev-1", 1, "evt-1", "{\"online\":true}");
    let gap = device_event("dev-1", 3, "evt-3", "{\"online\":false}");

    ok_or_panic(repo.apply_device_event(first, &ctx).await);
    ok_or_panic(repo.apply_device_event(gap, &ctx).await);

    let projection = ok_or_panic(repo.get_device("dev-1", SystemClock.now(), &ctx).await);
    assert_eq!(projection.sequence, 3);
    assert!(projection.stale);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn mismatched_payload_for_same_sequence_is_quarantined(
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
    let repo = PostgresProjectionRepository::new(pool);

    let first = device_event("dev-1", 1, "evt-1", "{\"online\":true}");
    let mismatch = device_event("dev-1", 1, "evt-1", "{\"online\":false}");

    ok_or_panic(repo.apply_device_event(first, &ctx).await);
    ok_or_panic(repo.apply_device_event(mismatch, &ctx).await);

    let projection = ok_or_panic(repo.get_device("dev-1", SystemClock.now(), &ctx).await);
    assert_eq!(projection.sequence, 1);
    assert_eq!(projection.payload, "{\"online\":true}");

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn shadow_rebuild_and_atomic_view_swap(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant = create_tenant(
        &pool,
        Uuid::parse_str("018e1234-5678-7abc-8def-0123456789ab").unwrap_or_else(|e| panic!("{e}")),
        "t1",
        "Tenant",
    )
    .await;
    let ctx = tenant_ctx(&tenant.as_uuid().to_string());
    let repo = PostgresProjectionRepository::new(pool);

    let initial = device_event("dev-1", 1, "evt-1", "{\"online\":true}");
    ok_or_panic(repo.apply_device_event(initial, &ctx).await);

    let rebuilt = device_event("dev-1", 2, "evt-2", "{\"online\":false}");
    let channel = channel_event("ch-1", 1, "evt-ch-1", "{\"stream\":true}");
    ok_or_panic(
        repo.rebuild_shadow(vec![rebuilt], vec![channel], SystemClock.now(), &ctx)
            .await,
    );

    let device_proj = ok_or_panic(repo.get_device("dev-1", SystemClock.now(), &ctx).await);
    assert_eq!(device_proj.sequence, 2);
    assert_eq!(device_proj.payload, "{\"online\":false}");
    assert!(!device_proj.stale);

    let channel_proj = ok_or_panic(repo.get_channel("ch-1", SystemClock.now(), &ctx).await);
    assert_eq!(channel_proj.sequence, 1);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn checkpoint_and_failure_are_persisted(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant = create_tenant(
        &pool,
        Uuid::parse_str("018e1234-5678-7abc-8def-0123456789ab").unwrap_or_else(|e| panic!("{e}")),
        "t1",
        "Tenant",
    )
    .await;
    let ctx = tenant_ctx(&tenant.as_uuid().to_string());
    let repo = PostgresProjectionRepository::new(pool);

    ok_or_panic(
        repo.checkpoint(
            "worker-1",
            "evt-42",
            SystemClock.now(),
            SystemClock.now(),
            &ctx,
        )
        .await,
    );

    let failure = ProjectionFailure {
        id: String::new(),
        source_event_id: "evt-42".to_string(),
        external_ref: "dev-1".to_string(),
        reason: "bad_payload".to_string(),
        payload: "not json".to_string(),
    };
    ok_or_panic(repo.record_failure(failure, SystemClock.now(), &ctx).await);

    Ok(())
}
