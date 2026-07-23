//! PostgreSQL implementation of the `TenantRepository` port.

use async_trait::async_trait;
use domain_organization::tenant::{Tenant, TenantStatus};
use foundation::{
    ErrorCode, PlatformError, RequestContext, Revision, TenantId, UserId, UtcTimestamp,
    chrono::{DateTime, Utc},
    uuid::Uuid,
};
use sqlx::{PgPool, Row};
use storage_api::{ListOptions, Page, TenantRepository};

use crate::{begin_tenant_transaction, db_error, paginate};

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
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let row = sqlx::query(
            "SELECT id, code, name, locale, timezone, status, revision, created_at, updated_at, actor
             FROM org.tenants
             WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        let tenant = row.map(row_to_tenant).transpose()?.ok_or_else(|| {
            PlatformError::new(ErrorCode::NotFound, "tenant not found".to_string())
        })?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(tenant)
    }

    async fn create(&self, tenant: &Tenant, ctx: &RequestContext) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        sqlx::query(
            "INSERT INTO org.tenants
             (id, code, name, locale, timezone, status, revision, created_at, updated_at, actor, deleted_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NULL)",
        )
        .bind(tenant.id.as_uuid())
        .bind(&tenant.code)
        .bind(&tenant.name)
        .bind(&tenant.locale)
        .bind(&tenant.timezone)
        .bind(tenant.status.as_str())
        .bind(tenant.revision.value() as i64)
        .bind(utc_to_db(tenant.created_at))
        .bind(utc_to_db(tenant.updated_at))
        .bind(tenant.actor.map(|a| *a.as_uuid()))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn update(
        &self,
        tenant: &Tenant,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let current: Option<(i64,)> =
            sqlx::query_as("SELECT revision FROM org.tenants WHERE id = $1 AND deleted_at IS NULL")
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
            "UPDATE org.tenants
             SET name = $1, locale = $2, timezone = $3, status = $4, revision = $5, updated_at = $6, actor = $7
             WHERE id = $8 AND revision = $9 AND deleted_at IS NULL",
        )
        .bind(&tenant.name)
        .bind(&tenant.locale)
        .bind(&tenant.timezone)
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

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn delete(
        &self,
        id: TenantId,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let current: Option<(i64,)> =
            sqlx::query_as("SELECT revision FROM org.tenants WHERE id = $1 AND deleted_at IS NULL")
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
            "UPDATE org.tenants
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

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn list(
        &self,
        ctx: &RequestContext,
        options: ListOptions,
    ) -> Result<Page<Tenant>, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let rows = sqlx::query(
            "SELECT id, code, name, locale, timezone, status, revision, created_at, updated_at, actor
             FROM org.tenants
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
            .map(row_to_tenant)
            .collect::<Result<Vec<_>, _>>()?;

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(paginate(items, options))
    }
}

fn row_to_tenant(row: sqlx::postgres::PgRow) -> Result<Tenant, PlatformError> {
    let id: Uuid = row.try_get("id").map_err(db_error)?;
    let code: String = row.try_get("code").map_err(db_error)?;
    let name: String = row.try_get("name").map_err(db_error)?;
    let locale: String = row.try_get("locale").map_err(db_error)?;
    let timezone: String = row.try_get("timezone").map_err(db_error)?;
    let status: String = row.try_get("status").map_err(db_error)?;
    let revision: i64 = row.try_get("revision").map_err(db_error)?;
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(db_error)?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at").map_err(db_error)?;
    let actor: Option<Uuid> = row.try_get("actor").map_err(db_error)?;

    Tenant::from_parts(
        TenantId::parse_str(&id.to_string())?,
        code,
        name,
        locale,
        timezone,
        TenantStatus::parse(&status)?,
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
