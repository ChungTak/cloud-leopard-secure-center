//! PostgreSQL implementation of the `IdempotencyRepository` port.

use crate::db_error;
use async_trait::async_trait;
use foundation::chrono::{DateTime, Utc};
use foundation::uuid::Uuid;
use foundation::{ErrorCode, PlatformError, RequestContext, TenantId, UserId, UtcTimestamp};
use sqlx::{PgPool, Row};
use storage_api::{IdempotencyRecord, IdempotencyRepository};

use crate::begin_tenant_transaction;

/// PostgreSQL-backed idempotency repository.
#[derive(Debug, Clone)]
pub struct PostgresIdempotencyRepository {
    pool: PgPool,
}

impl PostgresIdempotencyRepository {
    /// Create a new repository backed by `pool`.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl IdempotencyRepository for PostgresIdempotencyRepository {
    async fn find(
        &self,
        tenant_id: Option<TenantId>,
        principal_id: UserId,
        endpoint_scope: &str,
        idempotency_key: &str,
        ctx: &RequestContext,
    ) -> Result<Option<IdempotencyRecord>, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let row = sqlx::query(
            "SELECT tenant_id, principal_id, endpoint_scope, idempotency_key,
                    request_digest, response_status, response_body, expires_at
             FROM infra.idempotency_records
             WHERE tenant_id IS NOT DISTINCT FROM $1
               AND principal_id = $2
               AND endpoint_scope = $3
               AND idempotency_key = $4",
        )
        .bind(tenant_id.map(|id| *id.as_uuid()))
        .bind(*principal_id.as_uuid())
        .bind(endpoint_scope)
        .bind(idempotency_key)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        let record = row.map(row_to_record).transpose()?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(record)
    }

    async fn save(
        &self,
        record: &IdempotencyRecord,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let tenant_uuid = record.tenant_id.map(|id| *id.as_uuid());
        sqlx::query(
            "INSERT INTO infra.idempotency_records
             (tenant_id, principal_id, endpoint_scope, idempotency_key,
              request_digest, response_status, response_body, expires_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (tenant_id, principal_id, endpoint_scope, idempotency_key) DO UPDATE
             SET request_digest = EXCLUDED.request_digest,
                 response_status = EXCLUDED.response_status,
                 response_body = EXCLUDED.response_body,
                 expires_at = EXCLUDED.expires_at",
        )
        .bind(tenant_uuid)
        .bind(*record.principal_id.as_uuid())
        .bind(&record.endpoint_scope)
        .bind(&record.idempotency_key)
        .bind(&record.request_digest)
        .bind(record.response_status)
        .bind(record.response_body.as_ref())
        .bind(utc_to_db(record.expires_at))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn save_or_conflict(
        &self,
        record: &IdempotencyRecord,
        ctx: &RequestContext,
    ) -> Result<Option<IdempotencyRecord>, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let tenant_uuid = record.tenant_id.map(|id| *id.as_uuid());

        let existing = sqlx::query(
            "SELECT tenant_id, principal_id, endpoint_scope, idempotency_key,
                    request_digest, response_status, response_body, expires_at
             FROM infra.idempotency_records
             WHERE tenant_id IS NOT DISTINCT FROM $1
               AND principal_id = $2
               AND endpoint_scope = $3
               AND idempotency_key = $4
             FOR UPDATE",
        )
        .bind(tenant_uuid)
        .bind(*record.principal_id.as_uuid())
        .bind(&record.endpoint_scope)
        .bind(&record.idempotency_key)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        if let Some(row) = existing {
            let existing_record = row_to_record(row)?;
            if existing_record.request_digest != record.request_digest {
                return Err(PlatformError::new(
                    ErrorCode::Conflict,
                    "idempotency key reused with different request digest",
                ));
            }
            drop(tx);
            tx_managed.commit().await.map_err(db_error)?;
            return Ok(Some(existing_record));
        }

        sqlx::query(
            "INSERT INTO infra.idempotency_records
             (tenant_id, principal_id, endpoint_scope, idempotency_key,
              request_digest, response_status, response_body, expires_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (tenant_id, principal_id, endpoint_scope, idempotency_key) DO NOTHING",
        )
        .bind(tenant_uuid)
        .bind(*record.principal_id.as_uuid())
        .bind(&record.endpoint_scope)
        .bind(&record.idempotency_key)
        .bind(&record.request_digest)
        .bind(record.response_status)
        .bind(record.response_body.as_ref())
        .bind(utc_to_db(record.expires_at))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(None)
    }
}

fn row_to_record(row: sqlx::postgres::PgRow) -> Result<IdempotencyRecord, PlatformError> {
    let tenant_uuid: Option<Uuid> = row.try_get("tenant_id").map_err(db_error)?;
    let principal_uuid: Uuid = row.try_get("principal_id").map_err(db_error)?;
    let expires_at: DateTime<Utc> = row.try_get("expires_at").map_err(db_error)?;

    let tenant_id = match tenant_uuid {
        Some(uuid) => Some(TenantId::parse_str(&uuid.to_string())?),
        None => None,
    };

    Ok(IdempotencyRecord {
        tenant_id,
        principal_id: UserId::parse_str(&principal_uuid.to_string())?,
        endpoint_scope: row.try_get("endpoint_scope").map_err(db_error)?,
        idempotency_key: row.try_get("idempotency_key").map_err(db_error)?,
        request_digest: row.try_get("request_digest").map_err(db_error)?,
        response_status: row.try_get("response_status").map_err(db_error)?,
        response_body: row.try_get("response_body").map_err(db_error)?,
        expires_at: UtcTimestamp::from(expires_at),
    })
}

fn utc_to_db(ts: UtcTimestamp) -> DateTime<Utc> {
    ts.into()
}
