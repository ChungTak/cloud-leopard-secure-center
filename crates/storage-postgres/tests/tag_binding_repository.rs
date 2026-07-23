use domain_organization::tenant::Tenant;
use domain_resource::external_binding::{ExternalBinding, ExternalBindingState};
use domain_resource::tag::{MAX_TAGS_PER_RESOURCE, ResourceType, Tag};
use foundation::{ExternalBindingId, FakeClock, RequestContext, TagId, TenantId, uuid::Uuid};
use storage_api::{ExternalBindingRepository, ListOptions, TagRepository, TenantRepository};
use storage_postgres::external_binding_repository::PostgresExternalBindingRepository;
use storage_postgres::tag_repository::PostgresTagRepository;
use storage_postgres::tenant_repository::PostgresTenantRepository;

fn parse_uuid(s: &str) -> Uuid {
    Uuid::parse_str(s).unwrap_or_else(|e| panic!("{e}"))
}

fn parse_tenant(s: &str) -> TenantId {
    TenantId::parse_str(s).unwrap_or_else(|e| panic!("{e}"))
}

fn parse_tag(s: &str) -> TagId {
    TagId::parse_str(s).unwrap_or_else(|e| panic!("{e}"))
}

fn parse_binding(s: &str) -> ExternalBindingId {
    ExternalBindingId::parse_str(s).unwrap_or_else(|e| panic!("{e}"))
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

async fn create_tenant(pool: &sqlx::PgPool, id: Uuid, code: &str, name: &str) -> TenantId {
    let repo = PostgresTenantRepository::new(pool.clone());
    let tenant = ok_or_panic(Tenant::new(
        parse_tenant(&id.to_string()),
        code,
        name,
        Option::<&str>::None,
        Option::<&str>::None,
        &FakeClock::from_millis(1_000_000_000_000),
        None,
    ));
    ok_or_panic(repo.create(&tenant, &tenant_ctx(&id.to_string())).await);
    parse_tenant(&id.to_string())
}

#[sqlx::test(migrations = "../../migrations")]
async fn tag_key_is_normalized_and_count_is_limited(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant = create_tenant(
        &pool,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "t1",
        "Tenant",
    )
    .await;
    let repo = PostgresTagRepository::new(pool);
    let ctx = tenant_ctx(&tenant.as_uuid().to_string());
    let resource_id = parse_uuid("018e0000-0000-0000-0000-000000000001");

    let tag = ok_or_panic(Tag::new(
        parse_tag("018e0000-0000-0000-0000-000000000002"),
        tenant,
        ResourceType::Device,
        resource_id,
        "  Environment  ",
        "Production",
        &FakeClock::from_millis(1_000_000_000_000),
        None,
    ));
    ok_or_panic(repo.create(&tag, &ctx).await);

    let read = ok_or_panic(repo.by_id(tag.id, &ctx).await);
    assert_eq!(read.key, "environment");
    assert_eq!(read.value, "Production");

    for i in 1..MAX_TAGS_PER_RESOURCE {
        let t = ok_or_panic(Tag::new(
            TagId::parse_str(&format!("018e0000-0000-0000-0000-{:012}", i + 2))
                .unwrap_or_else(|e| panic!("{e}")),
            tenant,
            ResourceType::Device,
            resource_id,
            format!("key-{i}"),
            "v",
            &FakeClock::from_millis(1_000_000_000_000 + i as i64),
            None,
        ));
        ok_or_panic(repo.create(&t, &ctx).await);
    }

    let over = ok_or_panic(Tag::new(
        parse_tag("018effff-ffff-ffff-ffff-ffffffffffff"),
        tenant,
        ResourceType::Device,
        resource_id,
        "overflow",
        "v",
        &FakeClock::from_millis(1_000_000_001_000),
        None,
    ));
    assert!(repo.create(&over, &ctx).await.is_err());

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn external_binding_activation_and_conflict(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant = create_tenant(
        &pool,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "t1",
        "Tenant",
    )
    .await;
    let repo = PostgresExternalBindingRepository::new(pool);
    let ctx = tenant_ctx(&tenant.as_uuid().to_string());
    let resource_id = parse_uuid("018e0000-0000-0000-0000-000000000001");

    let first = ok_or_panic(ExternalBinding::auto_match(
        parse_binding("018e0000-0000-0000-0000-000000000002"),
        tenant,
        ResourceType::Device,
        resource_id,
        "upstream-123",
        "serial",
        &FakeClock::from_millis(1_000_000_000_000),
    ));
    ok_or_panic(repo.create(&first, &ctx).await);

    let second = ok_or_panic(ExternalBinding::auto_match(
        parse_binding("018e0000-0000-0000-0000-000000000003"),
        tenant,
        ResourceType::Device,
        parse_uuid("018e0000-0000-0000-0000-000000000002"),
        "upstream-123",
        "serial",
        &FakeClock::from_millis(1_000_000_000_001),
    ));
    ok_or_panic(repo.create(&second, &ctx).await);

    let activated = ok_or_panic(repo.activate(first.id, first.revision, &ctx).await);
    assert_eq!(activated.state, ExternalBindingState::Active);

    let conflicting = ok_or_panic(repo.activate(second.id, second.revision, &ctx).await);
    assert_eq!(conflicting.state, ExternalBindingState::Conflict);

    let list = ok_or_panic(
        repo.list_by_external_ref("serial", "upstream-123", &ctx, ListOptions::default())
            .await,
    );
    let states: Vec<_> = list.items.iter().map(|b| b.state).collect();
    assert!(states.contains(&ExternalBindingState::Active));
    assert!(states.contains(&ExternalBindingState::Conflict));

    ok_or_panic(repo.disable(activated.id, activated.revision, &ctx).await);
    let disabled = ok_or_panic(repo.by_id(activated.id, &ctx).await);
    assert_eq!(disabled.state, ExternalBindingState::Disabled);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn different_external_kinds_do_not_conflict(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant = create_tenant(
        &pool,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "t1",
        "Tenant",
    )
    .await;
    let repo = PostgresExternalBindingRepository::new(pool);
    let ctx = tenant_ctx(&tenant.as_uuid().to_string());

    let kinds = ["uuid", "serial", "mac"];
    for (i, kind) in kinds.iter().enumerate() {
        let binding = ok_or_panic(ExternalBinding::auto_match(
            ExternalBindingId::parse_str(&format!("018e0000-0000-0000-0000-{:012}", i + 2))
                .unwrap_or_else(|e| panic!("{e}")),
            tenant,
            ResourceType::Device,
            parse_uuid(&format!("018e0000-0000-0000-0000-{:012}", i + 1)),
            "ABCD-1234",
            *kind,
            &FakeClock::from_millis(1_000_000_000_000 + i as i64),
        ));
        ok_or_panic(repo.create(&binding, &ctx).await);
        let activated = ok_or_panic(repo.activate(binding.id, binding.revision, &ctx).await);
        assert_eq!(activated.state, ExternalBindingState::Active);
    }

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn non_pending_binding_cannot_be_activated(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let tenant = create_tenant(
        &pool,
        parse_uuid("018e1234-5678-7abc-8def-0123456789ab"),
        "t1",
        "Tenant",
    )
    .await;
    let repo = PostgresExternalBindingRepository::new(pool);
    let ctx = tenant_ctx(&tenant.as_uuid().to_string());

    let mut binding = ok_or_panic(ExternalBinding::auto_match(
        parse_binding("018e0000-0000-0000-0000-000000000002"),
        tenant,
        ResourceType::Device,
        parse_uuid("018e0000-0000-0000-0000-000000000001"),
        "upstream-1",
        "serial",
        &FakeClock::from_millis(1_000_000_000_000),
    ));
    binding.disable(&FakeClock::from_millis(1_000_000_000_001), None);
    ok_or_panic(repo.create(&binding, &ctx).await);

    assert!(
        repo.activate(binding.id, binding.revision, &ctx)
            .await
            .is_err()
    );

    Ok(())
}
