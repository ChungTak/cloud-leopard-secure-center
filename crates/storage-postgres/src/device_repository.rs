//! PostgreSQL implementation of the `DeviceRepository` port.

use async_trait::async_trait;
use domain_resource::device::{DeviceLifecycle, ManagedDevice, OnlineState};
use foundation::{
    AreaId, DeviceId, ErrorCode, OrganizationId, PlatformError, RequestContext, Revision, TenantId,
    UserId, UtcTimestamp,
    chrono::{DateTime, Utc},
    uuid::Uuid,
};
use sqlx::{PgPool, Row};
use storage_api::{DeviceRepository, Page};

use crate::begin_tenant_transaction;

/// PostgreSQL-backed managed device repository.
#[derive(Debug, Clone)]
pub struct PostgresDeviceRepository {
    pool: PgPool,
}

impl PostgresDeviceRepository {
    /// Create a new repository backed by `pool`.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DeviceRepository for PostgresDeviceRepository {
    async fn by_id(
        &self,
        id: DeviceId,
        ctx: &RequestContext,
    ) -> Result<ManagedDevice, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let row = sqlx::query(
            "SELECT id, tenant_id, organization_id, area_id, code, name, serial,
                    lifecycle, online_state, revision, created_at, updated_at, actor
             FROM resource.managed_devices
             WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        let device = match row {
            Some(row) => row_to_device(row)?,
            None => {
                return Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "managed device not found".to_string(),
                ));
            }
        };
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(device)
    }

    async fn create(
        &self,
        device: &ManagedDevice,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let tenant_uuid = ctx
            .tenant_id
            .map(|t| *t.as_uuid())
            .ok_or_else(missing_tenant)?;

        if *device.tenant_id.as_uuid() != tenant_uuid {
            return Err(PlatformError::new(
                ErrorCode::Invalid,
                "device tenant does not match context".to_string(),
            ));
        }

        sqlx::query(
            "INSERT INTO resource.managed_devices
             (id, tenant_id, organization_id, area_id, code, name, serial,
              lifecycle, online_state, revision, created_at, updated_at, actor, deleted_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, NULL)",
        )
        .bind(device.id.as_uuid())
        .bind(tenant_uuid)
        .bind(device.organization_id.map(|o| *o.as_uuid()))
        .bind(device.area_id.map(|a| *a.as_uuid()))
        .bind(&device.code)
        .bind(&device.name)
        .bind(&device.serial)
        .bind(device.lifecycle.as_str())
        .bind(device.online_state.as_str())
        .bind(device.revision.value() as i64)
        .bind(utc_to_db(device.created_at))
        .bind(utc_to_db(device.updated_at))
        .bind(device.actor.map(|a| *a.as_uuid()))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn update(
        &self,
        device: &ManagedDevice,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let current: Option<i64> = sqlx::query_scalar(
            "SELECT revision FROM resource.managed_devices WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(device.id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        match current {
            None => {
                return Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "managed device not found".to_string(),
                ));
            }
            Some(rev) if rev != expected.value() as i64 => {
                return Err(PlatformError::new(
                    ErrorCode::VersionMismatch,
                    "revision conflict".to_string(),
                ));
            }
            Some(_) => {}
        }

        let rows = sqlx::query(
            "UPDATE resource.managed_devices
             SET organization_id = $1, area_id = $2, code = $3, name = $4, serial = $5,
                 lifecycle = $6, online_state = $7, revision = $8, updated_at = $9, actor = $10
             WHERE id = $11 AND revision = $12 AND deleted_at IS NULL",
        )
        .bind(device.organization_id.map(|o| *o.as_uuid()))
        .bind(device.area_id.map(|a| *a.as_uuid()))
        .bind(&device.code)
        .bind(&device.name)
        .bind(&device.serial)
        .bind(device.lifecycle.as_str())
        .bind(device.online_state.as_str())
        .bind(device.revision.value() as i64)
        .bind(utc_to_db(device.updated_at))
        .bind(device.actor.map(|a| *a.as_uuid()))
        .bind(device.id.as_uuid())
        .bind(expected.value() as i64)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?
        .rows_affected();

        if rows == 0 {
            return Err(PlatformError::new(
                ErrorCode::VersionMismatch,
                "revision conflict".to_string(),
            ));
        }

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn delete(
        &self,
        id: DeviceId,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let current: Option<i64> = sqlx::query_scalar(
            "SELECT revision FROM resource.managed_devices WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        match current {
            None => {
                return Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "managed device not found".to_string(),
                ));
            }
            Some(rev) if rev != expected.value() as i64 => {
                return Err(PlatformError::new(
                    ErrorCode::VersionMismatch,
                    "revision conflict".to_string(),
                ));
            }
            Some(_) => {}
        }

        let rows = sqlx::query(
            "UPDATE resource.managed_devices
             SET deleted_at = $1, revision = $2
             WHERE id = $3 AND revision = $4 AND deleted_at IS NULL",
        )
        .bind(Utc::now())
        .bind(expected.value() as i64 + 1)
        .bind(id.as_uuid())
        .bind(expected.value() as i64)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?
        .rows_affected();

        if rows == 0 {
            return Err(PlatformError::new(
                ErrorCode::VersionMismatch,
                "revision conflict".to_string(),
            ));
        }

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn list(&self, ctx: &RequestContext) -> Result<Page<ManagedDevice>, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let rows = sqlx::query(
            "SELECT id, tenant_id, organization_id, area_id, code, name, serial,
                    lifecycle, online_state, revision, created_at, updated_at, actor
             FROM resource.managed_devices
             WHERE deleted_at IS NULL
             ORDER BY code
             LIMIT 100",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(db_error)?;

        let items = rows
            .into_iter()
            .map(row_to_device)
            .collect::<Result<Vec<_>, _>>()?;

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(Page {
            items,
            next_cursor: None,
        })
    }
}

fn row_to_device(row: sqlx::postgres::PgRow) -> Result<ManagedDevice, PlatformError> {
    let id: Uuid = row.try_get("id").map_err(db_error)?;
    let tenant_id: Uuid = row.try_get("tenant_id").map_err(db_error)?;
    let organization_id: Option<Uuid> = row.try_get("organization_id").map_err(db_error)?;
    let area_id: Option<Uuid> = row.try_get("area_id").map_err(db_error)?;
    let code: String = row.try_get("code").map_err(db_error)?;
    let name: String = row.try_get("name").map_err(db_error)?;
    let serial: Option<String> = row.try_get("serial").map_err(db_error)?;
    let lifecycle: String = row.try_get("lifecycle").map_err(db_error)?;
    let online_state: String = row.try_get("online_state").map_err(db_error)?;
    let revision: i64 = row.try_get("revision").map_err(db_error)?;
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(db_error)?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at").map_err(db_error)?;
    let actor: Option<Uuid> = row.try_get("actor").map_err(db_error)?;

    ManagedDevice::from_parts(
        DeviceId::parse_str(&id.to_string())?,
        TenantId::parse_str(&tenant_id.to_string())?,
        organization_id
            .map(|o| OrganizationId::parse_str(&o.to_string()))
            .transpose()?,
        area_id
            .map(|a| AreaId::parse_str(&a.to_string()))
            .transpose()?,
        code,
        name,
        serial,
        DeviceLifecycle::parse(&lifecycle)?,
        OnlineState::parse(&online_state)?,
        Revision::new(revision as u64),
        created_at.into(),
        updated_at.into(),
        actor
            .map(|a| UserId::parse_str(&a.to_string()))
            .transpose()?,
    )
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
