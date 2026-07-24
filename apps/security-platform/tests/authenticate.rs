use application::authenticate::authenticate;
use domain_identity::auth::{AuthenticationPolicy, AuthenticationResult};
use domain_identity::credential::Credential;
use domain_identity::password::Argon2idPasswordHasher;
use domain_identity::user::{User, UserStatus};
use domain_organization::tenant::Tenant;
use foundation::{RequestContext, SystemClock, SystemIdGenerator, SystemRandom, TenantId, UserId};
use std::net::IpAddr;
use std::str::FromStr;
use storage_api::{CredentialRepository, TenantRepository, UserRepository};
use storage_postgres::{
    credential_repository::PostgresCredentialRepository,
    login_attempt_repository::PostgresLoginAttemptRepository,
    tenant_repository::PostgresTenantRepository, user_repository::PostgresUserRepository,
};

fn generator() -> SystemIdGenerator {
    SystemIdGenerator::new(SystemClock, SystemRandom)
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

fn ok_or_panic<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
    match result {
        Ok(v) => v,
        Err(e) => panic!("{e}"),
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn successful_login_round_trip(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let users = PostgresUserRepository::new(pool.clone());
    let credentials = PostgresCredentialRepository::new(pool.clone());
    let tenants = PostgresTenantRepository::new(pool.clone());
    let attempts = PostgresLoginAttemptRepository::new(pool);
    let id_gen = generator();
    let tenant_id = parse_tenant("018e1234-5678-7abc-8def-0123456789ab");
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");
    let hasher = Argon2idPasswordHasher::default();

    let tenant = ok_or_panic(Tenant::new(
        tenant_id,
        "acme",
        "Acme",
        None::<String>,
        None::<String>,
        &SystemClock,
        None,
    ));
    ok_or_panic(tenants.create(&tenant, &ctx).await);

    let mut user = ok_or_panic(User::new(
        ok_or_panic(UserId::generate(&id_gen)),
        tenant_id,
        "mike",
        "Mike",
        &SystemClock,
        None,
    ));
    ok_or_panic(user.activate(&SystemClock, None));
    ok_or_panic(users.create(&user, &ctx).await);

    let hash = ok_or_panic(hasher.hash("secret123"));
    let credential = Credential::new_password(tenant_id, user.id, hash, "argon2id", &SystemClock);
    ok_or_panic(credentials.create(&credential, &ctx).await);

    let ip = IpAddr::from_str("127.0.0.1").ok();
    let result = ok_or_panic(
        authenticate(
            &users,
            &credentials,
            &attempts,
            &tenants,
            &hasher,
            &AuthenticationPolicy::default(),
            &ctx,
            "mike",
            "secret123",
            ip,
        )
        .await,
    );
    assert_eq!(result, AuthenticationResult::Authenticated);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn wrong_password_returns_invalid_credentials(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let users = PostgresUserRepository::new(pool.clone());
    let credentials = PostgresCredentialRepository::new(pool.clone());
    let tenants = PostgresTenantRepository::new(pool.clone());
    let attempts = PostgresLoginAttemptRepository::new(pool);
    let id_gen = generator();
    let tenant_id = parse_tenant("018e1234-5678-7abc-8def-0123456789ab");
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");
    let hasher = Argon2idPasswordHasher::default();

    let tenant = ok_or_panic(Tenant::new(
        tenant_id,
        "acme",
        "Acme",
        None::<String>,
        None::<String>,
        &SystemClock,
        None,
    ));
    ok_or_panic(tenants.create(&tenant, &ctx).await);

    let mut user = ok_or_panic(User::new(
        ok_or_panic(UserId::generate(&id_gen)),
        tenant_id,
        "mallory",
        "Mallory",
        &SystemClock,
        None,
    ));
    ok_or_panic(user.activate(&SystemClock, None));
    ok_or_panic(users.create(&user, &ctx).await);

    let hash = ok_or_panic(hasher.hash("secret123"));
    let credential = Credential::new_password(tenant_id, user.id, hash, "argon2id", &SystemClock);
    ok_or_panic(credentials.create(&credential, &ctx).await);

    let ip = IpAddr::from_str("10.0.0.1").ok();
    let result = ok_or_panic(
        authenticate(
            &users,
            &credentials,
            &attempts,
            &tenants,
            &hasher,
            &AuthenticationPolicy::default(),
            &ctx,
            "mallory",
            "wrong",
            ip,
        )
        .await,
    );
    assert_eq!(result, AuthenticationResult::InvalidCredentials);

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn repeated_failures_lock_account(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let users = PostgresUserRepository::new(pool.clone());
    let credentials = PostgresCredentialRepository::new(pool.clone());
    let tenants = PostgresTenantRepository::new(pool.clone());
    let attempts = PostgresLoginAttemptRepository::new(pool);
    let id_gen = generator();
    let tenant_id = parse_tenant("018e1234-5678-7abc-8def-0123456789ab");
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");
    let hasher = Argon2idPasswordHasher::default();
    let policy = AuthenticationPolicy {
        max_attempts_per_identity: 2,
        max_attempts_per_source: 20,
        window_seconds: 900,
    };

    let tenant = ok_or_panic(Tenant::new(
        tenant_id,
        "acme",
        "Acme",
        None::<String>,
        None::<String>,
        &SystemClock,
        None,
    ));
    ok_or_panic(tenants.create(&tenant, &ctx).await);

    let mut user = ok_or_panic(User::new(
        ok_or_panic(UserId::generate(&id_gen)),
        tenant_id,
        "victim",
        "Victim",
        &SystemClock,
        None,
    ));
    ok_or_panic(user.activate(&SystemClock, None));
    ok_or_panic(users.create(&user, &ctx).await);

    let hash = ok_or_panic(hasher.hash("secret123"));
    let credential = Credential::new_password(tenant_id, user.id, hash, "argon2id", &SystemClock);
    ok_or_panic(credentials.create(&credential, &ctx).await);

    let ip = IpAddr::from_str("192.168.0.1").ok();
    for _ in 0..3 {
        let _ = authenticate(
            &users,
            &credentials,
            &attempts,
            &tenants,
            &hasher,
            &policy,
            &ctx,
            "victim",
            "wrong",
            ip,
        )
        .await;
    }

    let locked = ok_or_panic(users.by_username("victim", &ctx).await);
    assert_eq!(locked.status, UserStatus::Locked);

    Ok(())
}
