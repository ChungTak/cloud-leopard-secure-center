//! PostgreSQL implementation of the `LoginAttemptRepository` port.

use async_trait::async_trait;
use foundation::{ErrorCode, PlatformError, RequestContext, TenantId};
use storage_api::LoginAttemptRepository;

use crate::begin_tenant_transaction;

/// PostgreSQL-backed login attempt repository.
#[derive(Debug, Clone)]
pub struct PostgresLoginAttemptRepository {
    pool: sqlx::PgPool,
}

impl PostgresLoginAttemptRepository {
    /// Create a new repository backed by `pool`.
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl LoginAttemptRepository for PostgresLoginAttemptRepository {
    async fn record(
        &self,
        tenant_id: TenantId,
        identity: &str,
        ip: Option<String>,
        success: bool,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        sqlx::query(
            "INSERT INTO audit.login_attempts (tenant_id, identity, success, ip_address, created_at)
             VALUES ($1, $2, $3, $4, now())",
        )
        .bind(*tenant_id.as_uuid())
        .bind(identity)
        .bind(success)
        .bind(ip)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn count_failures_by_identity(
        &self,
        tenant_id: TenantId,
        identity: &str,
        window_seconds: i64,
        ctx: &RequestContext,
    ) -> Result<i64, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM audit.login_attempts
             WHERE tenant_id = $1 AND identity = $2 AND success = false
               AND created_at > now() - ($3 || ' seconds')::interval",
        )
        .bind(*tenant_id.as_uuid())
        .bind(identity)
        .bind(window_seconds)
        .fetch_one(&mut *tx)
        .await
        .map_err(db_error)?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(row.0)
    }

    async fn count_failures_by_source(
        &self,
        tenant_id: TenantId,
        ip: String,
        window_seconds: i64,
        ctx: &RequestContext,
    ) -> Result<i64, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM audit.login_attempts
             WHERE tenant_id = $1 AND ip_address = $2 AND success = false
               AND created_at > now() - ($3 || ' seconds')::interval",
        )
        .bind(*tenant_id.as_uuid())
        .bind(ip)
        .bind(window_seconds)
        .fetch_one(&mut *tx)
        .await
        .map_err(db_error)?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(row.0)
    }
}

fn db_error(e: sqlx::Error) -> PlatformError {
    PlatformError::new(ErrorCode::Unavailable, e.to_string())
}
