//! PostgreSQL implementation of the `CameraRepository` port.

use async_trait::async_trait;
use domain_resource::camera::{Camera, Sensitivity};
use foundation::{
    AreaId, CameraId, DeviceId, ErrorCode, PlatformError, RequestContext, Revision, TenantId,
    UserId, UtcTimestamp,
    chrono::{DateTime, Utc},
    uuid::Uuid,
};
use sqlx::{PgPool, Row};
use storage_api::{CameraRepository, ListOptions, Page};

use crate::{begin_tenant_transaction, db_error, paginate, revision_from_i64};

/// PostgreSQL-backed camera repository.
#[derive(Debug, Clone)]
pub struct PostgresCameraRepository {
    pool: PgPool,
}

impl PostgresCameraRepository {
    /// Create a new repository backed by `pool`.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CameraRepository for PostgresCameraRepository {
    async fn by_id(&self, id: CameraId, ctx: &RequestContext) -> Result<Camera, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let row = sqlx::query(
            "SELECT id, tenant_id, device_id, area_id, code, name, sensitivity,
                    is_enabled, revision, created_at, updated_at, actor
             FROM resource.cameras
             WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        let camera = match row {
            Some(row) => row_to_camera(row)?,
            None => {
                return Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "camera not found".to_string(),
                ));
            }
        };
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(camera)
    }

    async fn create(&self, camera: &Camera, ctx: &RequestContext) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let tenant_uuid = ctx
            .tenant_id
            .map(|t| *t.as_uuid())
            .ok_or_else(missing_tenant)?;

        if *camera.tenant_id.as_uuid() != tenant_uuid {
            return Err(PlatformError::new(
                ErrorCode::Invalid,
                "camera tenant does not match context".to_string(),
            ));
        }

        sqlx::query(
            "INSERT INTO resource.cameras
             (id, tenant_id, device_id, area_id, code, name, sensitivity,
              is_enabled, revision, created_at, updated_at, actor, deleted_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, NULL)",
        )
        .bind(camera.id.as_uuid())
        .bind(tenant_uuid)
        .bind(camera.device_id.as_uuid())
        .bind(camera.area_id.map(|a| *a.as_uuid()))
        .bind(&camera.code)
        .bind(&camera.name)
        .bind(camera.sensitivity.as_str())
        .bind(camera.is_enabled)
        .bind(camera.revision.value() as i64)
        .bind(utc_to_db(camera.created_at))
        .bind(utc_to_db(camera.updated_at))
        .bind(camera.actor.map(|a| *a.as_uuid()))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn update(
        &self,
        camera: &Camera,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let current: Option<i64> = sqlx::query_scalar(
            "SELECT revision FROM resource.cameras WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(camera.id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        match current {
            None => {
                return Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "camera not found".to_string(),
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
            "UPDATE resource.cameras
             SET device_id = $1, area_id = $2, code = $3, name = $4, sensitivity = $5,
                 is_enabled = $6, revision = $7, updated_at = $8, actor = $9
             WHERE id = $10 AND revision = $11 AND deleted_at IS NULL",
        )
        .bind(camera.device_id.as_uuid())
        .bind(camera.area_id.map(|a| *a.as_uuid()))
        .bind(&camera.code)
        .bind(&camera.name)
        .bind(camera.sensitivity.as_str())
        .bind(camera.is_enabled)
        .bind(camera.revision.value() as i64)
        .bind(utc_to_db(camera.updated_at))
        .bind(camera.actor.map(|a| *a.as_uuid()))
        .bind(camera.id.as_uuid())
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
        id: CameraId,
        expected: Revision,
        deleted_at: UtcTimestamp,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let current: Option<i64> = sqlx::query_scalar(
            "SELECT revision FROM resource.cameras WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        match current {
            None => {
                return Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "camera not found".to_string(),
                ));
            }
            Some(rev) if rev != expected.to_i64()? => {
                return Err(PlatformError::new(
                    ErrorCode::VersionMismatch,
                    "revision conflict".to_string(),
                ));
            }
            Some(_) => {}
        }

        let deleted = utc_to_db(deleted_at);
        let rows = sqlx::query(
            "UPDATE resource.cameras
             SET deleted_at = $1, updated_at = $1, revision = $2, actor = $3
             WHERE id = $4 AND revision = $5 AND deleted_at IS NULL",
        )
        .bind(deleted)
        .bind(expected.next_i64()?)
        .bind(ctx.actor_id.map(|a| *a.as_uuid()))
        .bind(id.as_uuid())
        .bind(expected.to_i64()?)
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

    async fn list_by_device(
        &self,
        device_id: DeviceId,
        ctx: &RequestContext,
        options: ListOptions,
    ) -> Result<Page<Camera>, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let rows = sqlx::query(
            "SELECT id, tenant_id, device_id, area_id, code, name, sensitivity,
                    is_enabled, revision, created_at, updated_at, actor
             FROM resource.cameras
             WHERE device_id = $1 AND deleted_at IS NULL
             ORDER BY code
             LIMIT $2 OFFSET $3",
        )
        .bind(device_id.as_uuid())
        .bind((options.validate().limit as i64) + 1)
        .bind(options.validate().offset as i64)
        .fetch_all(&mut *tx)
        .await
        .map_err(db_error)?;

        let items = rows
            .into_iter()
            .map(row_to_camera)
            .collect::<Result<Vec<_>, _>>()?;

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(paginate(items, options))
    }
}

fn row_to_camera(row: sqlx::postgres::PgRow) -> Result<Camera, PlatformError> {
    let id: Uuid = row.try_get("id").map_err(db_error)?;
    let tenant_id: Uuid = row.try_get("tenant_id").map_err(db_error)?;
    let device_id: Uuid = row.try_get("device_id").map_err(db_error)?;
    let area_id: Option<Uuid> = row.try_get("area_id").map_err(db_error)?;
    let code: String = row.try_get("code").map_err(db_error)?;
    let name: String = row.try_get("name").map_err(db_error)?;
    let sensitivity: String = row.try_get("sensitivity").map_err(db_error)?;
    let is_enabled: bool = row.try_get("is_enabled").map_err(db_error)?;
    let revision: i64 = row.try_get("revision").map_err(db_error)?;
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(db_error)?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at").map_err(db_error)?;
    let actor: Option<Uuid> = row.try_get("actor").map_err(db_error)?;

    Camera::from_parts(
        CameraId::parse_str(&id.to_string())?,
        TenantId::parse_str(&tenant_id.to_string())?,
        DeviceId::parse_str(&device_id.to_string())?,
        area_id
            .map(|a| AreaId::parse_str(&a.to_string()))
            .transpose()?,
        code,
        name,
        Sensitivity::parse(&sensitivity)?,
        is_enabled,
        revision_from_i64(revision)?,
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

fn missing_tenant() -> PlatformError {
    PlatformError::new(
        ErrorCode::Invalid,
        "tenant_id is required in request context".to_string(),
    )
}
