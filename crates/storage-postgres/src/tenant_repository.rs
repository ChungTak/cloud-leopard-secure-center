//! PostgreSQL implementation of the `TenantRepository` port.

use async_trait::async_trait;
use domain_identity::tenant::{Tenant, TenantStatus};
use foundation::{
    ErrorCode, PlatformError, RequestContext, Revision, TenantId, UserId, UtcTimestamp,
    chrono::{DateTime, Utc},
    uuid::Uuid,
};
use sqlx::{PgPool, Row};
use storage_api::{Page, TenantRepository};

use crate::begin_tenant_transaction;

/// PostgreSQL-backed tenant repository.
#[derive(Debug, Clone)]
pub struct PostgresTenantRepository {
    pool: PgPool,
}

impl PostgresTenantRepository {
    /// Create a new repository backed by `pool`.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TenantRepository for PostgresTenantRepository {
    async fn by_id(&self, id: TenantId, ctx: &RequestContext) -> Result<Tenant, PlatformError> {
        let mut tx = begin_tenant_transaction(&self.pool, ctx).await?;
        let row = sqlx::query(
            "SELECT id, name, status, revision, created_at, updated_at, actor
             FROM iam.tenants
             WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        let tenant = row.map(row_to_tenant).transpose()?.ok_or_else(|| {
            PlatformError::new(ErrorCode::NotFound, "tenant not found".to_string())
        })?;
        tx.commit().await.map_err(db_error)?;
        Ok(tenant)
    }

    async fn create(&self, tenant: &Tenant, ctx: &RequestContext) -> Result<(), PlatformError> {
        let mut tx = begin_tenant_transaction(&self.pool, ctx).await?;
        sqlx::query(
            "INSERT INTO iam.tenants
             (id, name, status, revision, created_at, updated_at, actor, deleted_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, NULL)",
        )
        .bind(tenant.id.as_uuid())
        .bind(&tenant.name)
        .bind(tenant.status.as_str())
        .bind(tenant.revision.value() as i64)
        .bind(utc_to_db(tenant.created_at))
        .bind(utc_to_db(tenant.updated_at))
        .bind(tenant.actor.map(|a| *a.as_uuid()))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
        tx.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn update(
        &self,
        tenant: &Tenant,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let mut tx = begin_tenant_transaction(&self.pool, ctx).await?;

        let current: Option<(i64,)> =
            sqlx::query_as("SELECT revision FROM iam.tenants WHERE id = $1 AND deleted_at IS NULL")
                .bind(tenant.id.as_uuid())
                .fetch_optional(&mut *tx)
                .await
                .map_err(db_error)?;

        match current {
            None => {
                return Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "tenant not found".to_string(),
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

        let rows = sqlx::query(
            "UPDATE iam.tenants
             SET name = $1, status = $2, revision = $3, updated_at = $4, actor = $5
             WHERE id = $6 AND revision = $7 AND deleted_at IS NULL",
        )
        .bind(&tenant.name)
        .bind(tenant.status.as_str())
        .bind(tenant.revision.value() as i64)
        .bind(utc_to_db(tenant.updated_at))
        .bind(tenant.actor.map(|a| *a.as_uuid()))
        .bind(tenant.id.as_uuid())
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

        tx.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn delete(
        &self,
        id: TenantId,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let mut tx = begin_tenant_transaction(&self.pool, ctx).await?;

        let current: Option<(i64,)> =
            sqlx::query_as("SELECT revision FROM iam.tenants WHERE id = $1 AND deleted_at IS NULL")
                .bind(id.as_uuid())
                .fetch_optional(&mut *tx)
                .await
                .map_err(db_error)?;

        match current {
            None => {
                return Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "tenant not found".to_string(),
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

        let now = Utc::now();
        let rows = sqlx::query(
            "UPDATE iam.tenants
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

        tx.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn list(&self, ctx: &RequestContext) -> Result<Page<Tenant>, PlatformError> {
        let mut tx = begin_tenant_transaction(&self.pool, ctx).await?;
        let rows = sqlx::query(
            "SELECT id, name, status, revision, created_at, updated_at, actor
             FROM iam.tenants
             WHERE deleted_at IS NULL
             ORDER BY id
             LIMIT 100",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(db_error)?;

        let items = rows
            .into_iter()
            .map(row_to_tenant)
            .collect::<Result<Vec<_>, _>>()?;

        tx.commit().await.map_err(db_error)?;
        Ok(Page {
            items,
            next_cursor: None,
        })
    }
}

fn row_to_tenant(row: sqlx::postgres::PgRow) -> Result<Tenant, PlatformError> {
    let id: Uuid = row.try_get("id").map_err(db_error)?;
    let name: String = row.try_get("name").map_err(db_error)?;
    let status: String = row.try_get("status").map_err(db_error)?;
    let revision: i64 = row.try_get("revision").map_err(db_error)?;
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(db_error)?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at").map_err(db_error)?;
    let actor: Option<Uuid> = row.try_get("actor").map_err(db_error)?;

    Ok(Tenant {
        id: TenantId::parse_str(&id.to_string())?,
        name,
        status: TenantStatus::parse(&status)?,
        revision: Revision::new(revision as u64),
        created_at: created_at.into(),
        updated_at: updated_at.into(),
        actor: actor
            .map(|a| UserId::parse_str(&a.to_string()))
            .transpose()?,
    })
}

fn utc_to_db(ts: UtcTimestamp) -> DateTime<Utc> {
    match DateTime::from_timestamp_millis(ts.timestamp_millis()) {
        Some(dt) => dt,
        None => panic!("timestamp from database is invalid"),
    }
}

fn db_error(e: sqlx::Error) -> PlatformError {
    PlatformError::new(ErrorCode::Unavailable, e.to_string())
}
