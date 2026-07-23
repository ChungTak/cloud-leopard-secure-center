//! PostgreSQL implementation of the `OutboxRepository` port.

use async_trait::async_trait;
use foundation::chrono::{DateTime, Utc};
use foundation::uuid::Uuid;
use foundation::{ErrorCode, PlatformError, RequestContext, TenantId, UtcTimestamp};
use sqlx::{PgPool, Row};
use storage_api::{OutboxMessage, OutboxRepository};

use crate::begin_tenant_transaction;

/// PostgreSQL-backed outbox repository.
#[derive(Debug, Clone)]
pub struct PostgresOutboxRepository {
    pool: PgPool,
}

impl PostgresOutboxRepository {
    /// Create a new repository backed by `pool`.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl OutboxRepository for PostgresOutboxRepository {
    async fn append(
        &self,
        message: &OutboxMessage,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let tenant_uuid = message.tenant_id.map(|id| *id.as_uuid());
        sqlx::query(
            "INSERT INTO infra.outbox_messages
             (message_id, tenant_id, aggregate_type, aggregate_id, aggregate_sequence,
              event_type, payload, occurred_at, available_at, attempts, published_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7::jsonb, $8, $9, $10, NULL)
             ON CONFLICT (message_id) DO NOTHING",
        )
        .bind(message.message_id)
        .bind(tenant_uuid)
        .bind(&message.aggregate_type)
        .bind(&message.aggregate_id)
        .bind(message.aggregate_sequence)
        .bind(&message.event_type)
        .bind(&message.payload)
        .bind(utc_to_db(message.occurred_at))
        .bind(utc_to_db(message.available_at))
        .bind(message.attempts)
        .execute(&mut *tx)
        .await
        .map_err(crate::db_error)?;

        drop(tx);
        tx_managed.commit().await.map_err(crate::db_error)?;
        Ok(())
    }

    async fn claim(
        &self,
        batch_size: u64,
        lease: std::time::Duration,
        ctx: &RequestContext,
    ) -> Result<Vec<OutboxMessage>, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let rows = sqlx::query(
            "SELECT message_id, tenant_id, aggregate_type, aggregate_id, aggregate_sequence,
                    event_type, payload, occurred_at, available_at, attempts, published_at
             FROM infra.outbox_messages
             WHERE published_at IS NULL
               AND available_at <= clock_timestamp()
             ORDER BY available_at
             LIMIT $1
             FOR UPDATE SKIP LOCKED",
        )
        .bind(i64::try_from(batch_size).unwrap_or(i64::MAX))
        .fetch_all(&mut *tx)
        .await
        .map_err(crate::db_error)?;

        let mut messages = Vec::with_capacity(rows.len());
        for row in rows {
            messages.push(row_to_message(row)?);
        }

        // Mark claimed rows as in-flight by bumping attempts and moving the
        // availability time forward by the lease duration so concurrent claimers
        // skip messages that are already being published.
        let lease_seconds = lease.as_secs_f64();
        for message in &mut messages {
            message.attempts += 1;
            let row = sqlx::query(
                "UPDATE infra.outbox_messages
                 SET attempts = attempts + 1,
                     available_at = clock_timestamp() + $2 * interval '1 second'
                 WHERE message_id = $1
                 RETURNING available_at",
            )
            .bind(message.message_id)
            .bind(lease_seconds)
            .fetch_one(&mut *tx)
            .await
            .map_err(crate::db_error)?;
            let available_at: DateTime<Utc> =
                row.try_get("available_at").map_err(crate::db_error)?;
            message.available_at = UtcTimestamp::from(available_at);
        }

        drop(tx);
        tx_managed.commit().await.map_err(crate::db_error)?;
        Ok(messages)
    }

    async fn mark_published(
        &self,
        message_id: Uuid,
        published_at: UtcTimestamp,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let result = sqlx::query(
            "UPDATE infra.outbox_messages
             SET published_at = $2
             WHERE message_id = $1 AND published_at IS NULL",
        )
        .bind(message_id)
        .bind(utc_to_db(published_at))
        .execute(&mut *tx)
        .await
        .map_err(crate::db_error)?;

        if result.rows_affected() == 0 {
            return Err(PlatformError::new(
                ErrorCode::NotFound,
                "outbox message not found or already published".to_string(),
            ));
        }

        drop(tx);
        tx_managed.commit().await.map_err(crate::db_error)?;
        Ok(())
    }
}

fn row_to_message(row: sqlx::postgres::PgRow) -> Result<OutboxMessage, PlatformError> {
    let tenant_uuid: Option<Uuid> = row.try_get("tenant_id").map_err(crate::db_error)?;
    let occurred_at: DateTime<Utc> = row.try_get("occurred_at").map_err(crate::db_error)?;
    let available_at: DateTime<Utc> = row.try_get("available_at").map_err(crate::db_error)?;
    let published_at: Option<DateTime<Utc>> =
        row.try_get("published_at").map_err(crate::db_error)?;
    let message_uuid: Uuid = row.try_get("message_id").map_err(crate::db_error)?;

    let tenant_id = match tenant_uuid {
        Some(uuid) => Some(TenantId::parse_str(&uuid.to_string())?),
        None => None,
    };

    Ok(OutboxMessage {
        message_id: message_uuid,
        tenant_id,
        aggregate_type: row.try_get("aggregate_type").map_err(crate::db_error)?,
        aggregate_id: row.try_get("aggregate_id").map_err(crate::db_error)?,
        aggregate_sequence: row.try_get("aggregate_sequence").map_err(crate::db_error)?,
        event_type: row.try_get("event_type").map_err(crate::db_error)?,
        payload: {
            let value: serde_json::Value = row.try_get("payload").map_err(crate::db_error)?;
            value.to_string()
        },
        occurred_at: UtcTimestamp::from(occurred_at),
        available_at: UtcTimestamp::from(available_at),
        attempts: row.try_get("attempts").map_err(crate::db_error)?,
        published_at: published_at.map(UtcTimestamp::from),
    })
}

fn utc_to_db(ts: UtcTimestamp) -> DateTime<Utc> {
    ts.into()
}
