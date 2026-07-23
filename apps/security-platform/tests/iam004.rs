use application::api_key::{create_api_key, revoke_api_key, verify_api_key};
use application::mfa::{
    SecretResolver, enroll_totp, require_assurance, use_recovery_code, verify_totp,
};
use domain_identity::assurance::AssuranceLevel;
use domain_identity::totp;
use domain_identity::user::User;
use foundation::{
    Clock, ErrorCode, FakeClock, RequestContext, SystemClock, SystemIdGenerator, SystemRandom,
    TenantId, UserId, UtcTimestamp,
    chrono::{DateTime, Duration},
};
use std::collections::HashMap;
use std::sync::Mutex;
use storage_api::{ApiKeyRepository, UserRepository};
use storage_postgres::{
    api_key_repository::PostgresApiKeyRepository, mfa_repository::PostgresMfaRepository,
    user_repository::PostgresUserRepository,
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

async fn seed_user(pool: sqlx::PgPool) -> (User, PostgresUserRepository) {
    let users = PostgresUserRepository::new(pool);
    let id_gen = SystemIdGenerator::new(SystemClock, SystemRandom);
    let tenant_id = parse_tenant("018e1234-5678-7abc-8def-0123456789ab");
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");
    let mut user = ok_or_panic(User::new(
        ok_or_panic(UserId::generate(&id_gen)),
        tenant_id,
        "apikeyuser",
        "API Key User",
        &SystemClock,
        None,
    ));
    ok_or_panic(user.activate(&SystemClock, None));
    ok_or_panic(users.create(&user, &ctx).await);
    (user, users)
}

fn future<C: Clock>(clock: &C, seconds: i64) -> UtcTimestamp {
    UtcTimestamp::from(
        DateTime::<foundation::chrono::Utc>::from(clock.now()) + Duration::seconds(seconds),
    )
}

struct MemorySecretResolver {
    secrets: Mutex<HashMap<String, Vec<u8>>>,
}

impl MemorySecretResolver {
    fn new() -> Self {
        Self {
            secrets: Mutex::new(HashMap::new()),
        }
    }
}

impl SecretResolver for MemorySecretResolver {
    fn store(&self, ref_name: &str, value: &[u8]) -> Result<(), foundation::PlatformError> {
        let mut map = self.secrets.lock().map_err(|_| {
            foundation::PlatformError::new(ErrorCode::Internal, "secret resolver lock poisoned")
        })?;
        map.insert(ref_name.to_string(), value.to_vec());
        Ok(())
    }

    fn resolve(&self, ref_name: &str) -> Result<Option<Vec<u8>>, foundation::PlatformError> {
        let map = self.secrets.lock().map_err(|_| {
            foundation::PlatformError::new(ErrorCode::Internal, "secret resolver lock poisoned")
        })?;
        Ok(map.get(ref_name).cloned())
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn api_key_round_trip(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let (user, _) = seed_user(pool.clone()).await;
    let repo = PostgresApiKeyRepository::new(pool);
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");

    let created = ok_or_panic(
        create_api_key(
            &repo,
            &SystemRandom,
            &SystemClock,
            &ctx,
            user.id,
            "test-key",
            vec!["read".to_string()],
            vec![],
            future(&SystemClock, 3600),
        )
        .await,
    );

    let verified = ok_or_panic(
        verify_api_key(
            &repo,
            &created.raw_token,
            None,
            "read",
            SystemClock.now(),
            &ctx,
        )
        .await,
    );
    assert_eq!(verified.id, created.api_key.id);
    assert!(verified.last_used_at.is_some());

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn expired_api_key_is_rejected(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let (user, _) = seed_user(pool.clone()).await;
    let repo = PostgresApiKeyRepository::new(pool);
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");
    let clock = FakeClock::from_millis(0);

    let created = ok_or_panic(
        create_api_key(
            &repo,
            &SystemRandom,
            &clock,
            &ctx,
            user.id,
            "expiring-key",
            vec!["read".to_string()],
            vec![],
            future(&clock, 1),
        )
        .await,
    );

    let later = FakeClock::from_millis(5000);
    assert!(
        verify_api_key(&repo, &created.raw_token, None, "read", later.now(), &ctx)
            .await
            .is_err()
    );

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn api_key_scope_and_source_restrictions(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let (user, _) = seed_user(pool.clone()).await;
    let repo = PostgresApiKeyRepository::new(pool);
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");

    let read_key = ok_or_panic(
        create_api_key(
            &repo,
            &SystemRandom,
            &SystemClock,
            &ctx,
            user.id,
            "read-key",
            vec!["read".to_string()],
            vec![],
            future(&SystemClock, 3600),
        )
        .await,
    );
    assert!(
        verify_api_key(
            &repo,
            &read_key.raw_token,
            None,
            "write",
            SystemClock.now(),
            &ctx
        )
        .await
        .is_err()
    );

    let source_key = ok_or_panic(
        create_api_key(
            &repo,
            &SystemRandom,
            &SystemClock,
            &ctx,
            user.id,
            "source-key",
            vec!["read".to_string()],
            vec!["10.0.0.1".to_string()],
            future(&SystemClock, 3600),
        )
        .await,
    );
    assert!(
        verify_api_key(
            &repo,
            &source_key.raw_token,
            Some("10.0.0.2"),
            "read",
            SystemClock.now(),
            &ctx
        )
        .await
        .is_err()
    );
    assert!(
        verify_api_key(
            &repo,
            &source_key.raw_token,
            Some("10.0.0.1"),
            "read",
            SystemClock.now(),
            &ctx
        )
        .await
        .is_ok()
    );

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn api_key_raw_value_is_not_stored(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let (user, _) = seed_user(pool.clone()).await;
    let repo = PostgresApiKeyRepository::new(pool);
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");

    let created = ok_or_panic(
        create_api_key(
            &repo,
            &SystemRandom,
            &SystemClock,
            &ctx,
            user.id,
            "secret-key",
            vec!["read".to_string()],
            vec![],
            future(&SystemClock, 3600),
        )
        .await,
    );

    assert!({
        let found = ok_or_panic(repo.by_token_hash(&created.raw_token, &ctx).await);
        found.is_none()
    });

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn api_key_revocation_blocks_usage(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let (user, _) = seed_user(pool.clone()).await;
    let repo = PostgresApiKeyRepository::new(pool);
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");

    let created = ok_or_panic(
        create_api_key(
            &repo,
            &SystemRandom,
            &SystemClock,
            &ctx,
            user.id,
            "revoke-key",
            vec!["read".to_string()],
            vec![],
            future(&SystemClock, 3600),
        )
        .await,
    );

    ok_or_panic(revoke_api_key(&repo, created.api_key.id, &SystemClock, &ctx).await);
    assert!(
        verify_api_key(
            &repo,
            &created.raw_token,
            None,
            "read",
            SystemClock.now(),
            &ctx
        )
        .await
        .is_err()
    );

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn totp_round_trip_and_replay(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let (user, _) = seed_user(pool.clone()).await;
    let mfa_repo = PostgresMfaRepository::new(pool.clone());
    let resolver = MemorySecretResolver::new();
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");

    let enrolled = ok_or_panic(
        enroll_totp(
            &mfa_repo,
            &resolver,
            &SystemRandom,
            &SystemClock,
            &ctx,
            user.id,
            8,
        )
        .await,
    );

    let code = ok_or_panic(totp::current_code(&enrolled.raw_secret, SystemClock.now()));
    ok_or_panic(verify_totp(&mfa_repo, &resolver, &SystemClock, &ctx, user.id, &code).await);

    let replay = verify_totp(&mfa_repo, &resolver, &SystemClock, &ctx, user.id, &code).await;
    assert!(replay.is_err(), "same TOTP code must not be accepted twice");

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn recovery_code_one_time_use(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let (user, _) = seed_user(pool.clone()).await;
    let mfa_repo = PostgresMfaRepository::new(pool.clone());
    let resolver = MemorySecretResolver::new();
    let ctx = ctx_for("018e1234-5678-7abc-8def-0123456789ab");

    let enrolled = ok_or_panic(
        enroll_totp(
            &mfa_repo,
            &resolver,
            &SystemRandom,
            &SystemClock,
            &ctx,
            user.id,
            2,
        )
        .await,
    );

    let raw_code = enrolled.recovery_codes[0].clone();
    ok_or_panic(use_recovery_code(&mfa_repo, &ctx, user.id, &raw_code).await);
    assert!(
        use_recovery_code(&mfa_repo, &ctx, user.id, &raw_code)
            .await
            .is_err()
    );

    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn assurance_requirements_are_enforced(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let _ = pool;
    assert!(require_assurance(AssuranceLevel::Mfa, AssuranceLevel::Password).is_ok());
    assert!(require_assurance(AssuranceLevel::Password, AssuranceLevel::Mfa).is_err());
    assert!(require_assurance(AssuranceLevel::Hardware, AssuranceLevel::Mfa).is_ok());

    Ok(())
}
