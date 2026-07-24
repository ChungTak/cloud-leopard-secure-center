//! PostgreSQL implementation of the `JobRepository` port.

use async_trait::async_trait;
use foundation::chrono::{DateTime, Utc};
use foundation::uuid::Uuid;
use foundation::{
    Clock, PlatformError, RandomSource, RequestContext, TenantId, UtcTimestamp, generate_uuid,
};
use sqlx::{PgPool, Row};
use storage_api::{Job, JobRepository, JobStatus};

use crate::begin_tenant_transaction;

/// PostgreSQL-backed job repository.
#[derive(Clone)]
pub struct PostgresJobRepository {
    pool: PgPool,
    clock: std::sync::Arc<dyn Clock>,
    random: std::sync::Arc<dyn RandomSource>,
}

impl std::fmt::Debug for PostgresJobRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PostgresJobRepository")
            .field("pool", &self.pool)
            .finish()
    }
}

impl PostgresJobRepository {
    /// Create a new repository backed by `pool`.
    pub fn new(
        pool: PgPool,
        clock: impl Clock + 'static,
        random: impl RandomSource + 'static,
    ) -> Self {
        Self {
            pool,
            clock: std::sync::Arc::new(clock),
            random: std::sync::Arc::new(random),
        }
    }
}

#[async_trait]
impl JobRepository for PostgresJobRepository {
    async fn schedule(&self, job: &Job, ctx: &RequestContext) -> Result<Job, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let tenant_uuid = job.tenant_id.map(|id| *id.as_uuid());
        let job_id = match job.job_id {
            Some(id) => id,
            None => generate_uuid(&*self.clock, &*self.random)?,
        };
        let next_run = utc_to_db(job.next_run);
        let deadline = job.deadline.map(utc_to_db);
        let lease = job.lease_until.map(utc_to_db);

        let row = sqlx::query(
            "INSERT INTO infra.jobs
             (job_id, tenant_id, queue, payload, status, revision, lease_owner, lease_until,
              attempts, max_attempts, next_run, deadline)
             VALUES ($1, $2, $3, $4::jsonb, $5, $6, $7, $8, $9, $10, $11, $12)
             RETURNING job_id, tenant_id, queue, payload, status, revision,
                       lease_owner, lease_until, attempts, max_attempts, next_run, deadline",
        )
        .bind(job_id)
        .bind(tenant_uuid)
        .bind(&job.queue)
        .bind(&job.payload)
        .bind(status_to_db(job.status))
        .bind(job.revision)
        .bind(job.lease_owner.as_ref())
        .bind(lease)
        .bind(job.attempts)
        .bind(job.max_attempts)
        .bind(next_run)
        .bind(deadline)
        .fetch_one(&mut *tx)
        .await
        .map_err(crate::db_error)?;

        let result = row_to_job(row)?;
        drop(tx);
        tx_managed.commit().await.map_err(crate::db_error)?;
        Ok(result)
    }

    async fn claim(
        &self,
        queue: &str,
        worker_id: &str,
        lease: std::time::Duration,
        ctx: &RequestContext,
    ) -> Result<Option<Job>, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let tenant_uuid = ctx.tenant_id.map(|id| *id.as_uuid());
        let lease_seconds = lease.as_secs_f64();

        let row = sqlx::query(
            "WITH next AS (
                SELECT job_id
                FROM infra.jobs
                WHERE tenant_id IS NOT DISTINCT FROM $1
                  AND queue = $2
                  AND status IN ('pending', 'running')
                  AND attempts < max_attempts
                  AND next_run <= clock_timestamp()
                  AND (lease_until IS NULL OR lease_until <= clock_timestamp())
                ORDER BY next_run, created_at
                FOR UPDATE SKIP LOCKED
                LIMIT 1
             )
             UPDATE infra.jobs
             SET status = 'running',
                 lease_owner = $3,
                 lease_until = clock_timestamp() + $4 * interval '1 second',
                 attempts = attempts + 1,
                 revision = revision + 1,
                 next_run = clock_timestamp(),
                 updated_at = clock_timestamp()
             FROM next
             WHERE infra.jobs.job_id = next.job_id
             RETURNING infra.jobs.job_id, infra.jobs.tenant_id, infra.jobs.queue,
                       infra.jobs.payload, infra.jobs.status, infra.jobs.revision,
                       infra.jobs.lease_owner, infra.jobs.lease_until, infra.jobs.attempts,
                       infra.jobs.max_attempts, infra.jobs.next_run, infra.jobs.deadline",
        )
        .bind(tenant_uuid)
        .bind(queue)
        .bind(worker_id)
        .bind(lease_seconds)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::db_error)?;

        let result = match row {
            Some(r) => Some(row_to_job(r)?),
            None => None,
        };
        drop(tx);
        tx_managed.commit().await.map_err(crate::db_error)?;
        Ok(result)
    }

    async fn complete(
        &self,
        job_id: Uuid,
        worker_id: &str,
        revision: i64,
        ctx: &RequestContext,
    ) -> Result<Job, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let row = sqlx::query(
            "UPDATE infra.jobs
             SET status = 'completed',
                 lease_owner = NULL,
                 lease_until = NULL,
                 updated_at = clock_timestamp()
             WHERE job_id = $1
               AND lease_owner = $2
               AND revision = $3
               AND lease_until > clock_timestamp()
               AND status = 'running'
             RETURNING job_id, tenant_id, queue, payload, status, revision,
                       lease_owner, lease_until, attempts, max_attempts, next_run, deadline",
        )
        .bind(job_id)
        .bind(worker_id)
        .bind(revision)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::db_error)?;

        let result = match row {
            Some(r) => row_to_job(r)?,
            None => return Err(PlatformError::VersionMismatch),
        };
        drop(tx);
        tx_managed.commit().await.map_err(crate::db_error)?;
        Ok(result)
    }

    async fn fail(
        &self,
        job_id: Uuid,
        worker_id: &str,
        revision: i64,
        next_run: Option<UtcTimestamp>,
        ctx: &RequestContext,
    ) -> Result<Job, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let new_status = if next_run.is_some() {
            "pending"
        } else {
            "failed"
        };
        let next_run_db = next_run.map(utc_to_db);

        let row = sqlx::query(
            "UPDATE infra.jobs
             SET status = $4,
                 lease_owner = NULL,
                 lease_until = NULL,
                 next_run = COALESCE($5, next_run),
                 updated_at = clock_timestamp()
             WHERE job_id = $1
               AND lease_owner = $2
               AND revision = $3
               AND lease_until > clock_timestamp()
               AND status = 'running'
             RETURNING job_id, tenant_id, queue, payload, status, revision,
                       lease_owner, lease_until, attempts, max_attempts, next_run, deadline",
        )
        .bind(job_id)
        .bind(worker_id)
        .bind(revision)
        .bind(new_status)
        .bind(next_run_db)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::db_error)?;

        let result = match row {
            Some(r) => row_to_job(r)?,
            None => return Err(PlatformError::VersionMismatch),
        };
        drop(tx);
        tx_managed.commit().await.map_err(crate::db_error)?;
        Ok(result)
    }

    async fn cancel(&self, job_id: Uuid, ctx: &RequestContext) -> Result<Job, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let row = sqlx::query(
            "UPDATE infra.jobs
             SET status = 'cancelled',
                 lease_owner = NULL,
                 lease_until = NULL,
                 updated_at = clock_timestamp()
             WHERE job_id = $1
               AND status IN ('pending', 'running')
             RETURNING job_id, tenant_id, queue, payload, status, revision,
                       lease_owner, lease_until, attempts, max_attempts, next_run, deadline",
        )
        .bind(job_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::db_error)?;

        let result = match row {
            Some(r) => row_to_job(r)?,
            None => return Err(PlatformError::NotFound),
        };
        drop(tx);
        tx_managed.commit().await.map_err(crate::db_error)?;
        Ok(result)
    }
}

fn row_to_job(row: sqlx::postgres::PgRow) -> Result<Job, PlatformError> {
    let job_id: Uuid = row.try_get("job_id").map_err(crate::db_error)?;
    let tenant_uuid: Option<Uuid> = row.try_get("tenant_id").map_err(crate::db_error)?;
    let lease_owner: Option<String> = row.try_get("lease_owner").map_err(crate::db_error)?;
    let lease_until: Option<DateTime<Utc>> = row.try_get("lease_until").map_err(crate::db_error)?;
    let deadline: Option<DateTime<Utc>> = row.try_get("deadline").map_err(crate::db_error)?;
    let payload_value: serde_json::Value = row.try_get("payload").map_err(crate::db_error)?;

    let tenant_id = tenant_uuid
        .map(|u| TenantId::parse_str(&u.to_string()))
        .transpose()?;

    Ok(Job {
        job_id: Some(job_id),
        tenant_id,
        queue: row.try_get("queue").map_err(crate::db_error)?,
        payload: payload_value.to_string(),
        status: status_from_db(
            &row.try_get::<String, _>("status")
                .map_err(crate::db_error)?,
        )?,
        revision: row.try_get("revision").map_err(crate::db_error)?,
        lease_owner,
        lease_until: lease_until.map(UtcTimestamp::from),
        attempts: row.try_get("attempts").map_err(crate::db_error)?,
        max_attempts: row.try_get("max_attempts").map_err(crate::db_error)?,
        next_run: UtcTimestamp::from(
            row.try_get::<DateTime<Utc>, _>("next_run")
                .map_err(crate::db_error)?,
        ),
        deadline: deadline.map(UtcTimestamp::from),
    })
}

fn status_to_db(status: JobStatus) -> &'static str {
    match status {
        JobStatus::Pending => "pending",
        JobStatus::Running => "running",
        JobStatus::Completed => "completed",
        JobStatus::Failed => "failed",
        JobStatus::Cancelled => "cancelled",
    }
}

fn status_from_db(value: &str) -> Result<JobStatus, PlatformError> {
    match value {
        "pending" => Ok(JobStatus::Pending),
        "running" => Ok(JobStatus::Running),
        "completed" => Ok(JobStatus::Completed),
        "failed" => Ok(JobStatus::Failed),
        "cancelled" => Ok(JobStatus::Cancelled),
        _ => Err(PlatformError::invalid(
            "job_status",
            format!("unknown status: {value}"),
        )),
    }
}

fn utc_to_db(ts: UtcTimestamp) -> DateTime<Utc> {
    ts.into()
}
