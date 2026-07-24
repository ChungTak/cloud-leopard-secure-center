use foundation::chrono::{DateTime, Utc};
use foundation::retry::RetryPolicy;
use foundation::uuid::Uuid;
use foundation::{
    PlatformError, RandomSource, RequestContext, SystemClock, SystemRandom, TenantId, UtcTimestamp,
};
use storage_api::{InboxMessage, InboxRepository, InboxStatus, Job, JobRepository, JobStatus};
use storage_postgres::inbox_repository::PostgresInboxRepository;
use storage_postgres::job_repository::PostgresJobRepository;

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

fn ctx() -> RequestContext {
    RequestContext {
        tenant_id: Some(tenant()),
        ..Default::default()
    }
}

fn epoch() -> UtcTimestamp {
    UtcTimestamp::from(DateTime::<Utc>::UNIX_EPOCH)
}

fn future_hours(hours: i64) -> UtcTimestamp {
    let dt: DateTime<Utc> = epoch().into();
    UtcTimestamp::from(dt + foundation::chrono::Duration::hours(hours))
}

fn message_id(seed: u8) -> Uuid {
    let mut bytes = [0u8; 16];
    bytes[15] = seed;
    Uuid::from_bytes(bytes)
}

#[derive(Debug)]
struct ZeroRandom;

impl RandomSource for ZeroRandom {
    fn fill_bytes(&self, buf: &mut [u8]) -> Result<(), PlatformError> {
        for b in buf.iter_mut() {
            *b = 0;
        }
        Ok(())
    }
}

fn sleep(ms: u64) {
    std::thread::sleep(std::time::Duration::from_millis(ms));
}

#[sqlx::test(migrations = "../../migrations")]
async fn inbox_dedup_by_consumer_message(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresInboxRepository::new(pool);
    let msg = InboxMessage {
        message_id: message_id(1),
        tenant_id: Some(tenant()),
        consumer_id: "consumer-a".to_string(),
        status: InboxStatus::Pending,
        result_digest: None,
        attempts: 0,
        expires_at: future_hours(1),
    };

    let first = ok_or_panic(repo.receive(&msg, &ctx()).await);
    let second = ok_or_panic(repo.receive(&msg, &ctx()).await);

    assert_eq!(first.message_id, second.message_id);
    assert_eq!(first.consumer_id, second.consumer_id);
    assert_eq!(first.status, InboxStatus::Pending);
    assert_eq!(first.attempts, second.attempts);
    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn inbox_complete_persists_first_digest(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresInboxRepository::new(pool);
    let msg = InboxMessage {
        message_id: message_id(2),
        tenant_id: Some(tenant()),
        consumer_id: "consumer-a".to_string(),
        status: InboxStatus::Pending,
        result_digest: None,
        attempts: 0,
        expires_at: future_hours(1),
    };

    ok_or_panic(repo.receive(&msg, &ctx()).await);
    let completed = ok_or_panic(
        repo.complete("consumer-a", message_id(2), "digest-a", &ctx())
            .await,
    );
    assert_eq!(completed.result_digest.as_deref(), Some("digest-a"));

    let again = ok_or_panic(
        repo.complete("consumer-a", message_id(2), "digest-b", &ctx())
            .await,
    );
    assert_eq!(again.result_digest.as_deref(), Some("digest-a"));
    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn inbox_consumer_restart_returns_result(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresInboxRepository::new(pool);
    let msg = InboxMessage {
        message_id: message_id(3),
        tenant_id: Some(tenant()),
        consumer_id: "consumer-a".to_string(),
        status: InboxStatus::Pending,
        result_digest: None,
        attempts: 0,
        expires_at: future_hours(1),
    };

    ok_or_panic(repo.receive(&msg, &ctx()).await);
    ok_or_panic(
        repo.complete("consumer-a", message_id(3), "result-digest", &ctx())
            .await,
    );

    let replay = InboxMessage {
        message_id: message_id(3),
        tenant_id: Some(tenant()),
        consumer_id: "consumer-a".to_string(),
        status: InboxStatus::Pending,
        result_digest: None,
        attempts: 99,
        expires_at: future_hours(1),
    };
    let redelivered = ok_or_panic(repo.receive(&replay, &ctx()).await);

    assert_eq!(redelivered.status, InboxStatus::Completed);
    assert_eq!(redelivered.result_digest.as_deref(), Some("result-digest"));
    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn inbox_retention_covers_replay_window(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresInboxRepository::new(pool);
    let msg = InboxMessage {
        message_id: message_id(4),
        tenant_id: Some(tenant()),
        consumer_id: "consumer-a".to_string(),
        status: InboxStatus::Pending,
        result_digest: None,
        attempts: 0,
        expires_at: future_hours(24),
    };

    let received = ok_or_panic(repo.receive(&msg, &ctx()).await);
    let window_end: DateTime<Utc> = received.expires_at.into();
    let created: DateTime<Utc> = epoch().into();
    assert!(window_end - created >= foundation::chrono::Duration::hours(23));
    Ok(())
}

fn job(queue: &str, payload: &str) -> Job {
    Job {
        job_id: None,
        tenant_id: Some(tenant()),
        queue: queue.to_string(),
        payload: payload.to_string(),
        status: JobStatus::Pending,
        revision: 1,
        lease_owner: None,
        lease_until: None,
        attempts: 0,
        max_attempts: 5,
        next_run: epoch(),
        deadline: None,
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn job_happy_path(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresJobRepository::new(pool, SystemClock, SystemRandom);
    ok_or_panic(repo.schedule(&job("q1", "{}"), &ctx()).await);

    let claimed = some_or_panic(
        ok_or_panic(
            repo.claim("q1", "worker-1", std::time::Duration::from_secs(10), &ctx())
                .await,
        ),
        "job should be claimable",
    );
    assert_eq!(claimed.status, JobStatus::Running);
    assert_eq!(claimed.attempts, 1);
    assert_eq!(claimed.lease_owner.as_deref(), Some("worker-1"));

    let completed = ok_or_panic(
        repo.complete(
            some_or_panic(claimed.job_id, "job id missing"),
            "worker-1",
            claimed.revision,
            &ctx(),
        )
        .await,
    );
    assert_eq!(completed.status, JobStatus::Completed);

    let empty = ok_or_panic(
        repo.claim("q1", "worker-2", std::time::Duration::from_secs(1), &ctx())
            .await,
    );
    assert!(empty.is_none());
    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn job_lease_expires_and_reclaimed(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresJobRepository::new(pool, SystemClock, SystemRandom);
    ok_or_panic(repo.schedule(&job("q2", "{}"), &ctx()).await);

    let first = some_or_panic(
        ok_or_panic(
            repo.claim(
                "q2",
                "worker-1",
                std::time::Duration::from_millis(20),
                &ctx(),
            )
            .await,
        ),
        "first claim failed",
    );

    sleep(40);

    let second = some_or_panic(
        ok_or_panic(
            repo.claim("q2", "worker-2", std::time::Duration::from_secs(10), &ctx())
                .await,
        ),
        "second claim failed",
    );
    assert_eq!(second.lease_owner.as_deref(), Some("worker-2"));
    assert_eq!(second.attempts, 2);

    let first_id = some_or_panic(first.job_id, "job id missing");
    let result = repo
        .complete(first_id, "worker-1", first.revision, &ctx())
        .await;
    assert!(result.is_err(), "old lease complete should fail");

    let second_id = some_or_panic(second.job_id, "job id missing");
    let completed = ok_or_panic(
        repo.complete(second_id, "worker-2", second.revision, &ctx())
            .await,
    );
    assert_eq!(completed.status, JobStatus::Completed);
    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn job_dual_worker_complete_once(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresJobRepository::new(pool, SystemClock, SystemRandom);
    ok_or_panic(repo.schedule(&job("q3", "{}"), &ctx()).await);

    let first = some_or_panic(
        ok_or_panic(
            repo.claim(
                "q3",
                "worker-a",
                std::time::Duration::from_millis(10),
                &ctx(),
            )
            .await,
        ),
        "first claim failed",
    );

    sleep(20);

    let second = some_or_panic(
        ok_or_panic(
            repo.claim("q3", "worker-b", std::time::Duration::from_secs(10), &ctx())
                .await,
        ),
        "second claim failed",
    );
    assert_eq!(second.lease_owner.as_deref(), Some("worker-b"));

    let first_id = some_or_panic(first.job_id, "job id missing");
    let second_id = some_or_panic(second.job_id, "job id missing");

    let first_result = repo
        .complete(first_id, "worker-a", first.revision, &ctx())
        .await;
    let second_result = repo
        .complete(second_id, "worker-b", second.revision, &ctx())
        .await;

    assert!(
        first_result.is_err(),
        "first worker should lose after lease expiry"
    );
    assert!(second_result.is_ok(), "second worker should complete");
    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn job_transient_retry_succeeds(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresJobRepository::new(pool, SystemClock, SystemRandom);
    ok_or_panic(repo.schedule(&job("q4", "{}"), &ctx()).await);

    let first = some_or_panic(
        ok_or_panic(
            repo.claim("q4", "worker-1", std::time::Duration::from_secs(10), &ctx())
                .await,
        ),
        "first claim failed",
    );

    let policy = RetryPolicy::exponential(1, 100, 0, Some(5), None);
    let next_run = some_or_panic(
        policy.next_retry(epoch(), first.attempts as u32, &ZeroRandom),
        "retry should be scheduled",
    );

    let failed = ok_or_panic(
        repo.fail(
            some_or_panic(first.job_id, "job id missing"),
            "worker-1",
            first.revision,
            Some(next_run),
            &ctx(),
        )
        .await,
    );
    assert_eq!(failed.status, JobStatus::Pending);

    let retried = some_or_panic(
        ok_or_panic(
            repo.claim("q4", "worker-1", std::time::Duration::from_secs(10), &ctx())
                .await,
        ),
        "retry claim failed",
    );
    assert_eq!(retried.attempts, 2);

    let completed = ok_or_panic(
        repo.complete(
            some_or_panic(retried.job_id, "job id missing"),
            "worker-1",
            retried.revision,
            &ctx(),
        )
        .await,
    );
    assert_eq!(completed.status, JobStatus::Completed);
    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn job_poison_after_max_attempts(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresJobRepository::new(pool, SystemClock, SystemRandom);
    let mut template = job("q5", "{}");
    template.max_attempts = 2;
    ok_or_panic(repo.schedule(&template, &ctx()).await);

    let first = some_or_panic(
        ok_or_panic(
            repo.claim("q5", "worker-1", std::time::Duration::from_secs(10), &ctx())
                .await,
        ),
        "first claim failed",
    );
    let policy = RetryPolicy::exponential(1, 100, 0, Some(2), None);
    let next_run = some_or_panic(
        policy.next_retry(epoch(), first.attempts as u32, &ZeroRandom),
        "retry should be scheduled",
    );

    let failed = ok_or_panic(
        repo.fail(
            some_or_panic(first.job_id, "job id missing"),
            "worker-1",
            first.revision,
            Some(next_run),
            &ctx(),
        )
        .await,
    );
    assert_eq!(failed.status, JobStatus::Pending);

    let second = some_or_panic(
        ok_or_panic(
            repo.claim("q5", "worker-1", std::time::Duration::from_secs(10), &ctx())
                .await,
        ),
        "second claim failed",
    );
    assert_eq!(second.attempts, 2);

    let poisoned = ok_or_panic(
        repo.fail(
            some_or_panic(second.job_id, "job id missing"),
            "worker-1",
            second.revision,
            None,
            &ctx(),
        )
        .await,
    );
    assert_eq!(poisoned.status, JobStatus::Failed);

    let empty = ok_or_panic(
        repo.claim("q5", "worker-1", std::time::Duration::from_secs(1), &ctx())
            .await,
    );
    assert!(empty.is_none());
    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn job_cancel(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresJobRepository::new(pool, SystemClock, SystemRandom);
    let scheduled = ok_or_panic(repo.schedule(&job("q6", "{}"), &ctx()).await);
    let job_id = some_or_panic(scheduled.job_id, "job id missing");

    let cancelled = ok_or_panic(repo.cancel(job_id, &ctx()).await);
    assert_eq!(cancelled.status, JobStatus::Cancelled);

    let empty = ok_or_panic(
        repo.claim("q6", "worker-1", std::time::Duration::from_secs(1), &ctx())
            .await,
    );
    assert!(empty.is_none());
    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn job_retry_deadline_exceeded(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresJobRepository::new(pool, SystemClock, SystemRandom);
    ok_or_panic(repo.schedule(&job("q7", "{}"), &ctx()).await);

    let first = some_or_panic(
        ok_or_panic(
            repo.claim("q7", "worker-1", std::time::Duration::from_secs(10), &ctx())
                .await,
        ),
        "first claim failed",
    );

    let policy = RetryPolicy::exponential(10, 100, 0, None, Some(epoch()));
    assert!(
        policy
            .next_retry(epoch(), first.attempts as u32, &ZeroRandom)
            .is_none(),
        "retry should exceed deadline"
    );

    let failed = ok_or_panic(
        repo.fail(
            some_or_panic(first.job_id, "job id missing"),
            "worker-1",
            first.revision,
            None,
            &ctx(),
        )
        .await,
    );
    assert_eq!(failed.status, JobStatus::Failed);
    Ok(())
}

#[sqlx::test(migrations = "../../migrations")]
async fn job_retry_backoff_with_clock_advance(pool: sqlx::PgPool) -> sqlx::Result<()> {
    let repo = PostgresJobRepository::new(pool, SystemClock, SystemRandom);
    ok_or_panic(repo.schedule(&job("q8", "{}"), &ctx()).await);

    let first = some_or_panic(
        ok_or_panic(
            repo.claim("q8", "worker-1", std::time::Duration::from_secs(10), &ctx())
                .await,
        ),
        "first claim failed",
    );

    let policy = RetryPolicy::exponential(1, 100, 0, None, None);
    let first_retry = some_or_panic(
        policy.next_retry(epoch(), first.attempts as u32, &ZeroRandom),
        "first retry should be scheduled",
    );

    let failed = ok_or_panic(
        repo.fail(
            some_or_panic(first.job_id, "job id missing"),
            "worker-1",
            first.revision,
            Some(first_retry),
            &ctx(),
        )
        .await,
    );
    assert_eq!(failed.status, JobStatus::Pending);

    let second = some_or_panic(
        ok_or_panic(
            repo.claim("q8", "worker-1", std::time::Duration::from_secs(10), &ctx())
                .await,
        ),
        "second claim failed",
    );
    assert_eq!(second.attempts, 2);

    let second_retry = some_or_panic(
        policy.next_retry(epoch(), second.attempts as u32, &ZeroRandom),
        "second retry should be scheduled",
    );
    assert!(second_retry > first_retry, "backoff should increase");

    let second_failed = ok_or_panic(
        repo.fail(
            some_or_panic(second.job_id, "job id missing"),
            "worker-1",
            second.revision,
            Some(second_retry),
            &ctx(),
        )
        .await,
    );
    assert_eq!(second_failed.status, JobStatus::Pending);

    sleep(5);
    let third = some_or_panic(
        ok_or_panic(
            repo.claim("q8", "worker-1", std::time::Duration::from_secs(10), &ctx())
                .await,
        ),
        "third claim should be available after next_run",
    );
    assert_eq!(third.attempts, 3);

    ok_or_panic(
        repo.complete(
            some_or_panic(third.job_id, "job id missing"),
            "worker-1",
            third.revision,
            &ctx(),
        )
        .await,
    );
    Ok(())
}
