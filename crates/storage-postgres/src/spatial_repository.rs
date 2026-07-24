//! PostgreSQL implementation of the `SpatialRepository` port.

use async_trait::async_trait;
use domain_organization::spatial::{Area, Building, Floor, Site};
use foundation::{
    AreaId, BuildingId, ErrorCode, FloorId, OrganizationId, PlatformError, RequestContext,
    Revision, SiteId, TenantId, UserId, UtcTimestamp,
    chrono::{DateTime, Utc},
    uuid::Uuid,
};
use sqlx::{PgPool, Row};
use storage_api::{ListOptions, Page, SpatialRepository};

use crate::{begin_tenant_transaction, db_error, paginate, revision_from_i64};

/// PostgreSQL-backed spatial repository.
#[derive(Debug, Clone)]
pub struct PostgresSpatialRepository {
    pool: PgPool,
}

impl PostgresSpatialRepository {
    /// Create a new repository backed by `pool`.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SpatialRepository for PostgresSpatialRepository {
    async fn site_by_id(&self, id: SiteId, ctx: &RequestContext) -> Result<Site, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let row = sqlx::query(
            "SELECT id, tenant_id, organization_unit_id, code, name, address, timezone,
                    revision, created_at, updated_at, actor
             FROM org.sites
             WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;
        let site = row
            .map(site_row_to_site)
            .transpose()?
            .ok_or_else(|| PlatformError::new(ErrorCode::NotFound, "site not found".to_string()))?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(site)
    }

    async fn create_site(&self, site: &Site, ctx: &RequestContext) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        if let Some(ou_id) = site.organization_unit_id {
            let exists: Option<(Uuid,)> = sqlx::query_as(
                "SELECT id FROM org.organization_units WHERE id = $1 AND deleted_at IS NULL",
            )
            .bind(ou_id.as_uuid())
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_error)?;
            if exists.is_none() {
                return Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "organization unit not found".to_string(),
                ));
            }
        }
        sqlx::query(
            "INSERT INTO org.sites
             (id, tenant_id, organization_unit_id, code, name, address, timezone,
              revision, created_at, updated_at, actor, deleted_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, NULL)",
        )
        .bind(site.id.as_uuid())
        .bind(site.tenant_id.as_uuid())
        .bind(site.organization_unit_id.map(|o| *o.as_uuid()))
        .bind(&site.code)
        .bind(&site.name)
        .bind(&site.address)
        .bind(&site.timezone)
        .bind(site.revision.value() as i64)
        .bind(utc_to_db(site.created_at))
        .bind(utc_to_db(site.updated_at))
        .bind(site.actor.map(|a| *a.as_uuid()))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn update_site(
        &self,
        site: &Site,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        if let Some(ou_id) = site.organization_unit_id {
            let exists: Option<(Uuid,)> = sqlx::query_as(
                "SELECT id FROM org.organization_units WHERE id = $1 AND deleted_at IS NULL",
            )
            .bind(ou_id.as_uuid())
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_error)?;
            if exists.is_none() {
                return Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "organization unit not found".to_string(),
                ));
            }
        }
        let rows = sqlx::query(
            "UPDATE org.sites
             SET organization_unit_id = $1, code = $2, name = $3, address = $4, timezone = $5,
                 revision = $6, updated_at = $7, actor = $8
             WHERE id = $9 AND revision = $10 AND deleted_at IS NULL",
        )
        .bind(site.organization_unit_id.map(|o| *o.as_uuid()))
        .bind(&site.code)
        .bind(&site.name)
        .bind(&site.address)
        .bind(&site.timezone)
        .bind(site.revision.value() as i64)
        .bind(utc_to_db(site.updated_at))
        .bind(site.actor.map(|a| *a.as_uuid()))
        .bind(site.id.as_uuid())
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

    async fn delete_site(
        &self,
        id: SiteId,
        expected: Revision,
        deleted_at: UtcTimestamp,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let tenant_uuid = ctx
            .tenant_id
            .map(|t| *t.as_uuid())
            .ok_or_else(missing_tenant)?;
        let deps: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM org.buildings
             WHERE tenant_id = $1 AND site_id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_uuid)
        .bind(id.as_uuid())
        .fetch_one(&mut *tx)
        .await
        .map_err(db_error)?;
        if deps.0 > 0 {
            return Err(PlatformError::new(
                ErrorCode::Conflict,
                "site has buildings".to_string(),
            ));
        }
        let deleted = utc_to_db(deleted_at);
        let rows = sqlx::query(
            "UPDATE org.sites SET deleted_at = $1, updated_at = $1, revision = $2, actor = $3
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

    async fn list_sites(
        &self,
        ctx: &RequestContext,
        options: ListOptions,
    ) -> Result<Page<Site>, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let rows = sqlx::query(
            "SELECT id, tenant_id, organization_unit_id, code, name, address, timezone,
                    revision, created_at, updated_at, actor
             FROM org.sites
             WHERE deleted_at IS NULL
             ORDER BY code
             LIMIT $1 OFFSET $2",
        )
        .bind((options.validate().limit as i64) + 1)
        .bind(options.validate().offset as i64)
        .fetch_all(&mut *tx)
        .await
        .map_err(db_error)?;
        let items = rows
            .into_iter()
            .map(site_row_to_site)
            .collect::<Result<Vec<_>, _>>()?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(paginate(items, options))
    }

    async fn building_by_id(
        &self,
        id: BuildingId,
        ctx: &RequestContext,
    ) -> Result<Building, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let row = sqlx::query(
            "SELECT id, tenant_id, site_id, code, name, revision, created_at, updated_at, actor
             FROM org.buildings
             WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;
        let building = row
            .map(building_row_to_building)
            .transpose()?
            .ok_or_else(|| {
                PlatformError::new(ErrorCode::NotFound, "building not found".to_string())
            })?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(building)
    }

    async fn create_building(
        &self,
        building: &Building,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let site: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM org.sites WHERE id = $1 AND deleted_at IS NULL")
                .bind(building.site_id.as_uuid())
                .fetch_optional(&mut *tx)
                .await
                .map_err(db_error)?;
        if site.is_none() {
            return Err(PlatformError::new(
                ErrorCode::NotFound,
                "site not found".to_string(),
            ));
        }
        sqlx::query(
            "INSERT INTO org.buildings
             (id, tenant_id, site_id, code, name, revision, created_at, updated_at, actor, deleted_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NULL)",
        )
        .bind(building.id.as_uuid())
        .bind(building.tenant_id.as_uuid())
        .bind(building.site_id.as_uuid())
        .bind(&building.code)
        .bind(&building.name)
        .bind(building.revision.value() as i64)
        .bind(utc_to_db(building.created_at))
        .bind(utc_to_db(building.updated_at))
        .bind(building.actor.map(|a| *a.as_uuid()))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn update_building(
        &self,
        building: &Building,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let site: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM org.sites WHERE id = $1 AND deleted_at IS NULL")
                .bind(building.site_id.as_uuid())
                .fetch_optional(&mut *tx)
                .await
                .map_err(db_error)?;
        if site.is_none() {
            return Err(PlatformError::new(
                ErrorCode::NotFound,
                "site not found".to_string(),
            ));
        }
        let rows = sqlx::query(
            "UPDATE org.buildings
             SET site_id = $1, code = $2, name = $3, revision = $4, updated_at = $5, actor = $6
             WHERE id = $7 AND revision = $8 AND deleted_at IS NULL",
        )
        .bind(building.site_id.as_uuid())
        .bind(&building.code)
        .bind(&building.name)
        .bind(building.revision.value() as i64)
        .bind(utc_to_db(building.updated_at))
        .bind(building.actor.map(|a| *a.as_uuid()))
        .bind(building.id.as_uuid())
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

    async fn delete_building(
        &self,
        id: BuildingId,
        expected: Revision,
        deleted_at: UtcTimestamp,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let tenant_uuid = ctx
            .tenant_id
            .map(|t| *t.as_uuid())
            .ok_or_else(missing_tenant)?;
        let deps: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM org.floors
             WHERE tenant_id = $1 AND building_id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_uuid)
        .bind(id.as_uuid())
        .fetch_one(&mut *tx)
        .await
        .map_err(db_error)?;
        if deps.0 > 0 {
            return Err(PlatformError::new(
                ErrorCode::Conflict,
                "building has floors".to_string(),
            ));
        }
        let deleted = utc_to_db(deleted_at);
        let rows = sqlx::query(
            "UPDATE org.buildings SET deleted_at = $1, updated_at = $1, revision = $2, actor = $3
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

    async fn list_buildings(
        &self,
        ctx: &RequestContext,
        options: ListOptions,
    ) -> Result<Page<Building>, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let rows = sqlx::query(
            "SELECT id, tenant_id, site_id, code, name, revision, created_at, updated_at, actor
             FROM org.buildings
             WHERE deleted_at IS NULL
             ORDER BY code
             LIMIT $1 OFFSET $2",
        )
        .bind((options.validate().limit as i64) + 1)
        .bind(options.validate().offset as i64)
        .fetch_all(&mut *tx)
        .await
        .map_err(db_error)?;
        let items = rows
            .into_iter()
            .map(building_row_to_building)
            .collect::<Result<Vec<_>, _>>()?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(paginate(items, options))
    }

    async fn floor_by_id(&self, id: FloorId, ctx: &RequestContext) -> Result<Floor, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let row = sqlx::query(
            "SELECT id, tenant_id, building_id, code, name, level,
                    revision, created_at, updated_at, actor
             FROM org.floors
             WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;
        let floor = row.map(floor_row_to_floor).transpose()?.ok_or_else(|| {
            PlatformError::new(ErrorCode::NotFound, "floor not found".to_string())
        })?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(floor)
    }

    async fn create_floor(&self, floor: &Floor, ctx: &RequestContext) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let building: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM org.buildings WHERE id = $1 AND deleted_at IS NULL")
                .bind(floor.building_id.as_uuid())
                .fetch_optional(&mut *tx)
                .await
                .map_err(db_error)?;
        if building.is_none() {
            return Err(PlatformError::new(
                ErrorCode::NotFound,
                "building not found".to_string(),
            ));
        }
        sqlx::query(
            "INSERT INTO org.floors
             (id, tenant_id, building_id, code, name, level,
              revision, created_at, updated_at, actor, deleted_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NULL)",
        )
        .bind(floor.id.as_uuid())
        .bind(floor.tenant_id.as_uuid())
        .bind(floor.building_id.as_uuid())
        .bind(&floor.code)
        .bind(&floor.name)
        .bind(floor.level)
        .bind(floor.revision.value() as i64)
        .bind(utc_to_db(floor.created_at))
        .bind(utc_to_db(floor.updated_at))
        .bind(floor.actor.map(|a| *a.as_uuid()))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn update_floor(
        &self,
        floor: &Floor,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let building: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM org.buildings WHERE id = $1 AND deleted_at IS NULL")
                .bind(floor.building_id.as_uuid())
                .fetch_optional(&mut *tx)
                .await
                .map_err(db_error)?;
        if building.is_none() {
            return Err(PlatformError::new(
                ErrorCode::NotFound,
                "building not found".to_string(),
            ));
        }
        let rows = sqlx::query(
            "UPDATE org.floors
             SET building_id = $1, code = $2, name = $3, level = $4,
                 revision = $5, updated_at = $6, actor = $7
             WHERE id = $8 AND revision = $9 AND deleted_at IS NULL",
        )
        .bind(floor.building_id.as_uuid())
        .bind(&floor.code)
        .bind(&floor.name)
        .bind(floor.level)
        .bind(floor.revision.value() as i64)
        .bind(utc_to_db(floor.updated_at))
        .bind(floor.actor.map(|a| *a.as_uuid()))
        .bind(floor.id.as_uuid())
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

    async fn delete_floor(
        &self,
        id: FloorId,
        expected: Revision,
        deleted_at: UtcTimestamp,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let tenant_uuid = ctx
            .tenant_id
            .map(|t| *t.as_uuid())
            .ok_or_else(missing_tenant)?;
        let deps: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM org.areas
             WHERE tenant_id = $1 AND floor_id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_uuid)
        .bind(id.as_uuid())
        .fetch_one(&mut *tx)
        .await
        .map_err(db_error)?;
        if deps.0 > 0 {
            return Err(PlatformError::new(
                ErrorCode::Conflict,
                "floor has areas".to_string(),
            ));
        }
        let deleted = utc_to_db(deleted_at);
        let rows = sqlx::query(
            "UPDATE org.floors SET deleted_at = $1, updated_at = $1, revision = $2, actor = $3
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

    async fn list_floors(
        &self,
        ctx: &RequestContext,
        options: ListOptions,
    ) -> Result<Page<Floor>, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let rows = sqlx::query(
            "SELECT id, tenant_id, building_id, code, name, level,
                    revision, created_at, updated_at, actor
             FROM org.floors
             WHERE deleted_at IS NULL
             ORDER BY code
             LIMIT $1 OFFSET $2",
        )
        .bind((options.validate().limit as i64) + 1)
        .bind(options.validate().offset as i64)
        .fetch_all(&mut *tx)
        .await
        .map_err(db_error)?;
        let items = rows
            .into_iter()
            .map(floor_row_to_floor)
            .collect::<Result<Vec<_>, _>>()?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(paginate(items, options))
    }

    async fn area_by_id(&self, id: AreaId, ctx: &RequestContext) -> Result<Area, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let row = sqlx::query(
            "SELECT id, tenant_id, floor_id, parent_id, code, name,
                    coordinate_system, latitude, longitude, altitude,
                    revision, created_at, updated_at, actor
             FROM org.areas
             WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;
        let area = row
            .map(area_row_to_area)
            .transpose()?
            .ok_or_else(|| PlatformError::new(ErrorCode::NotFound, "area not found".to_string()))?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(area)
    }

    async fn create_area(&self, area: &Area, ctx: &RequestContext) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        validate_area_references(&mut *tx, area).await?;

        sqlx::query(
            "INSERT INTO org.areas
             (id, tenant_id, floor_id, parent_id, code, name,
              coordinate_system, latitude, longitude, altitude,
              revision, created_at, updated_at, actor, deleted_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, NULL)",
        )
        .bind(area.id.as_uuid())
        .bind(area.tenant_id.as_uuid())
        .bind(area.floor_id.map(|f| *f.as_uuid()))
        .bind(area.parent_id.map(|p| *p.as_uuid()))
        .bind(&area.code)
        .bind(&area.name)
        .bind(&area.coordinate_system)
        .bind(area.latitude)
        .bind(area.longitude)
        .bind(area.altitude)
        .bind(area.revision.value() as i64)
        .bind(utc_to_db(area.created_at))
        .bind(utc_to_db(area.updated_at))
        .bind(area.actor.map(|a| *a.as_uuid()))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

        let tenant_uuid = area.tenant_id.as_uuid();
        let area_uuid = area.id.as_uuid();
        sqlx::query(
            "INSERT INTO org.area_closure (tenant_id, ancestor_id, descendant_id, depth)
             VALUES ($1, $2, $2, 0)",
        )
        .bind(*tenant_uuid)
        .bind(area_uuid)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

        if let Some(parent_id) = area.parent_id {
            sqlx::query(
                "INSERT INTO org.area_closure
                     (tenant_id, ancestor_id, descendant_id, depth)
                 SELECT $1, ancestor_id, $2, depth + 1
                 FROM org.area_closure
                 WHERE tenant_id = $1 AND descendant_id = $3",
            )
            .bind(*tenant_uuid)
            .bind(area_uuid)
            .bind(parent_id.as_uuid())
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        }

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn update_area(
        &self,
        area: &Area,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let current: Option<(i64, Option<Uuid>)> = sqlx::query_as(
            "SELECT revision, parent_id FROM org.areas WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(area.id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        let (current_revision, old_parent_uuid) = match current {
            None => {
                return Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "area not found".to_string(),
                ));
            }
            Some((rev, _parent)) if rev != expected.value() as i64 => {
                return Err(PlatformError::new(
                    ErrorCode::VersionMismatch,
                    "revision conflict".to_string(),
                ));
            }
            Some((rev, parent)) => (rev, parent),
        };

        validate_area_references(&mut *tx, area).await?;

        let new_parent_uuid = area.parent_id.map(|p| *p.as_uuid());
        if new_parent_uuid != old_parent_uuid {
            if let Some(parent_uuid) = new_parent_uuid {
                let is_descendant: Option<(Uuid,)> = sqlx::query_as(
                    "SELECT 1 FROM org.area_closure
                     WHERE tenant_id = $1 AND ancestor_id = $2 AND descendant_id = $3",
                )
                .bind(area.tenant_id.as_uuid())
                .bind(area.id.as_uuid())
                .bind(parent_uuid)
                .fetch_optional(&mut *tx)
                .await
                .map_err(db_error)?;
                if is_descendant.is_some() {
                    return Err(PlatformError::invalid(
                        "parent_id",
                        "cannot move an area under one of its descendants",
                    ));
                }

                update_area_closure(
                    &mut *tx,
                    area.tenant_id.as_uuid(),
                    area.id.as_uuid(),
                    Some(parent_uuid),
                )
                .await?;
            } else {
                update_area_closure(&mut *tx, area.tenant_id.as_uuid(), area.id.as_uuid(), None)
                    .await?;
            }
        }

        let rows = sqlx::query(
            "UPDATE org.areas
             SET floor_id = $1, parent_id = $2, code = $3, name = $4,
                 coordinate_system = $5, latitude = $6, longitude = $7, altitude = $8,
                 revision = $9, updated_at = $10, actor = $11
             WHERE id = $12 AND revision = $13 AND deleted_at IS NULL",
        )
        .bind(area.floor_id.map(|f| *f.as_uuid()))
        .bind(area.parent_id.map(|p| *p.as_uuid()))
        .bind(&area.code)
        .bind(&area.name)
        .bind(&area.coordinate_system)
        .bind(area.latitude)
        .bind(area.longitude)
        .bind(area.altitude)
        .bind(area.revision.value() as i64)
        .bind(utc_to_db(area.updated_at))
        .bind(area.actor.map(|a| *a.as_uuid()))
        .bind(area.id.as_uuid())
        .bind(current_revision)
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

    async fn delete_area(
        &self,
        id: AreaId,
        expected: Revision,
        deleted_at: UtcTimestamp,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let tenant_uuid = ctx
            .tenant_id
            .map(|t| *t.as_uuid())
            .ok_or_else(missing_tenant)?;
        let deps: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM org.area_closure
             WHERE tenant_id = $1 AND ancestor_id = $2 AND depth = 1",
        )
        .bind(tenant_uuid)
        .bind(id.as_uuid())
        .fetch_one(&mut *tx)
        .await
        .map_err(db_error)?;
        if deps.0 > 0 {
            return Err(PlatformError::new(
                ErrorCode::Conflict,
                "area has children".to_string(),
            ));
        }
        let deleted = utc_to_db(deleted_at);
        let rows = sqlx::query(
            "UPDATE org.areas SET deleted_at = $1, updated_at = $1, revision = $2, actor = $3
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
        sqlx::query(
            "DELETE FROM org.area_closure
             WHERE tenant_id = $1 AND (ancestor_id = $2 OR descendant_id = $2)",
        )
        .bind(tenant_uuid)
        .bind(id.as_uuid())
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn list_areas(
        &self,
        ctx: &RequestContext,
        options: ListOptions,
    ) -> Result<Page<Area>, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let rows = sqlx::query(
            "SELECT id, tenant_id, floor_id, parent_id, code, name,
                    coordinate_system, latitude, longitude, altitude,
                    revision, created_at, updated_at, actor
             FROM org.areas
             WHERE deleted_at IS NULL
             ORDER BY code
             LIMIT $1 OFFSET $2",
        )
        .bind((options.validate().limit as i64) + 1)
        .bind(options.validate().offset as i64)
        .fetch_all(&mut *tx)
        .await
        .map_err(db_error)?;
        let items = rows
            .into_iter()
            .map(area_row_to_area)
            .collect::<Result<Vec<_>, _>>()?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(paginate(items, options))
    }

    async fn areas_within_radius(
        &self,
        latitude: f64,
        longitude: f64,
        radius_meters: f64,
        ctx: &RequestContext,
        options: ListOptions,
    ) -> Result<Page<Area>, PlatformError> {
        if radius_meters < 0.0 {
            return Err(PlatformError::invalid(
                "radius_meters",
                "radius must be non-negative",
            ));
        }
        if !(-90.0..=90.0).contains(&latitude) || !(-180.0..=180.0).contains(&longitude) {
            return Err(PlatformError::invalid(
                "coordinates",
                "center coordinates are out of range",
            ));
        }
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let rows = sqlx::query(
            "SELECT id, tenant_id, floor_id, parent_id, code, name,
                    coordinate_system, latitude, longitude, altitude,
                    revision, created_at, updated_at, actor
             FROM org.areas
             WHERE deleted_at IS NULL
               AND latitude IS NOT NULL
               AND longitude IS NOT NULL
             ORDER BY code",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(db_error)?;
        let areas = rows
            .into_iter()
            .map(area_row_to_area)
            .collect::<Result<Vec<_>, _>>()?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;

        let mut items: Vec<Area> = areas
            .into_iter()
            .filter(|a| {
                if a.coordinate_system != "WGS84" {
                    return false;
                }
                let (Some(lat), Some(lon)) = (a.latitude, a.longitude) else {
                    return false;
                };
                haversine_distance(latitude, longitude, lat, lon) <= radius_meters
            })
            .collect();

        let offset = options.validate().offset as usize;
        let limit = options.validate().limit.max(1) as usize;
        if offset >= items.len() {
            return Ok(Page {
                items: Vec::new(),
                next_cursor: None,
            });
        }
        let has_more = items.len() - offset > limit;
        let next_cursor = if has_more {
            Some((options.validate().offset + limit as u64).to_string())
        } else {
            None
        };
        items = items.into_iter().skip(offset).take(limit).collect();
        Ok(Page { items, next_cursor })
    }

    async fn is_area_descendant_of(
        &self,
        ancestor: AreaId,
        descendant: AreaId,
        ctx: &RequestContext,
    ) -> Result<bool, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let exists: Option<(i64,)> = sqlx::query_as(
            "SELECT 1::int8 FROM org.area_closure
             WHERE tenant_id = $1 AND ancestor_id = $2 AND descendant_id = $3
             LIMIT 1",
        )
        .bind(
            ctx.tenant_id
                .map(|t| *t.as_uuid())
                .ok_or_else(missing_tenant)?,
        )
        .bind(ancestor.as_uuid())
        .bind(descendant.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(exists.is_some())
    }
}

async fn validate_area_references(
    tx: &mut sqlx::postgres::PgConnection,
    area: &Area,
) -> Result<(), PlatformError> {
    if let Some(floor_id) = area.floor_id {
        let floor: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM org.floors WHERE id = $1 AND deleted_at IS NULL")
                .bind(floor_id.as_uuid())
                .fetch_optional(&mut *tx)
                .await
                .map_err(db_error)?;
        if floor.is_none() {
            return Err(PlatformError::new(
                ErrorCode::NotFound,
                "floor not found".to_string(),
            ));
        }
    }
    if let Some(parent_id) = area.parent_id {
        let parent: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM org.areas WHERE id = $1 AND deleted_at IS NULL")
                .bind(parent_id.as_uuid())
                .fetch_optional(&mut *tx)
                .await
                .map_err(db_error)?;
        if parent.is_none() {
            return Err(PlatformError::new(
                ErrorCode::NotFound,
                "parent area not found".to_string(),
            ));
        }
    }
    Ok(())
}

async fn update_area_closure(
    tx: &mut sqlx::postgres::PgConnection,
    tenant_uuid: &Uuid,
    area_uuid: &Uuid,
    new_parent_uuid: Option<Uuid>,
) -> Result<(), PlatformError> {
    sqlx::query(
        "DELETE FROM org.area_closure c
         WHERE c.tenant_id = $1
           AND c.descendant_id IN (
               SELECT descendant_id FROM org.area_closure
               WHERE tenant_id = $1 AND ancestor_id = $2
           )
           AND c.ancestor_id IN (
               SELECT ancestor_id FROM org.area_closure
               WHERE tenant_id = $1 AND descendant_id = $2 AND ancestor_id != $2
           )",
    )
    .bind(*tenant_uuid)
    .bind(*area_uuid)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;

    if let Some(parent_uuid) = new_parent_uuid {
        sqlx::query(
            "INSERT INTO org.area_closure
                 (tenant_id, ancestor_id, descendant_id, depth)
             SELECT $1, a.ancestor_id, d.descendant_id, a.depth + 1 + d.depth
             FROM (SELECT ancestor_id, depth FROM org.area_closure
                   WHERE tenant_id = $1 AND descendant_id = $2) a,
                  (SELECT descendant_id, depth FROM org.area_closure
                   WHERE tenant_id = $1 AND ancestor_id = $3) d
             ON CONFLICT (tenant_id, ancestor_id, descendant_id) DO UPDATE SET depth = EXCLUDED.depth",
        )
        .bind(*tenant_uuid)
        .bind(parent_uuid)
        .bind(*area_uuid)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    }

    Ok(())
}

fn site_row_to_site(row: sqlx::postgres::PgRow) -> Result<Site, PlatformError> {
    let id: Uuid = row.try_get("id").map_err(db_error)?;
    let tenant_id: Uuid = row.try_get("tenant_id").map_err(db_error)?;
    let organization_unit_id: Option<Uuid> =
        row.try_get("organization_unit_id").map_err(db_error)?;
    let code: String = row.try_get("code").map_err(db_error)?;
    let name: String = row.try_get("name").map_err(db_error)?;
    let address: String = row.try_get("address").map_err(db_error)?;
    let timezone: String = row.try_get("timezone").map_err(db_error)?;
    let revision: i64 = row.try_get("revision").map_err(db_error)?;
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(db_error)?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at").map_err(db_error)?;
    let actor: Option<Uuid> = row.try_get("actor").map_err(db_error)?;

    Site::from_parts(
        SiteId::parse_str(&id.to_string())?,
        TenantId::parse_str(&tenant_id.to_string())?,
        organization_unit_id
            .map(|o| OrganizationId::parse_str(&o.to_string()))
            .transpose()?,
        code,
        name,
        address,
        timezone,
        revision_from_i64(revision)?,
        created_at.into(),
        updated_at.into(),
        actor
            .map(|a| UserId::parse_str(&a.to_string()))
            .transpose()?,
    )
}

fn building_row_to_building(row: sqlx::postgres::PgRow) -> Result<Building, PlatformError> {
    let id: Uuid = row.try_get("id").map_err(db_error)?;
    let tenant_id: Uuid = row.try_get("tenant_id").map_err(db_error)?;
    let site_id: Uuid = row.try_get("site_id").map_err(db_error)?;
    let code: String = row.try_get("code").map_err(db_error)?;
    let name: String = row.try_get("name").map_err(db_error)?;
    let revision: i64 = row.try_get("revision").map_err(db_error)?;
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(db_error)?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at").map_err(db_error)?;
    let actor: Option<Uuid> = row.try_get("actor").map_err(db_error)?;

    Building::from_parts(
        BuildingId::parse_str(&id.to_string())?,
        TenantId::parse_str(&tenant_id.to_string())?,
        SiteId::parse_str(&site_id.to_string())?,
        code,
        name,
        revision_from_i64(revision)?,
        created_at.into(),
        updated_at.into(),
        actor
            .map(|a| UserId::parse_str(&a.to_string()))
            .transpose()?,
    )
}

fn floor_row_to_floor(row: sqlx::postgres::PgRow) -> Result<Floor, PlatformError> {
    let id: Uuid = row.try_get("id").map_err(db_error)?;
    let tenant_id: Uuid = row.try_get("tenant_id").map_err(db_error)?;
    let building_id: Uuid = row.try_get("building_id").map_err(db_error)?;
    let code: String = row.try_get("code").map_err(db_error)?;
    let name: String = row.try_get("name").map_err(db_error)?;
    let level: i32 = row.try_get("level").map_err(db_error)?;
    let revision: i64 = row.try_get("revision").map_err(db_error)?;
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(db_error)?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at").map_err(db_error)?;
    let actor: Option<Uuid> = row.try_get("actor").map_err(db_error)?;

    Floor::from_parts(
        FloorId::parse_str(&id.to_string())?,
        TenantId::parse_str(&tenant_id.to_string())?,
        BuildingId::parse_str(&building_id.to_string())?,
        code,
        name,
        level,
        revision_from_i64(revision)?,
        created_at.into(),
        updated_at.into(),
        actor
            .map(|a| UserId::parse_str(&a.to_string()))
            .transpose()?,
    )
}

fn area_row_to_area(row: sqlx::postgres::PgRow) -> Result<Area, PlatformError> {
    let id: Uuid = row.try_get("id").map_err(db_error)?;
    let tenant_id: Uuid = row.try_get("tenant_id").map_err(db_error)?;
    let floor_id: Option<Uuid> = row.try_get("floor_id").map_err(db_error)?;
    let parent_id: Option<Uuid> = row.try_get("parent_id").map_err(db_error)?;
    let code: String = row.try_get("code").map_err(db_error)?;
    let name: String = row.try_get("name").map_err(db_error)?;
    let coordinate_system: String = row.try_get("coordinate_system").map_err(db_error)?;
    let latitude: Option<f64> = row.try_get("latitude").map_err(db_error)?;
    let longitude: Option<f64> = row.try_get("longitude").map_err(db_error)?;
    let altitude: Option<f64> = row.try_get("altitude").map_err(db_error)?;
    let revision: i64 = row.try_get("revision").map_err(db_error)?;
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(db_error)?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at").map_err(db_error)?;
    let actor: Option<Uuid> = row.try_get("actor").map_err(db_error)?;

    Area::from_parts(
        AreaId::parse_str(&id.to_string())?,
        TenantId::parse_str(&tenant_id.to_string())?,
        floor_id
            .map(|f| FloorId::parse_str(&f.to_string()))
            .transpose()?,
        parent_id
            .map(|p| AreaId::parse_str(&p.to_string()))
            .transpose()?,
        code,
        name,
        coordinate_system,
        latitude,
        longitude,
        altitude,
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
    PlatformError::new(ErrorCode::Unauthenticated, "missing tenant".to_string())
}

fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const EARTH_RADIUS_M: f64 = 6_371_000.0;
    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();
    let a = (d_lat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (d_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    EARTH_RADIUS_M * c
}
