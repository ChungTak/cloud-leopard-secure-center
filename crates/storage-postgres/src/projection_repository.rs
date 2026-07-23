//! PostgreSQL implementation of the signaling projection repository.

use async_trait::async_trait;
use domain_resource::projection::{
    ChannelEvent, ChannelProjection, DeviceEvent, DeviceProjection, ProjectionFailure,
    is_projection_stale,
};
use foundation::{
    ErrorCode, IdGenerator, PlatformError, RequestContext, SystemClock, SystemIdGenerator,
    SystemRandom, UtcTimestamp,
    chrono::{DateTime, Utc},
    uuid::Uuid,
};
use sqlx::{AssertSqlSafe, PgPool, Row};
use storage_api::ProjectionRepository;

use crate::begin_tenant_transaction;

const STALE_TTL_MILLIS: i64 = 300_000; // 5 minutes
const DEFAULT_DEVICE_VIEW: &str = "devices";
const SHADOW_DEVICE_VIEW: &str = "devices_shadow";
const DEFAULT_CHANNEL_VIEW: &str = "channels";
const SHADOW_CHANNEL_VIEW: &str = "channels_shadow";

/// PostgreSQL-backed projection repository.
#[derive(Debug, Clone)]
pub struct PostgresProjectionRepository {
    pool: PgPool,
}

impl PostgresProjectionRepository {
    /// Create a new repository backed by `pool`.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProjectionRepository for PostgresProjectionRepository {
    async fn apply_device_event(
        &self,
        event: DeviceEvent,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        event.validate()?;
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let tenant_uuid = ctx
            .tenant_id
            .map(|t| *t.as_uuid())
            .ok_or_else(missing_tenant)?;

        ensure_active_view(&mut *tx, tenant_uuid).await?;

        let device_view: (String,) = sqlx::query_as(
            "SELECT device_view FROM projection.active_view WHERE tenant_id = $1 FOR UPDATE",
        )
        .bind(tenant_uuid)
        .fetch_one(&mut *tx)
        .await
        .map_err(db_error)?;
        let table = device_view.0;
        validate_table_name(&table)?;

        let existing =
            fetch_device_projection_sql(&mut *tx, &table, tenant_uuid, &event.external_ref).await?;

        match existing {
            None => {
                let stale = !event.is_contiguous(None);
                insert_device_projection(&mut *tx, &table, tenant_uuid, &event, stale).await?;
            }
            Some(proj) => {
                if event.sequence < proj.sequence {
                    // out of order
                    return Ok(());
                }
                if event.sequence == proj.sequence {
                    if event.payload == proj.payload {
                        // duplicate
                        return Ok(());
                    }
                    // mismatched payload for same sequence
                    record_failure_sql(
                        &mut *tx,
                        tenant_uuid,
                        &event.source_event_id,
                        &event.external_ref,
                        "payload_mismatch",
                        &event.payload,
                    )
                    .await?;
                    return Ok(());
                }
                // newer sequence
                let stale = !event.is_contiguous(Some(proj.sequence));
                upsert_device_projection(&mut *tx, &table, tenant_uuid, &event, stale).await?;
            }
        }

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn get_device(
        &self,
        external_ref: &str,
        ctx: &RequestContext,
    ) -> Result<DeviceProjection, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let tenant_uuid = ctx
            .tenant_id
            .map(|t| *t.as_uuid())
            .ok_or_else(missing_tenant)?;

        ensure_active_view(&mut *tx, tenant_uuid).await?;

        let device_view: (String,) =
            sqlx::query_as("SELECT device_view FROM projection.active_view WHERE tenant_id = $1")
                .bind(tenant_uuid)
                .fetch_one(&mut *tx)
                .await
                .map_err(db_error)?;
        let table = device_view.0;
        validate_table_name(&table)?;

        let row = fetch_device_projection_sql(&mut *tx, &table, tenant_uuid, external_ref).await?;
        let projection = match row {
            Some(proj) => {
                let mut proj = proj;
                let now = UtcTimestamp::now();
                proj.stale =
                    proj.stale || is_projection_stale(proj.observed_at, now, STALE_TTL_MILLIS);
                proj
            }
            None => {
                return Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "device projection not found".to_string(),
                ));
            }
        };

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(projection)
    }

    async fn apply_channel_event(
        &self,
        event: ChannelEvent,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        event.validate()?;
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let tenant_uuid = ctx
            .tenant_id
            .map(|t| *t.as_uuid())
            .ok_or_else(missing_tenant)?;

        ensure_active_view(&mut *tx, tenant_uuid).await?;

        let channel_view: (String,) = sqlx::query_as(
            "SELECT channel_view FROM projection.active_view WHERE tenant_id = $1 FOR UPDATE",
        )
        .bind(tenant_uuid)
        .fetch_one(&mut *tx)
        .await
        .map_err(db_error)?;
        let table = channel_view.0;
        validate_table_name(&table)?;

        let existing =
            fetch_channel_projection_sql(&mut *tx, &table, tenant_uuid, &event.external_ref)
                .await?;

        match existing {
            None => {
                let stale = !event.is_contiguous(None);
                insert_channel_projection(&mut *tx, &table, tenant_uuid, &event, stale).await?;
            }
            Some(proj) => {
                if event.sequence < proj.sequence {
                    return Ok(());
                }
                if event.sequence == proj.sequence {
                    if event.payload == proj.payload {
                        return Ok(());
                    }
                    record_failure_sql(
                        &mut *tx,
                        tenant_uuid,
                        &event.source_event_id,
                        &event.external_ref,
                        "payload_mismatch",
                        &event.payload,
                    )
                    .await?;
                    return Ok(());
                }
                let stale = !event.is_contiguous(Some(proj.sequence));
                upsert_channel_projection(&mut *tx, &table, tenant_uuid, &event, stale).await?;
            }
        }

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn get_channel(
        &self,
        external_ref: &str,
        ctx: &RequestContext,
    ) -> Result<ChannelProjection, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let tenant_uuid = ctx
            .tenant_id
            .map(|t| *t.as_uuid())
            .ok_or_else(missing_tenant)?;

        ensure_active_view(&mut *tx, tenant_uuid).await?;

        let channel_view: (String,) =
            sqlx::query_as("SELECT channel_view FROM projection.active_view WHERE tenant_id = $1")
                .bind(tenant_uuid)
                .fetch_one(&mut *tx)
                .await
                .map_err(db_error)?;
        let table = channel_view.0;
        validate_table_name(&table)?;

        let row = fetch_channel_projection_sql(&mut *tx, &table, tenant_uuid, external_ref).await?;
        let projection = match row {
            Some(proj) => {
                let mut proj = proj;
                let now = UtcTimestamp::now();
                proj.stale =
                    proj.stale || is_projection_stale(proj.observed_at, now, STALE_TTL_MILLIS);
                proj
            }
            None => {
                return Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "channel projection not found".to_string(),
                ));
            }
        };

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(projection)
    }

    async fn rebuild_shadow(
        &self,
        device_events: Vec<DeviceEvent>,
        channel_events: Vec<ChannelEvent>,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let tenant_uuid = ctx
            .tenant_id
            .map(|t| *t.as_uuid())
            .ok_or_else(missing_tenant)?;

        ensure_active_view(&mut *tx, tenant_uuid).await?;

        let active_view: (String, String, i64) = sqlx::query_as(
            "SELECT device_view, channel_view, generation FROM projection.active_view
             WHERE tenant_id = $1 FOR UPDATE",
        )
        .bind(tenant_uuid)
        .fetch_one(&mut *tx)
        .await
        .map_err(db_error)?;

        let device_shadow = other_view(&active_view.0)?;
        let channel_shadow = other_view(&active_view.1)?;

        // Truncate shadow tables for this tenant.
        let sql = format!("DELETE FROM projection.{device_shadow} WHERE tenant_id = $1");
        sqlx::query(AssertSqlSafe(sql))
            .bind(tenant_uuid)
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        let sql = format!("DELETE FROM projection.{channel_shadow} WHERE tenant_id = $1");
        sqlx::query(AssertSqlSafe(sql))
            .bind(tenant_uuid)
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;

        let mut device_last: std::collections::HashMap<String, i64> =
            std::collections::HashMap::new();
        let mut sorted_devices = device_events;
        sorted_devices.sort_by(|a, b| {
            a.external_ref
                .cmp(&b.external_ref)
                .then(a.sequence.cmp(&b.sequence))
        });

        for event in sorted_devices {
            if event.external_ref.trim().is_empty() {
                continue;
            }
            let last = device_last.get(&event.external_ref).copied();
            if let Some(last) = last
                && event.sequence <= last
            {
                continue;
            }
            let stale = !event.is_contiguous(last);
            insert_device_projection(&mut *tx, device_shadow, tenant_uuid, &event, stale).await?;
            device_last.insert(event.external_ref.clone(), event.sequence);
        }

        let mut channel_last: std::collections::HashMap<String, i64> =
            std::collections::HashMap::new();
        let mut sorted_channels = channel_events;
        sorted_channels.sort_by(|a, b| {
            a.external_ref
                .cmp(&b.external_ref)
                .then(a.sequence.cmp(&b.sequence))
        });

        for event in sorted_channels {
            if event.external_ref.trim().is_empty() {
                continue;
            }
            let last = channel_last.get(&event.external_ref).copied();
            if let Some(last) = last
                && event.sequence <= last
            {
                continue;
            }
            let stale = !event.is_contiguous(last);
            insert_channel_projection(&mut *tx, channel_shadow, tenant_uuid, &event, stale).await?;
            channel_last.insert(event.external_ref.clone(), event.sequence);
        }

        sqlx::query(
            "UPDATE projection.active_view
             SET device_view = $1, channel_view = $2, generation = $3, updated_at = $4
             WHERE tenant_id = $5",
        )
        .bind(device_shadow)
        .bind(channel_shadow)
        .bind(active_view.2 + 1)
        .bind(Utc::now())
        .bind(tenant_uuid)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn checkpoint(
        &self,
        worker_id: &str,
        last_event_id: &str,
        observed_at: UtcTimestamp,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let tenant_uuid = ctx
            .tenant_id
            .map(|t| *t.as_uuid())
            .ok_or_else(missing_tenant)?;

        sqlx::query(
            "INSERT INTO projection.checkpoints (worker_id, tenant_id, last_event_id, last_observed_at, updated_at)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (worker_id, tenant_id) DO UPDATE
             SET last_event_id = EXCLUDED.last_event_id,
                 last_observed_at = EXCLUDED.last_observed_at,
                 updated_at = EXCLUDED.updated_at",
        )
        .bind(worker_id)
        .bind(tenant_uuid)
        .bind(last_event_id)
        .bind(utc_to_db(observed_at))
        .bind(Utc::now())
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn record_failure(
        &self,
        failure: ProjectionFailure,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let tenant_uuid = ctx
            .tenant_id
            .map(|t| *t.as_uuid())
            .ok_or_else(missing_tenant)?;

        let id = if failure.id.is_empty() {
            let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
            generator.generate()?
        } else {
            Uuid::parse_str(&failure.id)
                .map_err(|e| PlatformError::invalid("failure_id", e.to_string()))?
        };

        sqlx::query(
            "INSERT INTO projection.failures
             (id, tenant_id, source_event_id, external_ref, reason, payload, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(id)
        .bind(tenant_uuid)
        .bind(&failure.source_event_id)
        .bind(&failure.external_ref)
        .bind(&failure.reason)
        .bind(&failure.payload)
        .bind(Utc::now())
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }
}

async fn ensure_active_view(
    tx: &mut sqlx::postgres::PgConnection,
    tenant_uuid: Uuid,
) -> Result<(), PlatformError> {
    sqlx::query(
        "INSERT INTO projection.active_view (tenant_id, device_view, channel_view, generation, updated_at)
         VALUES ($1, 'devices', 'channels', 1, $2)
         ON CONFLICT (tenant_id) DO NOTHING",
    )
    .bind(tenant_uuid)
    .bind(Utc::now())
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;
    Ok(())
}

fn validate_table_name(name: &str) -> Result<(), PlatformError> {
    match name {
        DEFAULT_DEVICE_VIEW | SHADOW_DEVICE_VIEW | DEFAULT_CHANNEL_VIEW | SHADOW_CHANNEL_VIEW => {
            Ok(())
        }
        _ => Err(PlatformError::new(
            ErrorCode::Invalid,
            format!("invalid projection table name: {name}"),
        )),
    }
}

fn other_view(view: &str) -> Result<&'static str, PlatformError> {
    match view {
        DEFAULT_DEVICE_VIEW => Ok(SHADOW_DEVICE_VIEW),
        SHADOW_DEVICE_VIEW => Ok(DEFAULT_DEVICE_VIEW),
        DEFAULT_CHANNEL_VIEW => Ok(SHADOW_CHANNEL_VIEW),
        SHADOW_CHANNEL_VIEW => Ok(DEFAULT_CHANNEL_VIEW),
        _ => Err(PlatformError::new(
            ErrorCode::Invalid,
            format!("invalid projection view: {view}"),
        )),
    }
}

async fn fetch_device_projection_sql(
    tx: &mut sqlx::postgres::PgConnection,
    table: &str,
    tenant_uuid: Uuid,
    external_ref: &str,
) -> Result<Option<DeviceProjection>, PlatformError> {
    let sql = format!(
        "SELECT external_ref, sequence, source_event_id, observed_at, payload, stale
         FROM projection.{table}
         WHERE tenant_id = $1 AND external_ref = $2"
    );
    let row = sqlx::query(AssertSqlSafe(sql))
        .bind(tenant_uuid)
        .bind(external_ref)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

    row.map(row_to_device_projection).transpose()
}

async fn insert_device_projection(
    tx: &mut sqlx::postgres::PgConnection,
    table: &str,
    tenant_uuid: Uuid,
    event: &DeviceEvent,
    stale: bool,
) -> Result<(), PlatformError> {
    let sql = format!(
        "INSERT INTO projection.{table}
         (tenant_id, external_ref, sequence, source_event_id, observed_at, payload, stale)
         VALUES ($1, $2, $3, $4, $5, $6, $7)"
    );
    sqlx::query(AssertSqlSafe(sql))
        .bind(tenant_uuid)
        .bind(&event.external_ref)
        .bind(event.sequence)
        .bind(&event.source_event_id)
        .bind(utc_to_db(event.observed_at))
        .bind(&event.payload)
        .bind(stale)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    Ok(())
}

async fn upsert_device_projection(
    tx: &mut sqlx::postgres::PgConnection,
    table: &str,
    tenant_uuid: Uuid,
    event: &DeviceEvent,
    stale: bool,
) -> Result<(), PlatformError> {
    let sql = format!(
        "INSERT INTO projection.{table}
         (tenant_id, external_ref, sequence, source_event_id, observed_at, payload, stale)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (tenant_id, external_ref) DO UPDATE
         SET sequence = EXCLUDED.sequence,
             source_event_id = EXCLUDED.source_event_id,
             observed_at = EXCLUDED.observed_at,
             payload = EXCLUDED.payload,
             stale = EXCLUDED.stale"
    );
    sqlx::query(AssertSqlSafe(sql))
        .bind(tenant_uuid)
        .bind(&event.external_ref)
        .bind(event.sequence)
        .bind(&event.source_event_id)
        .bind(utc_to_db(event.observed_at))
        .bind(&event.payload)
        .bind(stale)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    Ok(())
}

fn row_to_device_projection(row: sqlx::postgres::PgRow) -> Result<DeviceProjection, PlatformError> {
    let external_ref: String = row.try_get("external_ref").map_err(db_error)?;
    let sequence: i64 = row.try_get("sequence").map_err(db_error)?;
    let source_event_id: String = row.try_get("source_event_id").map_err(db_error)?;
    let observed_at: DateTime<Utc> = row.try_get("observed_at").map_err(db_error)?;
    let payload: String = row.try_get("payload").map_err(db_error)?;
    let stale: bool = row.try_get("stale").map_err(db_error)?;
    Ok(DeviceProjection {
        external_ref,
        sequence,
        source_event_id,
        observed_at: observed_at.into(),
        payload,
        stale,
    })
}

async fn fetch_channel_projection_sql(
    tx: &mut sqlx::postgres::PgConnection,
    table: &str,
    tenant_uuid: Uuid,
    external_ref: &str,
) -> Result<Option<ChannelProjection>, PlatformError> {
    let sql = format!(
        "SELECT external_ref, sequence, source_event_id, observed_at, payload, stale
         FROM projection.{table}
         WHERE tenant_id = $1 AND external_ref = $2"
    );
    let row = sqlx::query(AssertSqlSafe(sql))
        .bind(tenant_uuid)
        .bind(external_ref)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

    row.map(row_to_channel_projection).transpose()
}

async fn insert_channel_projection(
    tx: &mut sqlx::postgres::PgConnection,
    table: &str,
    tenant_uuid: Uuid,
    event: &ChannelEvent,
    stale: bool,
) -> Result<(), PlatformError> {
    let sql = format!(
        "INSERT INTO projection.{table}
         (tenant_id, external_ref, sequence, source_event_id, observed_at, payload, stale)
         VALUES ($1, $2, $3, $4, $5, $6, $7)"
    );
    sqlx::query(AssertSqlSafe(sql))
        .bind(tenant_uuid)
        .bind(&event.external_ref)
        .bind(event.sequence)
        .bind(&event.source_event_id)
        .bind(utc_to_db(event.observed_at))
        .bind(&event.payload)
        .bind(stale)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    Ok(())
}

async fn upsert_channel_projection(
    tx: &mut sqlx::postgres::PgConnection,
    table: &str,
    tenant_uuid: Uuid,
    event: &ChannelEvent,
    stale: bool,
) -> Result<(), PlatformError> {
    let sql = format!(
        "INSERT INTO projection.{table}
         (tenant_id, external_ref, sequence, source_event_id, observed_at, payload, stale)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (tenant_id, external_ref) DO UPDATE
         SET sequence = EXCLUDED.sequence,
             source_event_id = EXCLUDED.source_event_id,
             observed_at = EXCLUDED.observed_at,
             payload = EXCLUDED.payload,
             stale = EXCLUDED.stale"
    );
    sqlx::query(AssertSqlSafe(sql))
        .bind(tenant_uuid)
        .bind(&event.external_ref)
        .bind(event.sequence)
        .bind(&event.source_event_id)
        .bind(utc_to_db(event.observed_at))
        .bind(&event.payload)
        .bind(stale)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    Ok(())
}

fn row_to_channel_projection(
    row: sqlx::postgres::PgRow,
) -> Result<ChannelProjection, PlatformError> {
    let external_ref: String = row.try_get("external_ref").map_err(db_error)?;
    let sequence: i64 = row.try_get("sequence").map_err(db_error)?;
    let source_event_id: String = row.try_get("source_event_id").map_err(db_error)?;
    let observed_at: DateTime<Utc> = row.try_get("observed_at").map_err(db_error)?;
    let payload: String = row.try_get("payload").map_err(db_error)?;
    let stale: bool = row.try_get("stale").map_err(db_error)?;
    Ok(ChannelProjection {
        external_ref,
        sequence,
        source_event_id,
        observed_at: observed_at.into(),
        payload,
        stale,
    })
}

async fn record_failure_sql(
    tx: &mut sqlx::postgres::PgConnection,
    tenant_uuid: Uuid,
    source_event_id: &str,
    external_ref: &str,
    reason: &str,
    payload: &str,
) -> Result<(), PlatformError> {
    let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
    let id = generator.generate()?;
    sqlx::query(
        "INSERT INTO projection.failures
         (id, tenant_id, source_event_id, external_ref, reason, payload, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(id)
    .bind(tenant_uuid)
    .bind(source_event_id)
    .bind(external_ref)
    .bind(reason)
    .bind(payload)
    .bind(Utc::now())
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;
    Ok(())
}

fn utc_to_db(ts: UtcTimestamp) -> DateTime<Utc> {
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
