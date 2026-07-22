use application::session::{change_password, disable_user, issue_token_pair, refresh_token_pair};
use application::token_service::TokenService;
use domain_identity::password::Argon2idPasswordHasher;
use domain_identity::user::User;
use foundation::{
    Clock, FakeClock, RequestContext, SystemClock, SystemIdGenerator, SystemRandom, TenantId,
    UserId, UtcTimestamp,
    chrono::{DateTime, Duration},
};
use storage_api::{CredentialRepository, UserRepository};
use storage_postgres::{
    credential_repository::PostgresCredentialRepository,
    session_repository::PostgresSessionRepository, user_repository::PostgresUserRepository,
};

fn parse_tenant(s: &str) -> TenantId {
    match TenantId::parse_str(s) {
        Ok(v) => v,
        Err(e) => panic!("{e}"),
    }
}

fn ok_or_panic<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
    match result {
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

async fn seed_user(
    pool: sqlx::PgPool,
) -> (PostgresUserRepository, PostgresCredentialRepository, User) {
    let users = PostgresUserRepository::new(pool.clone());
    let credentials = PostgresCredentialRepository::new(pool);
    let id_gen = SystemIdGenerator::new(SystemClock, SystemRandom);
    let tenant_id = parse_tenant("018e1234-5678-7abc-8def-0123456789ab");
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");
    let mut user = ok_or_panic(User::new(
        UserId::generate(&id_gen),
        tenant_id,
        "tokenuser",
        "Token User",
        &SystemClock,
        None,
    ));
    ok_or_panic(user.activate(&SystemClock, None));
    ok_or_panic(users.create(&user, &ctx).await);

    let hasher = Argon2idPasswordHasher::default();
    let hash = ok_or_panic(hasher.hash("secret123"));
    let credential = domain_identity::credential::Credential::new_password(
        tenant_id,
        user.id,
        hash,
        "argon2id",
        &SystemClock,
    );
    ok_or_panic(credentials.create(&credential, &ctx).await);
    (users, credentials, user)
}

fn token_service() -> TokenService {
    ok_or_panic(TokenService::new(
        b"a-very-secret-key-of-at-least-32-bytes",
        "clsc",
        "clsc-api",
        3600,
    ))
}

fn refresh_ttl<C: Clock>(clock: &C) -> UtcTimestamp {
    UtcTimestamp::from(
        DateTime::<foundation::chrono::Utc>::from(clock.now()) + Duration::seconds(86400),
    )
}

#[sqlx::test(migrations = "../../migrations")]
async fn access_token_round_trip(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let (_, _, user) = seed_user(pool.clone()).await;
    let sessions = PostgresSessionRepository::new(pool);
    let service = token_service();
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");

    let pair = ok_or_panic(
        issue_token_pair(
            &sessions,
            &service,
            &SystemRandom,
            &SystemClock,
            &ctx,
            &user,
            refresh_ttl(&SystemClock),
        )
        .await,
    );

    let claims = ok_or_panic(service.verify_access_token(
        &pair.access_token,
        SystemClock.now(),
        user.session_version,
    ));
    assert_eq!(claims.sub, user.id);
    assert_eq!(claims.tenant_id, user.tenant_id);
    assert_eq!(claims.session_version, user.session_version);
    assert_eq!(claims.iss, "clsc");
    assert_eq!(claims.aud, "clsc-api");

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn expired_access_token_is_rejected(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let (_, _, user) = seed_user(pool.clone()).await;
    let sessions = PostgresSessionRepository::new(pool);
    let service = ok_or_panic(TokenService::new(
        b"a-very-secret-key-of-at-least-32-bytes",
        "clsc",
        "clsc-api",
        1,
    ));
    let issue_clock = FakeClock::from_millis(0);
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");

    let pair = ok_or_panic(
        issue_token_pair(
            &sessions,
            &service,
            &SystemRandom,
            &issue_clock,
            &ctx,
            &user,
            refresh_ttl(&issue_clock),
        )
        .await,
    );

    let verify_clock = FakeClock::from_millis(5000);
    assert!(
        service
            .verify_access_token(&pair.access_token, verify_clock.now(), user.session_version)
            .is_err()
    );

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn wrong_issuer_or_audience_is_rejected(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let (_, _, user) = seed_user(pool.clone()).await;
    let sessions = PostgresSessionRepository::new(pool);
    let service = token_service();
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");

    let pair = ok_or_panic(
        issue_token_pair(
            &sessions,
            &service,
            &SystemRandom,
            &SystemClock,
            &ctx,
            &user,
            refresh_ttl(&SystemClock),
        )
        .await,
    );

    let other = ok_or_panic(TokenService::new(
        b"a-very-secret-key-of-at-least-32-bytes",
        "other",
        "clsc-api",
        3600,
    ));
    assert!(
        other
            .verify_access_token(&pair.access_token, SystemClock.now(), user.session_version)
            .is_err()
    );

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn old_session_version_is_rejected_after_logout(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let (users, _, user) = seed_user(pool.clone()).await;
    let sessions = PostgresSessionRepository::new(pool);
    let service = token_service();
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");

    let pair = ok_or_panic(
        issue_token_pair(
            &sessions,
            &service,
            &SystemRandom,
            &SystemClock,
            &ctx,
            &user,
            refresh_ttl(&SystemClock),
        )
        .await,
    );

    ok_or_panic(application::session::logout(&users, &sessions, &SystemClock, &ctx, user.id).await);

    let reloaded = ok_or_panic(users.by_id(user.id, &ctx).await);
    assert!(
        service
            .verify_access_token(
                &pair.access_token,
                SystemClock.now(),
                reloaded.session_version
            )
            .is_err()
    );

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn refresh_token_replay_revokes_family(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let (users, _, user) = seed_user(pool.clone()).await;
    let sessions = PostgresSessionRepository::new(pool);
    let service = token_service();
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");

    let pair = ok_or_panic(
        issue_token_pair(
            &sessions,
            &service,
            &SystemRandom,
            &SystemClock,
            &ctx,
            &user,
            refresh_ttl(&SystemClock),
        )
        .await,
    );

    let refreshed = ok_or_panic(
        refresh_token_pair(
            &users,
            &sessions,
            &service,
            &SystemRandom,
            &SystemClock,
            &ctx,
            &pair.refresh_token,
            refresh_ttl(&SystemClock),
        )
        .await,
    );

    let replay = refresh_token_pair(
        &users,
        &sessions,
        &service,
        &SystemRandom,
        &SystemClock,
        &ctx,
        &pair.refresh_token,
        refresh_ttl(&SystemClock),
    )
    .await;
    assert!(replay.is_err());

    let replay_new = refresh_token_pair(
        &users,
        &sessions,
        &service,
        &SystemRandom,
        &SystemClock,
        &ctx,
        &refreshed.refresh_token,
        refresh_ttl(&SystemClock),
    )
    .await;
    assert!(replay_new.is_err());

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn concurrent_refresh_only_one_succeeds(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let (users, _, user) = seed_user(pool.clone()).await;
    let sessions = PostgresSessionRepository::new(pool);
    let service = token_service();
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");

    let pair = ok_or_panic(
        issue_token_pair(
            &sessions,
            &service,
            &SystemRandom,
            &SystemClock,
            &ctx,
            &user,
            refresh_ttl(&SystemClock),
        )
        .await,
    );

    let users1 = &users;
    let users2 = &users;
    let sessions1 = &sessions;
    let sessions2 = &sessions;
    let service1 = &service;
    let service2 = &service;
    let ctx1 = &ctx;
    let ctx2 = &ctx;
    let rt = refresh_ttl(&SystemClock);
    let raw = &pair.refresh_token;

    let (r1, r2) = tokio::join!(
        async move {
            refresh_token_pair(
                users1,
                sessions1,
                service1,
                &SystemRandom,
                &SystemClock,
                ctx1,
                raw,
                rt,
            )
            .await
        },
        async move {
            refresh_token_pair(
                users2,
                sessions2,
                service2,
                &SystemRandom,
                &SystemClock,
                ctx2,
                raw,
                rt,
            )
            .await
        },
    );

    let success = r1.is_ok() as usize + r2.is_ok() as usize;
    assert_eq!(success, 1, "concurrent refresh must allow only one success");

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn password_change_increments_session_version(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let (users, credentials, user) = seed_user(pool.clone()).await;
    let sessions = PostgresSessionRepository::new(pool);
    let service = token_service();
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");
    let hasher = Argon2idPasswordHasher::default();

    let pair = ok_or_panic(
        issue_token_pair(
            &sessions,
            &service,
            &SystemRandom,
            &SystemClock,
            &ctx,
            &user,
            refresh_ttl(&SystemClock),
        )
        .await,
    );

    ok_or_panic(
        change_password(
            &users,
            &credentials,
            &sessions,
            &hasher,
            &SystemClock,
            &ctx,
            user.id,
            "secret123",
            "new-secret-456",
        )
        .await,
    );

    let reloaded = ok_or_panic(users.by_id(user.id, &ctx).await);
    assert!(
        service
            .verify_access_token(
                &pair.access_token,
                SystemClock.now(),
                reloaded.session_version
            )
            .is_err()
    );

    let credential = ok_or_panic(
        credentials
            .by_user_and_type(user.id, "password_hash", &ctx)
            .await,
    );
    assert!(
        hasher
            .verify("new-secret-456", &credential.value)
            .unwrap_or(false)
    );
    assert!(
        !hasher
            .verify("secret123", &credential.value)
            .unwrap_or(true)
    );

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn disabling_user_increments_session_version(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let (users, _, user) = seed_user(pool.clone()).await;
    let sessions = PostgresSessionRepository::new(pool);
    let service = token_service();
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");

    let pair = ok_or_panic(
        issue_token_pair(
            &sessions,
            &service,
            &SystemRandom,
            &SystemClock,
            &ctx,
            &user,
            refresh_ttl(&SystemClock),
        )
        .await,
    );

    ok_or_panic(disable_user(&users, &sessions, &SystemClock, &ctx, user.id).await);

    let reloaded = ok_or_panic(users.by_id(user.id, &ctx).await);
    assert!(
        service
            .verify_access_token(
                &pair.access_token,
                SystemClock.now(),
                reloaded.session_version
            )
            .is_err()
    );
    assert_eq!(reloaded.status, domain_identity::user::UserStatus::Disabled);

    Ok(())
}
