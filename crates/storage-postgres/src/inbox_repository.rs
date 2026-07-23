//! PostgreSQL implementation of the `InboxRepository` port.

use async_trait::async_trait;
use foundation::chrono::{DateTime, Utc};
use foundation::uuid::Uuid;
use foundation::{PlatformError, RequestContext, TenantId, UtcTimestamp};
use sqlx::{PgPool, Row};
use storage_api::{InboxMessage, InboxRepository, InboxStatus};

use crate::begin_tenant_transaction;

/// PostgreSQL-backed inbox repository.
#[derive(Debug, Clone)]
pub struct PostgresInboxRepository {
    pool: PgPool,
}

impl PostgresInboxRepository {
    /// Create a new repository backed by `pool`.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl InboxRepository for PostgresInboxRepository {
    async fn receive(
        &self,
        message: &InboxMessage,
        ctx: &RequestContext,
    ) -> Result<InboxMessage, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let tenant_uuid = message.tenant_id.map(|id| *id.as_uuid());
        let inserted = sqlx::query(
            "INSERT INTO infra.inbox_messages
             (tenant_id, consumer_id, message_id, status, result_digest, attempts, expires_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (tenant_id, consumer_id, message_id) DO NOTHING
             RETURNING status, result_digest, attempts, expires_at",
        )
        .bind(tenant_uuid)
        .bind(&message.consumer_id)
        .bind(message.message_id)
        .bind(status_to_db(message.status))
        .bind(message.result_digest.as_ref())
        .bind(message.attempts)
        .bind(utc_to_db(message.expires_at))
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::db_error)?;

        if let Some(row) = inserted {
            let result = row_to_message(
                row,
                message.tenant_id,
                &message.consumer_id,
                message.message_id,
            )?;
            drop(tx);
            tx_managed.commit().await.map_err(crate::db_error)?;
            return Ok(result);
        }

        let existing = sqlx::query(
            "SELECT status, result_digest, attempts, expires_at
             FROM infra.inbox_messages
             WHERE tenant_id IS NOT DISTINCT FROM $1
               AND consumer_id = $2
               AND message_id = $3",
        )
        .bind(tenant_uuid)
        .bind(&message.consumer_id)
        .bind(message.message_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(crate::db_error)?;

        let result = row_to_message(
            existing,
            message.tenant_id,
            &message.consumer_id,
            message.message_id,
        )?;
        drop(tx);
        tx_managed.commit().await.map_err(crate::db_error)?;
        Ok(result)
    }

    async fn complete(
        &self,
        consumer_id: &str,
        message_id: Uuid,
        result_digest: &str,
        ctx: &RequestContext,
    ) -> Result<InboxMessage, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let tenant_uuid = ctx.tenant_id.map(|id| *id.as_uuid());
        let row = sqlx::query(
            "UPDATE infra.inbox_messages
             SET status = 'completed',
                 result_digest = COALESCE(result_digest, $1),
                 updated_at = clock_timestamp()
             WHERE tenant_id IS NOT DISTINCT FROM $2
               AND consumer_id = $3
               AND message_id = $4
             RETURNING status, result_digest, attempts, expires_at",
        )
        .bind(result_digest)
        .bind(tenant_uuid)
        .bind(consumer_id)
        .bind(message_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(crate::db_error)?;

        let result = row_to_message(row, ctx.tenant_id, consumer_id, message_id)?;
        drop(tx);
        tx_managed.commit().await.map_err(crate::db_error)?;
        Ok(result)
    }
}

fn row_to_message(
    row: sqlx::postgres::PgRow,
    tenant_id: Option<TenantId>,
    consumer_id: &str,
    message_id: Uuid,
) -> Result<InboxMessage, PlatformError> {
    let status: String = row.try_get("status").map_err(crate::db_error)?;
    let result_digest: Option<String> = row.try_get("result_digest").map_err(crate::db_error)?;
    let attempts: i32 = row.try_get("attempts").map_err(crate::db_error)?;
    let expires_at: DateTime<Utc> = row.try_get("expires_at").map_err(crate::db_error)?;

    Ok(InboxMessage {
        message_id,
        tenant_id,
        consumer_id: consumer_id.to_string(),
        status: status_from_db(&status)?,
        result_digest,
        attempts,
        expires_at: UtcTimestamp::from(expires_at),
    })
}

fn status_to_db(status: InboxStatus) -> &'static str {
    match status {
        InboxStatus::Pending => "pending",
        InboxStatus::Completed => "completed",
        InboxStatus::Failed => "failed",
    }
}

fn status_from_db(value: &str) -> Result<InboxStatus, PlatformError> {
    match value {
        "pending" => Ok(InboxStatus::Pending),
        "completed" => Ok(InboxStatus::Completed),
        "failed" => Ok(InboxStatus::Failed),
        _ => Err(foundation::PlatformError::invalid(
            "inbox_status",
            format!("unknown status: {value}"),
        )),
    }
}

fn utc_to_db(ts: UtcTimestamp) -> DateTime<Utc> {
    ts.into()
}
