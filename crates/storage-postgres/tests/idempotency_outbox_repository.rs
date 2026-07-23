use chrono::Duration as ChronoDuration;
use foundation::chrono::{DateTime, Utc};
use foundation::uuid::Uuid;
use foundation::{Clock, RequestContext, SystemClock, TenantId, UserId, UtcTimestamp};
use storage_api::{IdempotencyRecord, IdempotencyRepository, OutboxMessage, OutboxRepository};
use storage_postgres::idempotency_repository::PostgresIdempotencyRepository;
use storage_postgres::outbox_repository::PostgresOutboxRepository;

fn ok_or_panic<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(v) => v,
        Err(e) => panic!("{e:?}"),
    }
}

fn some_or_panic<T>(option: Option<T>, message: &str) -> T {
    match option {
        Some(v) => v,
        None => panic!("{message}"),
    }
}

fn tenant() -> TenantId {
    ok_or_panic(TenantId::parse_str("018e1234-5678-7abc-8def-0123456789ab"))
}

fn principal() -> UserId {
    ok_or_panic(UserId::parse_str("018e0000-0000-0000-0000-000000000001"))
}

fn ctx() -> RequestContext {
    RequestContext {
        tenant_id: Some(tenant()),
        ..Default::default()
    }
}

fn now() -> UtcTimestamp {
    SystemClock.now()
}

fn future(hours: i64) -> UtcTimestamp {
    let dt: DateTime<Utc> = now().into();
    (dt + ChronoDuration::hours(hours)).into()
}

fn epoch() -> UtcTimestamp {
    UtcTimestamp::from(DateTime::<Utc>::UNIX_EPOCH)
}

fn message_id(seed: u8) -> Uuid {
    let mut bytes = [0u8; 16];
    bytes[15] = seed;
    Uuid::from_bytes(bytes)
}

#[sqlx::test(migrations = "../../migrations")]
async fn idempotency_save_and_find_round_trip(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresIdempotencyRepository::new(pool);
    let record = IdempotencyRecord {
        tenant_id: Some(tenant()),
        principal_id: principal(),
        endpoint_scope: "tenant.create".to_string(),
        idempotency_key: "key-1".to_string(),
        request_digest: "digest-1".to_string(),
        response_status: Some(200),
        response_body: Some("ok".to_string()),
        expires_at: future(1),
    };

    ok_or_panic(repo.save(&record, &ctx()).await);
    let found = ok_or_panic(
        repo.find(
            Some(tenant()),
            principal(),
            "tenant.create",
            "key-1",
            &ctx(),
        )
        .await,
    );
    assert!(found.is_some());
    let found = some_or_panic(found, "idempotency record not found");
    assert_eq!(found.request_digest, "digest-1");
    assert_eq!(found.response_status, Some(200));
    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn idempotency_conflict_stores_latest_digest(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresIdempotencyRepository::new(pool);
    let first = IdempotencyRecord {
        tenant_id: Some(tenant()),
        principal_id: principal(),
        endpoint_scope: "tenant.create".to_string(),
        idempotency_key: "key-1".to_string(),
        request_digest: "digest-1".to_string(),
        response_status: Some(200),
        response_body: Some("ok".to_string()),
        expires_at: future(1),
    };
    let second = IdempotencyRecord {
        request_digest: "digest-2".to_string(),
        response_status: Some(409),
        response_body: Some("conflict".to_string()),
        ..first.clone()
    };

    ok_or_panic(repo.save(&first, &ctx()).await);
    ok_or_panic(repo.save(&second, &ctx()).await);
    let found = ok_or_panic(
        repo.find(
            Some(tenant()),
            principal(),
            "tenant.create",
            "key-1",
            &ctx(),
        )
        .await,
    );
    assert_eq!(some_or_panic(found, "idempotency record not found").request_digest, "digest-2");
    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn idempotency_keys_are_isolated_by_principal(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresIdempotencyRepository::new(pool);
    let other_principal = ok_or_panic(UserId::parse_str("018e0000-0000-0000-0000-000000000002"));
    let record = IdempotencyRecord {
        tenant_id: Some(tenant()),
        principal_id: principal(),
        endpoint_scope: "tenant.create".to_string(),
        idempotency_key: "shared-key".to_string(),
        request_digest: "digest".to_string(),
        response_status: None,
        response_body: None,
        expires_at: future(1),
    };

    ok_or_panic(repo.save(&record, &ctx()).await);
    let found = ok_or_panic(
        repo.find(
            Some(tenant()),
            other_principal,
            "tenant.create",
            "shared-key",
            &ctx(),
        )
        .await,
    );
    assert!(found.is_none());
    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn outbox_append_and_claim_round_trip(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresOutboxRepository::new(pool.clone());
    let message = OutboxMessage {
        message_id: message_id(1),
        tenant_id: Some(tenant()),
        aggregate_type: "Tenant".to_string(),
        aggregate_id: "tenant-1".to_string(),
        aggregate_sequence: 1,
        event_type: "TenantCreated".to_string(),
        payload: "{}".to_string(),
        occurred_at: now(),
        available_at: epoch(),
        attempts: 0,
        published_at: None,
    };

    ok_or_panic(repo.append(&message, &ctx()).await);
    let claimed = ok_or_panic(
        repo.claim(10, std::time::Duration::from_secs(1), &ctx())
            .await,
    );
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].message_id, message.message_id);
    assert_eq!(claimed[0].attempts, 1);

    ok_or_panic(repo.mark_published(message.message_id, now(), &ctx()).await);
    let claimed = ok_or_panic(
        repo.claim(10, std::time::Duration::from_secs(1), &ctx())
            .await,
    );
    assert!(claimed.is_empty());
    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn outbox_claim_is_bounded_and_ordered(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresOutboxRepository::new(pool.clone());
    for i in 1..=5 {
        let message = OutboxMessage {
            message_id: message_id(i),
            tenant_id: Some(tenant()),
            aggregate_type: "Tenant".to_string(),
            aggregate_id: "tenant-1".to_string(),
            aggregate_sequence: i64::from(i),
            event_type: "Event".to_string(),
            payload: "{}".to_string(),
            occurred_at: now(),
            available_at: (DateTime::from(epoch()) + ChronoDuration::milliseconds(i64::from(i)))
                .into(),
            attempts: 0,
            published_at: None,
        };
        ok_or_panic(repo.append(&message, &ctx()).await);
    }

    let claimed = ok_or_panic(repo.claim(2, std::time::Duration::ZERO, &ctx()).await);
    assert_eq!(claimed.len(), 2);
    assert!(
        claimed[0].available_at.timestamp_millis() <= claimed[1].available_at.timestamp_millis()
    );

    // Marking only the first still leaves the rest; message 2 was claimed with a
    // zero-length lease so it can be reclaimed immediately.
    ok_or_panic(
        repo.mark_published(claimed[0].message_id, now(), &ctx())
            .await,
    );
    let claimed = ok_or_panic(
        repo.claim(10, std::time::Duration::from_secs(1), &ctx())
            .await,
    );
    assert_eq!(claimed.len(), 4);
    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn outbox_append_is_idempotent(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresOutboxRepository::new(pool.clone());
    let message = OutboxMessage {
        message_id: message_id(1),
        tenant_id: Some(tenant()),
        aggregate_type: "Tenant".to_string(),
        aggregate_id: "tenant-1".to_string(),
        aggregate_sequence: 1,
        event_type: "TenantCreated".to_string(),
        payload: "{}".to_string(),
        occurred_at: now(),
        available_at: epoch(),
        attempts: 0,
        published_at: None,
    };

    ok_or_panic(repo.append(&message, &ctx()).await);
    ok_or_panic(repo.append(&message, &ctx()).await);
    let claimed = ok_or_panic(
        repo.claim(10, std::time::Duration::from_secs(1), &ctx())
            .await,
    );
    assert_eq!(claimed.len(), 1);
    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn outbox_unavailable_message_is_not_claimed(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresOutboxRepository::new(pool.clone());
    let message = OutboxMessage {
        message_id: message_id(1),
        tenant_id: Some(tenant()),
        aggregate_type: "Tenant".to_string(),
        aggregate_id: "tenant-1".to_string(),
        aggregate_sequence: 1,
        event_type: "TenantCreated".to_string(),
        payload: "{}".to_string(),
        occurred_at: now(),
        available_at: future(1),
        attempts: 0,
        published_at: None,
    };

    ok_or_panic(repo.append(&message, &ctx()).await);
    let claimed = ok_or_panic(
        repo.claim(10, std::time::Duration::from_secs(1), &ctx())
            .await,
    );
    assert!(claimed.is_empty());
    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn idempotency_record_survives_one_hundred_repeats(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresIdempotencyRepository::new(pool);
    let base = IdempotencyRecord {
        tenant_id: Some(tenant()),
        principal_id: principal(),
        endpoint_scope: "tenant.create".to_string(),
        idempotency_key: "key-100".to_string(),
        request_digest: "digest-0".to_string(),
        response_status: Some(200),
        response_body: Some("ok".to_string()),
        expires_at: future(1),
    };

    for i in 1..=100 {
        let mut record = base.clone();
        record.request_digest = format!("digest-{i}");
        ok_or_panic(repo.save(&record, &ctx()).await);
    }

    let found = ok_or_panic(
        repo.find(
            Some(tenant()),
            principal(),
            "tenant.create",
            "key-100",
            &ctx(),
        )
        .await,
    );
    assert_eq!(some_or_panic(found, "idempotency record not found").request_digest, "digest-100");
    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn outbox_claim_lease_recovers_from_crash_window(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresOutboxRepository::new(pool.clone());
    let message = OutboxMessage {
        message_id: message_id(1),
        tenant_id: Some(tenant()),
        aggregate_type: "Tenant".to_string(),
        aggregate_id: "tenant-1".to_string(),
        aggregate_sequence: 1,
        event_type: "TenantCreated".to_string(),
        payload: "{}".to_string(),
        occurred_at: now(),
        available_at: epoch(),
        attempts: 0,
        published_at: None,
    };

    ok_or_panic(repo.append(&message, &ctx()).await);

    // First publisher claims the message but crashes before marking published.
    let claimed = ok_or_panic(
        repo.claim(10, std::time::Duration::from_millis(100), &ctx())
            .await,
    );
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].attempts, 1);

    // Wait for the short lease to expire.
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    // A recovery publisher can claim the same message again.
    let claimed = ok_or_panic(
        repo.claim(10, std::time::Duration::from_secs(1), &ctx())
            .await,
    );
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].attempts, 2);

    ok_or_panic(
        repo.mark_published(claimed[0].message_id, now(), &ctx())
            .await,
    );
    let claimed = ok_or_panic(
        repo.claim(10, std::time::Duration::from_secs(1), &ctx())
            .await,
    );
    assert!(claimed.is_empty());
    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn outbox_dual_publisher_claims_only_once(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresOutboxRepository::new(pool.clone());
    let message = OutboxMessage {
        message_id: message_id(1),
        tenant_id: Some(tenant()),
        aggregate_type: "Tenant".to_string(),
        aggregate_id: "tenant-1".to_string(),
        aggregate_sequence: 1,
        event_type: "TenantCreated".to_string(),
        payload: "{}".to_string(),
        occurred_at: now(),
        available_at: epoch(),
        attempts: 0,
        published_at: None,
    };

    ok_or_panic(repo.append(&message, &ctx()).await);

    let repo_a = PostgresOutboxRepository::new(pool.clone());
    let repo_b = PostgresOutboxRepository::new(pool.clone());
    let ctx_a = ctx();
    let ctx_b = ctx();

    let (a, b) = tokio::join!(
        repo_a.claim(1, std::time::Duration::from_secs(1), &ctx_a),
        repo_b.claim(1, std::time::Duration::from_secs(1), &ctx_b),
    );

    let a = ok_or_panic(a);
    let b = ok_or_panic(b);
    assert_eq!(
        a.len() + b.len(),
        1,
        "only one publisher may claim the message"
    );

    let winner_id = if a.is_empty() {
        b[0].message_id
    } else {
        a[0].message_id
    };
    ok_or_panic(repo.mark_published(winner_id, now(), &ctx()).await);

    let claimed = ok_or_panic(
        repo.claim(10, std::time::Duration::from_secs(1), &ctx())
            .await,
    );
    assert!(claimed.is_empty());
    Ok(())
}
