//! PostgreSQL implementation of the `RoleRepository` port.

use async_trait::async_trait;
use domain_authorization::role::Role;
use foundation::{
    ErrorCode, PlatformError, RequestContext, Revision, RoleId, TenantId, UserId, UtcTimestamp,
    chrono::{DateTime, Utc},
    uuid::Uuid,
};
use sqlx::{PgPool, Row};
use storage_api::{ListOptions, Page, RoleRepository};

use crate::{begin_tenant_transaction, paginate};

/// PostgreSQL-backed role repository.
#[derive(Debug, Clone)]
pub struct PostgresRoleRepository {
    pool: PgPool,
}

impl PostgresRoleRepository {
    /// Create a new repository backed by `pool`.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RoleRepository for PostgresRoleRepository {
    async fn by_id(&self, id: RoleId, ctx: &RequestContext) -> Result<Role, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let row = sqlx::query(
            "SELECT id, tenant_id, name, is_builtin, revision, created_at, updated_at, actor
             FROM authz.roles
             WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        let mut role = match row {
            Some(row) => row_to_role(row)?,
            None => {
                return Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "role not found".to_string(),
                ));
            }
        };
        role.permissions = fetch_permissions_for_id(&mut *tx, id).await?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(role)
    }

    async fn create(&self, role: &Role, ctx: &RequestContext) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let tenant_uuid = ctx
            .tenant_id
            .map(|t| *t.as_uuid())
            .ok_or_else(missing_tenant)?;

        if role.tenant_id.is_some() && role.tenant_id != ctx.tenant_id {
            return Err(PlatformError::new(
                ErrorCode::Invalid,
                "role tenant does not match context".to_string(),
            ));
        }

        sqlx::query(
            "INSERT INTO authz.roles
             (id, tenant_id, name, is_builtin, revision, created_at, updated_at, actor, deleted_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NULL)",
        )
        .bind(role.id.as_uuid())
        .bind(tenant_uuid)
        .bind(&role.name)
        .bind(role.is_builtin)
        .bind(role.revision.value() as i64)
        .bind(utc_to_db(role.created_at))
        .bind(utc_to_db(role.updated_at))
        .bind(role.actor.map(|a| *a.as_uuid()))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

        insert_permissions(&mut *tx, role).await?;

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn update(
        &self,
        role: &Role,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let existing: Option<(bool, i64)> = sqlx::query_as(
            "SELECT is_builtin, revision FROM authz.roles WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(role.id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        let (builtin, rev) = match existing {
            Some(v) => v,
            None => {
                return Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "role not found".to_string(),
                ));
            }
        };

        if rev != expected.value() as i64 {
            return Err(PlatformError::new(
                ErrorCode::VersionMismatch,
                "revision conflict".to_string(),
            ));
        }

        if builtin {
            return Err(PlatformError::new(
                ErrorCode::Invalid,
                "built-in roles cannot be modified".to_string(),
            ));
        }

        let rows = sqlx::query(
            "UPDATE authz.roles
             SET name = $1, revision = $2, updated_at = $3, actor = $4
             WHERE id = $5 AND revision = $6 AND deleted_at IS NULL",
        )
        .bind(&role.name)
        .bind(role.revision.value() as i64)
        .bind(utc_to_db(role.updated_at))
        .bind(role.actor.map(|a| *a.as_uuid()))
        .bind(role.id.as_uuid())
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

        sqlx::query("DELETE FROM authz.role_permissions WHERE role_id = $1")
            .bind(role.id.as_uuid())
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        insert_permissions(&mut *tx, role).await?;

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn delete(
        &self,
        id: RoleId,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let existing: Option<(bool, i64)> = sqlx::query_as(
            "SELECT is_builtin, revision FROM authz.roles WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        let (builtin, rev) = match existing {
            Some(v) => v,
            None => {
                return Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "role not found".to_string(),
                ));
            }
        };

        if rev != expected.value() as i64 {
            return Err(PlatformError::new(
                ErrorCode::VersionMismatch,
                "revision conflict".to_string(),
            ));
        }

        if builtin {
            return Err(PlatformError::new(
                ErrorCode::Invalid,
                "built-in roles cannot be deleted".to_string(),
            ));
        }

        let rows = sqlx::query(
            "UPDATE authz.roles
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

    async fn list(
        &self,
        ctx: &RequestContext,
        options: ListOptions,
    ) -> Result<Page<Role>, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let rows = sqlx::query(
            "SELECT id, tenant_id, name, is_builtin, revision, created_at, updated_at, actor
             FROM authz.roles
             WHERE deleted_at IS NULL
             ORDER BY name
             LIMIT $1 OFFSET $2",
        )
        .bind((options.validate().limit as i64) + 1)
        .bind(options.validate().offset as i64)
        .fetch_all(&mut *tx)
        .await
        .map_err(db_error)?;

        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let mut role = row_to_role(row)?;
            let id = role.id;
            role.permissions = fetch_permissions_for_id(&mut *tx, id).await?;
            items.push(role);
        }

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(paginate(items, options))
    }
}

async fn fetch_permissions_for_id(
    executor: impl sqlx::Executor<'_, Database = sqlx::Postgres>,
    id: RoleId,
) -> Result<Vec<String>, PlatformError> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT permission_key FROM authz.role_permissions WHERE role_id = $1 ORDER BY permission_key",
    )
    .bind(id.as_uuid())
    .fetch_all(executor)
    .await
    .map_err(db_error)?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

async fn insert_permissions(
    tx: &mut sqlx::postgres::PgConnection,
    role: &Role,
) -> Result<(), PlatformError> {
    for key in &role.permissions {
        sqlx::query(
            "INSERT INTO authz.role_permissions (role_id, tenant_id, permission_key)
             VALUES ($1, $2, $3)",
        )
        .bind(role.id.as_uuid())
        .bind(role.tenant_id.map(|t| *t.as_uuid()))
        .bind(key)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    }
    Ok(())
}

fn row_to_role(row: sqlx::postgres::PgRow) -> Result<Role, PlatformError> {
    let id: Uuid = row.try_get("id").map_err(db_error)?;
    let tenant_id: Uuid = row.try_get("tenant_id").map_err(db_error)?;
    let name: String = row.try_get("name").map_err(db_error)?;
    let is_builtin: bool = row.try_get("is_builtin").map_err(db_error)?;
    let revision: i64 = row.try_get("revision").map_err(db_error)?;
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(db_error)?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at").map_err(db_error)?;
    let actor: Option<Uuid> = row.try_get("actor").map_err(db_error)?;

    Role::from_parts(
        RoleId::parse_str(&id.to_string())?,
        Some(TenantId::parse_str(&tenant_id.to_string())?),
        name,
        is_builtin,
        vec![],
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
