use domain_identity::user::User;
use foundation::{
    RequestContext, Revision, SystemClock, SystemIdGenerator, SystemRandom, TenantId, UserId,
};
use storage_api::UserRepository;
use storage_postgres::user_repository::PostgresUserRepository;

fn generator() -> SystemIdGenerator {
    SystemIdGenerator::new(SystemClock, SystemRandom)
}

fn ctx_for(tenant: &str) -> RequestContext {
    RequestContext {
        tenant_id: Some(parse_tenant(tenant)),
        ..Default::default()
    }
}

fn parse_tenant(s: &str) -> TenantId {
    match TenantId::parse_str(s) {
        Ok(v) => v,
        Err(e) => panic!("{e}"),
    }
}

fn tenant_id() -> TenantId {
    parse_tenant("018e1234-5678-7abc-8def-0123456789ab")
}

fn ok_or_panic<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
    match result {
        Ok(v) => v,
        Err(e) => panic!("{e}"),
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn create_and_read_round_trip(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresUserRepository::new(pool);
    let id_gen = generator();
    let user = ok_or_panic(User::new(
        UserId::generate(&id_gen),
        tenant_id(),
        "alice",
        "Alice Example",
        &SystemClock,
        None,
    ));

    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");
    ok_or_panic(repo.create(&user, &ctx).await);

    let loaded = ok_or_panic(repo.by_id(user.id, &ctx).await);
    assert_eq!(loaded.username, "alice");
    assert_eq!(loaded.display_name, "Alice Example");

    let by_name = ok_or_panic(repo.by_username("ALICE", &ctx).await);
    assert_eq!(by_name.id, user.id);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn duplicate_username_in_same_tenant_fails(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresUserRepository::new(pool);
    let id_gen = generator();
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");

    let user = ok_or_panic(User::new(
        UserId::generate(&id_gen),
        tenant_id(),
        "bob",
        "Bob",
        &SystemClock,
        None,
    ));
    ok_or_panic(repo.create(&user, &ctx).await);

    let duplicate = ok_or_panic(User::new(
        UserId::generate(&id_gen),
        tenant_id(),
        "bob",
        "Another Bob",
        &SystemClock,
        None,
    ));
    let result = repo.create(&duplicate, &ctx).await;
    assert!(result.is_err());

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn same_username_in_different_tenants_is_allowed(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresUserRepository::new(pool);
    let id_gen = generator();

    let tenant_a = ctx_for("018e1234-5678-7abc-8def-0123456789ab");
    let tenant_b = ctx_for("018e1234-5678-7abc-8def-0123456789ac");

    let user_a = ok_or_panic(User::new(
        UserId::generate(&id_gen),
        tenant_id(),
        "charlie",
        "Charlie A",
        &SystemClock,
        None,
    ));
    ok_or_panic(repo.create(&user_a, &tenant_a).await);

    let user_b = ok_or_panic(User::new(
        UserId::generate(&id_gen),
        parse_tenant("018e1234-5678-7abc-8def-0123456789ac"),
        "charlie",
        "Charlie B",
        &SystemClock,
        None,
    ));
    ok_or_panic(repo.create(&user_b, &tenant_b).await);

    let loaded_b = ok_or_panic(repo.by_id(user_b.id, &tenant_b).await);
    assert_eq!(loaded_b.username, "charlie");

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn soft_delete_allows_recreate_with_same_username(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresUserRepository::new(pool);
    let id_gen = generator();
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");

    let user = ok_or_panic(User::new(
        UserId::generate(&id_gen),
        tenant_id(),
        "dave",
        "Dave",
        &SystemClock,
        None,
    ));
    ok_or_panic(repo.create(&user, &ctx).await);

    ok_or_panic(repo.delete(user.id, Revision::initial(), &ctx).await);

    let replacement = ok_or_panic(User::new(
        UserId::generate(&id_gen),
        tenant_id(),
        "dave",
        "Dave 2",
        &SystemClock,
        None,
    ));
    ok_or_panic(repo.create(&replacement, &ctx).await);

    let loaded = ok_or_panic(repo.by_username("dave", &ctx).await);
    assert_eq!(loaded.id, replacement.id);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn stale_revision_update_fails(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresUserRepository::new(pool);
    let id_gen = generator();
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");

    let mut user = ok_or_panic(User::new(
        UserId::generate(&id_gen),
        tenant_id(),
        "eve",
        "Eve",
        &SystemClock,
        None,
    ));
    ok_or_panic(repo.create(&user, &ctx).await);

    ok_or_panic(user.activate(&SystemClock, None));
    let result = repo.update(&user, Revision::new(99), &ctx).await;
    assert!(result.is_err());

    Ok(())
}
