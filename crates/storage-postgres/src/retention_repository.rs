use async_trait::async_trait;
use domain_audit::retention::{
    CleanupBatchResult, LegalHold, RetentionPolicy, RetentionTarget, TenantRetentionOverride,
};
use foundation::{ErrorCode, PlatformError, TenantId, UtcTimestamp};
use sqlx::{PgPool, Postgres, Row, Transaction};
use storage_api::RetentionRepository;

fn db_error(e: sqlx::Error) -> PlatformError {
    use sqlx::Error;
    match e {
        Error::RowNotFound => PlatformError::new(ErrorCode::NotFound, "retention record not found"),
        Error::Database(db) if db.constraint().is_some() => {
            PlatformError::new(ErrorCode::Conflict, db.to_string())
        }
        _ => PlatformError::new(ErrorCode::Unavailable, e.to_string()),
    }
}

fn built_in_default(target: RetentionTarget) -> Result<RetentionPolicy, PlatformError> {
    let days = match target {
        RetentionTarget::AuditRecords => 365,
        RetentionTarget::AuditEvents => 90,
        RetentionTarget::LoginAttempts => 30,
        RetentionTarget::Outbox => 7,
        RetentionTarget::Inbox => 7,
    };
    RetentionPolicy::new(target, days, 1000)
}

fn parent_table(target: RetentionTarget) -> &'static str {
    match target {
        RetentionTarget::AuditRecords => "records",
        RetentionTarget::AuditEvents => "events",
        RetentionTarget::LoginAttempts => "login_attempts",
        RetentionTarget::Outbox => "outbox_messages",
        RetentionTarget::Inbox => "inbox_messages",
    }
}

fn resource_columns(target: RetentionTarget) -> Option<(&'static str, &'static str)> {
    match target {
        RetentionTarget::AuditRecords => Some(("target_type", "target_id")),
        RetentionTarget::AuditEvents => Some(("resource_type", "resource_id")),
        _ => None,
    }
}

fn timestamp_column(target: RetentionTarget) -> &'static str {
    match target {
        RetentionTarget::AuditRecords => "occurred_at",
        _ => "created_at",
    }
}

/// PostgreSQL-backed retention repository.
#[derive(Debug, Clone)]
pub struct PostgresRetentionRepository {
    pool: PgPool,
}

impl PostgresRetentionRepository {
    /// Create a new repository backed by `pool`.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Start a transaction and switch to the cleanup worker role, which bypasses row-level security.
    async fn begin_cleanup_transaction(&self) -> Result<Transaction<'_, Postgres>, PlatformError> {
        let mut tx = self.pool.begin().await.map_err(db_error)?;
        sqlx::query("SET LOCAL ROLE clsc_cleanup_worker")
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        Ok(tx)
    }
}

#[async_trait]
impl RetentionRepository for PostgresRetentionRepository {
    async fn save_policy(&self, policy: &RetentionPolicy) -> Result<(), PlatformError> {
        let mut tx = self.begin_cleanup_transaction().await?;

        sqlx::query(
            "INSERT INTO audit.retention_policy (target, tenant_id, days)
             VALUES ($1, NULL, $2)
             ON CONFLICT (target, tenant_id) DO UPDATE SET days = EXCLUDED.days",
        )
        .bind(policy.target.as_str())
        .bind(i64::from(policy.days))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

        tx.commit().await.map_err(db_error)
    }

    async fn get_policy(&self, target: RetentionTarget) -> Result<RetentionPolicy, PlatformError> {
        let row = sqlx::query(
            "SELECT days FROM audit.retention_policy
             WHERE target = $1 AND tenant_id IS NULL",
        )
        .bind(target.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(db_error)?;

        match row {
            Some(r) => {
                let days: i64 = r.get("days");
                let days_u32: u32 = days.try_into().map_err(|_| {
                    PlatformError::new(ErrorCode::Invalid, "invalid retention days".to_string())
                })?;
                RetentionPolicy::new(target, days_u32, 1000)
            }
            None => built_in_default(target),
        }
    }

    async fn set_tenant_override(
        &self,
        override_value: &TenantRetentionOverride,
    ) -> Result<(), PlatformError> {
        let mut tx = self.begin_cleanup_transaction().await?;

        sqlx::query(
            "INSERT INTO audit.retention_policy (target, tenant_id, days)
             VALUES ($1, $2, $3)
             ON CONFLICT (target, tenant_id) DO UPDATE SET days = EXCLUDED.days",
        )
        .bind(override_value.target.as_str())
        .bind(override_value.tenant_id.as_uuid())
        .bind(i64::from(override_value.days))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

        tx.commit().await.map_err(db_error)
    }

    async fn get_effective_days(
        &self,
        target: RetentionTarget,
        tenant_id: Option<TenantId>,
    ) -> Result<u32, PlatformError> {
        if let Some(t) = tenant_id {
            let row = sqlx::query(
                "SELECT days FROM audit.retention_policy
                 WHERE target = $1 AND tenant_id = $2",
            )
            .bind(target.as_str())
            .bind(t.as_uuid())
            .fetch_optional(&self.pool)
            .await
            .map_err(db_error)?;
            if let Some(r) = row {
                let days: i64 = r.get("days");
                return u32::try_from(days).map_err(|_| {
                    PlatformError::new(ErrorCode::Invalid, "invalid retention days".to_string())
                });
            }
        }

        let policy = self.get_policy(target).await?;
        Ok(policy.days)
    }

    async fn add_legal_hold(&self, hold: &LegalHold) -> Result<(), PlatformError> {
        let mut tx = self.begin_cleanup_transaction().await?;

        sqlx::query(
            "INSERT INTO audit.legal_holds (resource_type, resource_id, held_until)
             VALUES ($1, $2, $3)
             ON CONFLICT (resource_type, resource_id) DO UPDATE SET held_until = EXCLUDED.held_until",
        )
        .bind(&hold.resource_type)
        .bind(&hold.resource_id)
        .bind(chrono::DateTime::<chrono::Utc>::from(hold.held_until))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

        tx.commit().await.map_err(db_error)
    }

    async fn remove_legal_hold(
        &self,
        resource_type: &str,
        resource_id: &str,
    ) -> Result<(), PlatformError> {
        let mut tx = self.begin_cleanup_transaction().await?;

        sqlx::query("DELETE FROM audit.legal_holds WHERE resource_type = $1 AND resource_id = $2")
            .bind(resource_type)
            .bind(resource_id)
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;

        tx.commit().await.map_err(db_error)
    }

    async fn list_partitions_to_clean(
        &self,
        target: RetentionTarget,
        cutoff: UtcTimestamp,
    ) -> Result<Vec<String>, PlatformError> {
        let parent = parent_table(target);
        let rows = sqlx::query(
            "SELECT c.relname AS partition_name
             FROM pg_inherits i
             JOIN pg_class c ON c.oid = i.inhrelid
             JOIN pg_class p ON p.oid = i.inhparent
             JOIN pg_namespace n ON n.oid = p.relnamespace
             WHERE n.nspname = 'audit' AND p.relname = $1",
        )
        .bind(parent)
        .fetch_all(&self.pool)
        .await
        .map_err(db_error)?;

        let cutoff_dt: chrono::DateTime<chrono::Utc> = cutoff.into();
        let mut partitions = Vec::new();
        for row in rows {
            let name: String = row.get("partition_name");
            if let Some(end) = partition_end_from_name(&name)
                && cutoff_dt >= end
            {
                partitions.push(name);
            }
        }
        partitions.sort();
        Ok(partitions)
    }

    async fn acquire_lease(
        &self,
        target: RetentionTarget,
        partition: &str,
        worker_id: &str,
        lease_until: UtcTimestamp,
    ) -> Result<bool, PlatformError> {
        let mut tx = self.begin_cleanup_transaction().await?;

        let existing = sqlx::query(
            "SELECT lease_until, worker_id FROM audit.cleanup_checkpoint
             WHERE table_name = $1 AND partition_name = $2
             FOR UPDATE",
        )
        .bind(target.as_str())
        .bind(partition)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        let now_dt: chrono::DateTime<chrono::Utc> = UtcTimestamp::now().into();
        if let Some(r) = existing {
            let lease: Option<chrono::DateTime<chrono::Utc>> = r.get("lease_until");
            let owner: Option<String> = r.get("worker_id");
            if let (Some(l), Some(o)) = (lease, owner)
                && o != worker_id
                && now_dt < l
            {
                tx.rollback().await.map_err(db_error)?;
                return Ok(false);
            }
            sqlx::query(
                "UPDATE audit.cleanup_checkpoint
                 SET lease_until = $1, worker_id = $2, started_at = now(), completed_at = NULL
                 WHERE table_name = $3 AND partition_name = $4",
            )
            .bind(chrono::DateTime::<chrono::Utc>::from(lease_until))
            .bind(worker_id)
            .bind(target.as_str())
            .bind(partition)
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        } else {
            sqlx::query(
                "INSERT INTO audit.cleanup_checkpoint
                 (table_name, partition_name, cutoff, last_id, lease_until, worker_id, started_at)
                 VALUES ($1, $2, now(), 0, $3, $4, now())",
            )
            .bind(target.as_str())
            .bind(partition)
            .bind(chrono::DateTime::<chrono::Utc>::from(lease_until))
            .bind(worker_id)
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        }

        tx.commit().await.map_err(db_error)?;
        Ok(true)
    }

    async fn release_lease(
        &self,
        target: RetentionTarget,
        partition: &str,
        worker_id: &str,
    ) -> Result<(), PlatformError> {
        let mut tx = self.begin_cleanup_transaction().await?;

        sqlx::query(
            "UPDATE audit.cleanup_checkpoint
             SET lease_until = NULL, worker_id = NULL
             WHERE table_name = $1 AND partition_name = $2 AND worker_id = $3",
        )
        .bind(target.as_str())
        .bind(partition)
        .bind(worker_id)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

        tx.commit().await.map_err(db_error)
    }

    async fn cleanup_batch(
        &self,
        target: RetentionTarget,
        partition: &str,
        cutoff: UtcTimestamp,
        batch_size: u64,
    ) -> Result<CleanupBatchResult, PlatformError> {
        let mut tx = self.begin_cleanup_transaction().await?;

        let timestamp = timestamp_column(target);
        let row = if let Some((type_col, id_col)) = resource_columns(target) {
            sqlx::query("SELECT audit.cleanup_batch($1, $2, $3, $4, $5, $6, $7) AS deleted")
                .bind(target.as_str())
                .bind(partition)
                .bind(chrono::DateTime::<chrono::Utc>::from(cutoff))
                .bind(batch_size as i64)
                .bind(timestamp)
                .bind(type_col)
                .bind(id_col)
                .fetch_one(&mut *tx)
                .await
                .map_err(db_error)?
        } else {
            sqlx::query("SELECT audit.cleanup_batch($1, $2, $3, $4, $5) AS deleted")
                .bind(target.as_str())
                .bind(partition)
                .bind(chrono::DateTime::<chrono::Utc>::from(cutoff))
                .bind(batch_size as i64)
                .bind(timestamp)
                .fetch_one(&mut *tx)
                .await
                .map_err(db_error)?
        };

        tx.commit().await.map_err(db_error)?;

        let deleted: i64 = row.get("deleted");
        Ok(CleanupBatchResult {
            rows_deleted: deleted as u64,
            finished: (deleted as u64) < batch_size,
        })
    }

    async fn drop_partition(
        &self,
        target: RetentionTarget,
        partition: &str,
        backup_confirmed: bool,
    ) -> Result<(), PlatformError> {
        if !backup_confirmed {
            return Err(PlatformError::new(
                ErrorCode::Invalid,
                "partition drop requires a confirmed recoverable backup",
            ));
        }

        let mut tx = self.begin_cleanup_transaction().await?;

        sqlx::query(
            "INSERT INTO audit.records (actor, tenant_id, action, target_type, target_id, result, details)
             VALUES ('cleanup_worker', NULL, 'partition_drop', $1, $2, 'pending', jsonb_build_object('partition', $2))",
        )
        .bind(target.as_str())
        .bind(partition)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

        tx.commit().await.map_err(db_error)?;

        Err(PlatformError::new(
            ErrorCode::Unsupported,
            "partition drop and backup orchestration are not implemented in this build",
        ))
    }
}

fn partition_end_from_name(name: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    let suffix = name.rsplit('_').take(2).collect::<Vec<_>>();
    if suffix.len() != 2 {
        return None;
    }
    let year: i32 = suffix[1].parse().ok()?;
    let month: u32 = suffix[0].parse().ok()?;
    let start = chrono::NaiveDate::from_ymd_opt(year, month, 1)?
        .and_hms_opt(0, 0, 0)?
        .and_utc();
    Some(start + chrono::Months::new(1))
}
