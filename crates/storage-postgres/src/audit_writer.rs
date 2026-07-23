//! PostgreSQL append-only audit writer.

use async_trait::async_trait;
use domain_audit::audit_record::{AuditDetails, AuditRecord, AuditRecordId};
use foundation::{ErrorCode, PlatformError, RequestContext, UtcTimestamp, chrono::{DateTime, Utc}};
use sqlx::PgPool;
use storage_api::AuditWriter;

use crate::begin_tenant_transaction;

/// PostgreSQL-backed append-only audit writer.
#[derive(Debug, Clone)]
pub struct PostgresAuditWriter {
    pool: PgPool,
}

impl PostgresAuditWriter {
    /// Create a new writer backed by `pool`.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AuditWriter for PostgresAuditWriter {
    async fn write(
        &self,
        record: &AuditRecord,
        ctx: &RequestContext,
    ) -> Result<AuditRecordId, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        sqlx::query("SET LOCAL ROLE clsc_audit_writer")
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;

        let tenant_uuid = ctx
            .tenant_id
            .map(|t| *t.as_uuid())
            .ok_or_else(missing_tenant)?;

        if *record.tenant_id.as_uuid() != tenant_uuid {
            return Err(PlatformError::new(
                ErrorCode::Invalid,
                "audit record tenant does not match context".to_string(),
            ));
        }

        let id: i64 = sqlx::query_scalar(
            "INSERT INTO audit.records
             (tenant_id, actor_type, actor_id, action, target_type, target_id,
              result, risk, request_id, trace_id, source_ip, before_digest, after_digest,
              occurred_at, details)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
             RETURNING id",
        )
        .bind(tenant_uuid)
        .bind(&record.actor_type)
        .bind(&record.actor_id)
        .bind(&record.action)
        .bind(&record.target_type)
        .bind(&record.target_id)
        .bind(record.result.as_str())
        .bind(record.risk.as_str())
        .bind(record.request_id.as_ref())
        .bind(record.trace_id.as_ref())
        .bind(record.source_ip.as_ref())
        .bind(record.before_digest.as_ref())
        .bind(record.after_digest.as_ref())
        .bind(timestamp_to_db(record.occurred_at))
        .bind(details_to_json(&record.details)?)
        .fetch_one(&mut *tx)
        .await
        .map_err(db_error)?;

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(AuditRecordId::new(id))
    }
}

fn details_to_json(details: &AuditDetails) -> Result<serde_json::Value, PlatformError> {
    serde_json::from_str(&details.value)
        .map_err(|e| PlatformError::invalid("details", format!("invalid audit details JSON: {e}")))
}

fn timestamp_to_db(ts: UtcTimestamp) -> DateTime<Utc> {
    ts.into()
}

fn db_error(e: sqlx::Error) -> PlatformError {
    PlatformError::new(ErrorCode::Unavailable, e.to_string())
}

fn missing_tenant() -> PlatformError {
    PlatformError::new(
        ErrorCode::Invalid,
        "tenant_id is required in request context".to_string(),
    )
}
