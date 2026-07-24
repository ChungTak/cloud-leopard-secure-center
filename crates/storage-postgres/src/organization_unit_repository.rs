//! PostgreSQL implementation of the `OrganizationUnitRepository` port with closure-table support.

use async_trait::async_trait;
use domain_organization::organization_unit::OrganizationUnit;
use foundation::{
    ErrorCode, OrganizationId, PlatformError, RequestContext, Revision, TenantId, UserId,
    UtcTimestamp,
    chrono::{DateTime, Utc},
    uuid::Uuid,
};
use sqlx::{PgPool, Row};
use storage_api::{ListOptions, OrganizationUnitRepository, Page};

use crate::{begin_tenant_transaction, db_error, paginate, revision_from_i64};

/// PostgreSQL-backed organization unit repository.
#[derive(Debug, Clone)]
pub struct PostgresOrganizationUnitRepository {
    pool: PgPool,
}

impl PostgresOrganizationUnitRepository {
    /// Create a new repository backed by `pool`.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl OrganizationUnitRepository for PostgresOrganizationUnitRepository {
    async fn by_id(
        &self,
        id: OrganizationId,
        ctx: &RequestContext,
    ) -> Result<OrganizationUnit, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let row = sqlx::query(
            "SELECT id, tenant_id, parent_id, code, name, revision, created_at, updated_at, actor
             FROM org.organization_units
             WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        let unit = row.map(row_to_unit).transpose()?.ok_or_else(|| {
            PlatformError::new(
                ErrorCode::NotFound,
                "organization unit not found".to_string(),
            )
        })?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(unit)
    }

    async fn create(
        &self,
        unit: &OrganizationUnit,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        if let Some(parent_id) = unit.parent_id {
            let parent: Option<(Uuid,)> = sqlx::query_as(
                "SELECT id FROM org.organization_units WHERE id = $1 AND deleted_at IS NULL",
            )
            .bind(parent_id.as_uuid())
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_error)?;
            if parent.is_none() {
                drop(tx);
                tx_managed.rollback().await.map_err(db_error)?;
                return Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "parent organization unit not found".to_string(),
                ));
            }
        }

        sqlx::query(
            "INSERT INTO org.organization_units
             (id, tenant_id, parent_id, code, name, revision, created_at, updated_at, actor, deleted_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NULL)",
        )
        .bind(unit.id.as_uuid())
        .bind(unit.tenant_id.as_uuid())
        .bind(unit.parent_id.map(|p| *p.as_uuid()))
        .bind(&unit.code)
        .bind(&unit.name)
        .bind(unit.revision.value() as i64)
        .bind(utc_to_db(unit.created_at))
        .bind(utc_to_db(unit.updated_at))
        .bind(unit.actor.map(|a| *a.as_uuid()))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

        let tenant_uuid = unit.tenant_id.as_uuid();
        let unit_uuid = unit.id.as_uuid();
        sqlx::query(
            "INSERT INTO org.organization_unit_closure (tenant_id, ancestor_id, descendant_id, depth)
             VALUES ($1, $2, $2, 0)",
        )
        .bind(*tenant_uuid)
        .bind(unit_uuid)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

        if let Some(parent_id) = unit.parent_id {
            sqlx::query(
                "INSERT INTO org.organization_unit_closure
                     (tenant_id, ancestor_id, descendant_id, depth)
                 SELECT $1, ancestor_id, $2, depth + 1
                 FROM org.organization_unit_closure
                 WHERE tenant_id = $1 AND descendant_id = $3",
            )
            .bind(*tenant_uuid)
            .bind(unit_uuid)
            .bind(parent_id.as_uuid())
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        }

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn update(
        &self,
        unit: &OrganizationUnit,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let current: Option<(i64, Option<Uuid>)> = sqlx::query_as(
            "SELECT revision, parent_id FROM org.organization_units WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(unit.id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        let (current_revision, old_parent_uuid) = match current {
            None => {
                return Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "organization unit not found".to_string(),
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

        let new_parent_uuid = unit.parent_id.map(|p| *p.as_uuid());
        if new_parent_uuid != old_parent_uuid {
            if let Some(parent_uuid) = new_parent_uuid {
                let parent_exists: Option<(Uuid,)> = sqlx::query_as(
                    "SELECT id FROM org.organization_units WHERE id = $1 AND deleted_at IS NULL",
                )
                .bind(parent_uuid)
                .fetch_optional(&mut *tx)
                .await
                .map_err(db_error)?;
                if parent_exists.is_none() {
                    return Err(PlatformError::new(
                        ErrorCode::NotFound,
                        "parent organization unit not found".to_string(),
                    ));
                }

                let is_descendant: Option<(Uuid,)> = sqlx::query_as(
                    "SELECT 1 FROM org.organization_unit_closure
                     WHERE tenant_id = $1 AND ancestor_id = $2 AND descendant_id = $3",
                )
                .bind(unit.tenant_id.as_uuid())
                .bind(unit.id.as_uuid())
                .bind(parent_uuid)
                .fetch_optional(&mut *tx)
                .await
                .map_err(db_error)?;
                if is_descendant.is_some() {
                    return Err(PlatformError::invalid(
                        "parent_id",
                        "cannot move an organization unit under one of its descendants",
                    ));
                }

                update_closure(
                    &mut *tx,
                    unit.tenant_id.as_uuid(),
                    unit.id.as_uuid(),
                    old_parent_uuid,
                    Some(parent_uuid),
                )
                .await?;
            } else {
                update_closure(
                    &mut *tx,
                    unit.tenant_id.as_uuid(),
                    unit.id.as_uuid(),
                    old_parent_uuid,
                    None,
                )
                .await?;
            }
        }

        let rows = sqlx::query(
            "UPDATE org.organization_units
             SET parent_id = $1, code = $2, name = $3, revision = $4, updated_at = $5, actor = $6
             WHERE id = $7 AND revision = $8 AND deleted_at IS NULL",
        )
        .bind(unit.parent_id.map(|p| *p.as_uuid()))
        .bind(&unit.code)
        .bind(&unit.name)
        .bind(unit.revision.value() as i64)
        .bind(utc_to_db(unit.updated_at))
        .bind(unit.actor.map(|a| *a.as_uuid()))
        .bind(unit.id.as_uuid())
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

    async fn delete(
        &self,
        id: OrganizationId,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let current: Option<(i64,)> = sqlx::query_as(
            "SELECT revision FROM org.organization_units WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        match current {
            None => {
                return Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "organization unit not found".to_string(),
                ));
            }
            Some((rev,)) if rev != expected.value() as i64 => {
                return Err(PlatformError::new(
                    ErrorCode::VersionMismatch,
                    "revision conflict".to_string(),
                ));
            }
            Some(_) => {}
        }

        let tenant_uuid = ctx.tenant_id.ok_or_else(|| {
            PlatformError::new(ErrorCode::Unauthenticated, "missing tenant".to_string())
        })?;

        let children: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM org.organization_units WHERE tenant_id = $1 AND parent_id = $2 AND deleted_at IS NULL",
        )
        .bind(tenant_uuid.as_uuid())
        .bind(id.as_uuid())
        .fetch_one(&mut *tx)
        .await
        .map_err(db_error)?;

        if children.0 > 0 {
            return Err(PlatformError::new(
                ErrorCode::Conflict,
                "organization unit has children".to_string(),
            ));
        }

        let now = Utc::now();
        let rows = sqlx::query(
            "UPDATE org.organization_units
             SET deleted_at = $1, revision = $2
             WHERE id = $3 AND revision = $4 AND deleted_at IS NULL",
        )
        .bind(now)
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

        sqlx::query(
            "DELETE FROM org.organization_unit_closure
             WHERE tenant_id = $1 AND (ancestor_id = $2 OR descendant_id = $2)",
        )
        .bind(tenant_uuid.as_uuid())
        .bind(id.as_uuid())
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn list(
        &self,
        ctx: &RequestContext,
        options: ListOptions,
    ) -> Result<Page<OrganizationUnit>, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let rows = sqlx::query(
            "SELECT id, tenant_id, parent_id, code, name, revision, created_at, updated_at, actor
             FROM org.organization_units
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
            .map(row_to_unit)
            .collect::<Result<Vec<_>, _>>()?;

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(paginate(items, options))
    }

    async fn is_descendant_of(
        &self,
        ancestor: OrganizationId,
        descendant: OrganizationId,
        ctx: &RequestContext,
    ) -> Result<bool, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let exists: Option<(i64,)> = sqlx::query_as(
            "SELECT 1::int8 FROM org.organization_unit_closure
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

async fn update_closure(
    tx: &mut sqlx::postgres::PgConnection,
    tenant_uuid: &Uuid,
    unit_uuid: &Uuid,
    _old_parent_uuid: Option<Uuid>,
    new_parent_uuid: Option<Uuid>,
) -> Result<(), PlatformError> {
    sqlx::query(
        "DELETE FROM org.organization_unit_closure c
         WHERE c.tenant_id = $1
           AND c.descendant_id IN (
               SELECT descendant_id FROM org.organization_unit_closure
               WHERE tenant_id = $1 AND ancestor_id = $2
           )
           AND c.ancestor_id IN (
               SELECT ancestor_id FROM org.organization_unit_closure
               WHERE tenant_id = $1 AND descendant_id = $2 AND ancestor_id != $2
           )",
    )
    .bind(*tenant_uuid)
    .bind(*unit_uuid)
    .execute(&mut *tx)
    .await
    .map_err(db_error)?;

    if let Some(parent_uuid) = new_parent_uuid {
        sqlx::query(
            "INSERT INTO org.organization_unit_closure
                 (tenant_id, ancestor_id, descendant_id, depth)
             SELECT $1, a.ancestor_id, d.descendant_id, a.depth + 1 + d.depth
             FROM (SELECT ancestor_id, depth FROM org.organization_unit_closure
                   WHERE tenant_id = $1 AND descendant_id = $2) a,
                  (SELECT descendant_id, depth FROM org.organization_unit_closure
                   WHERE tenant_id = $1 AND ancestor_id = $3) d
             ON CONFLICT (tenant_id, ancestor_id, descendant_id) DO UPDATE SET depth = EXCLUDED.depth",
        )
        .bind(*tenant_uuid)
        .bind(parent_uuid)
        .bind(*unit_uuid)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    }

    Ok(())
}

fn row_to_unit(row: sqlx::postgres::PgRow) -> Result<OrganizationUnit, PlatformError> {
    let id: Uuid = row.try_get("id").map_err(db_error)?;
    let tenant_id: Uuid = row.try_get("tenant_id").map_err(db_error)?;
    let parent_id: Option<Uuid> = row.try_get("parent_id").map_err(db_error)?;
    let code: String = row.try_get("code").map_err(db_error)?;
    let name: String = row.try_get("name").map_err(db_error)?;
    let revision: i64 = row.try_get("revision").map_err(db_error)?;
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(db_error)?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at").map_err(db_error)?;
    let actor: Option<Uuid> = row.try_get("actor").map_err(db_error)?;

    OrganizationUnit::from_parts(
        OrganizationId::parse_str(&id.to_string())?,
        TenantId::parse_str(&tenant_id.to_string())?,
        parent_id
            .map(|p| OrganizationId::parse_str(&p.to_string()))
            .transpose()?,
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

fn utc_to_db(ts: UtcTimestamp) -> DateTime<Utc> {
    ts.into()
}

fn missing_tenant() -> PlatformError {
    PlatformError::new(
        ErrorCode::Invalid,
        "tenant_id is required in request context".to_string(),
    )
}
