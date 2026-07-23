use foundation::uuid::Uuid;
use foundation::{RequestContext, SystemClock, TenantId, UserId, UtcTimestamp};
use storage_api::UnitOfWork;
use storage_api::{OutboxMessage, OutboxRepository, TenantRepository};
use storage_postgres::outbox_repository::PostgresOutboxRepository;
use storage_postgres::tenant_repository::PostgresTenantRepository;
use storage_postgres::unit_of_work::PostgresUnitOfWork;

fn ok_or_panic<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(v) => v,
        Err(e) => panic!("{e:?}"),
    }
}

fn parse_uuid(s: &str) -> Uuid {
    ok_or_panic(Uuid::parse_str(s))
}

fn parse_tenant(s: &str) -> TenantId {
    ok_or_panic(TenantId::parse_str(s))
}

fn ctx_for(tenant: &str) -> RequestContext {
    RequestContext {
        tenant_id: Some(parse_tenant(tenant)),
        actor_id: Some(
            UserId::parse_str("018e0000-0000-0000-0000-000000000001")
                .unwrap_or_else(|e| panic!("{e:?}")),
        ),
        ..Default::default()
    }
}

fn new_tenant(id: Uuid, code: &str, name: &str) -> domain_organization::tenant::Tenant {
    ok_or_panic(domain_organization::tenant::Tenant::new(
        parse_tenant(&id.to_string()),
        code.to_string(),
        name.to_string(),
        None::<String>,
        None::<String>,
        &SystemClock,
        Some(
            UserId::parse_str("018e0000-0000-0000-0000-000000000001")
                .unwrap_or_else(|e| panic!("{e:?}")),
        ),
    ))
}

#[sqlx::test(migrations = "../../migrations")]
async fn uow_commits_aggregate_and_outbox_together(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let uow = PostgresUnitOfWork::new(pool.clone());
    let tenant_id = parse_uuid("018e1234-5678-7abc-8def-0123456789ab");
    let ctx = ctx_for(&tenant_id.to_string());

    uow.execute(&ctx, || async {
        let repo = PostgresTenantRepository::new(pool.clone());
        let outbox = PostgresOutboxRepository::new(pool.clone());
        let tenant = new_tenant(tenant_id, "acme", "Acme Corp");
        repo.create(&tenant, &ctx).await?;

        let message = OutboxMessage {
            message_id: parse_uuid("00000000-0000-0000-0000-000000000001"),
            tenant_id: Some(parse_tenant(&tenant_id.to_string())),
            aggregate_type: "Tenant".to_string(),
            aggregate_id: tenant_id.to_string(),
            aggregate_sequence: 1,
            event_type: "TenantCreated".to_string(),
            payload: "{}".to_string(),
            occurred_at: UtcTimestamp::now(),
            available_at: UtcTimestamp::from(
                foundation::chrono::DateTime::<foundation::chrono::Utc>::UNIX_EPOCH,
            ),
            attempts: 0,
            published_at: None,
        };
        outbox.append(&message, &ctx).await?;

        Ok::<(), foundation::PlatformError>(())
    })
    .await
    .unwrap_or_else(|e| panic!("{e:?}"));

    let repo = PostgresTenantRepository::new(pool.clone());
    let outbox = PostgresOutboxRepository::new(pool.clone());
    let loaded = ok_or_panic(repo.by_id(parse_tenant(&tenant_id.to_string()), &ctx).await);
    assert_eq!(loaded.code, "acme");

    let claimed = ok_or_panic(
        outbox
            .claim(10, std::time::Duration::from_secs(1), &ctx)
            .await,
    );
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].aggregate_id, tenant_id.to_string());

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn uow_rolls_back_both_sides_on_failure(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let uow = PostgresUnitOfWork::new(pool.clone());
    let tenant_id = parse_uuid("018e1234-5678-7abc-8def-0123456789ab");
    let ctx = ctx_for(&tenant_id.to_string());

    let result: Result<(), foundation::PlatformError> = uow
        .execute(&ctx, || async {
            let repo = PostgresTenantRepository::new(pool.clone());
            let outbox = PostgresOutboxRepository::new(pool.clone());
            let tenant = new_tenant(tenant_id, "acme", "Acme Corp");
            repo.create(&tenant, &ctx).await?;

            let message = OutboxMessage {
                message_id: parse_uuid("00000000-0000-0000-0000-000000000002"),
                tenant_id: Some(parse_tenant(&tenant_id.to_string())),
                aggregate_type: "Tenant".to_string(),
                aggregate_id: tenant_id.to_string(),
                aggregate_sequence: 1,
                event_type: "TenantCreated".to_string(),
                payload: "{}".to_string(),
                occurred_at: UtcTimestamp::now(),
                available_at: UtcTimestamp::from(
                    foundation::chrono::DateTime::<foundation::chrono::Utc>::UNIX_EPOCH,
                ),
                attempts: 0,
                published_at: None,
            };
            outbox.append(&message, &ctx).await?;

            Err(foundation::PlatformError::invalid("simulate", "rollback"))
        })
        .await;

    assert!(result.is_err());

    let repo = PostgresTenantRepository::new(pool.clone());
    let outbox = PostgresOutboxRepository::new(pool.clone());
    let loaded = repo.by_id(parse_tenant(&tenant_id.to_string()), &ctx).await;
    assert!(loaded.is_err());

    let claimed = ok_or_panic(
        outbox
            .claim(10, std::time::Duration::from_secs(1), &ctx)
            .await,
    );
    assert!(claimed.is_empty());

    Ok(())
}
