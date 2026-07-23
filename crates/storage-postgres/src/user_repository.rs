//! PostgreSQL implementation of the `UserRepository` port.

use async_trait::async_trait;
use domain_identity::user::{User, UserStatus, normalize_username};
use foundation::{
    ErrorCode, PlatformError, RequestContext, Revision, TenantId, UserId, UtcTimestamp,
    chrono::{DateTime, Utc},
    uuid::Uuid,
};
use sqlx::Row;
use storage_api::{ListOptions, Page, UserRepository};

use crate::{begin_tenant_transaction, paginate};

/// PostgreSQL-backed user repository.
#[derive(Debug, Clone)]
pub struct PostgresUserRepository {
    pool: sqlx::PgPool,
}

impl PostgresUserRepository {
    /// Create a new repository backed by `pool`.
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for PostgresUserRepository {
    async fn by_id(&self, id: UserId, ctx: &RequestContext) -> Result<User, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let row = sqlx::query(
            "SELECT id, tenant_id, username, display_name, status, session_version, revision, created_at, updated_at, actor, deleted_at
             FROM iam.users
             WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(*id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        let user = row
            .map(row_to_user)
            .transpose()?
            .ok_or_else(|| PlatformError::new(ErrorCode::NotFound, "user not found"))?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(user)
    }

    async fn by_username(
        &self,
        username: &str,
        ctx: &RequestContext,
    ) -> Result<User, PlatformError> {
        let username = normalize_username(username)?;
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let row = sqlx::query(
            "SELECT id, tenant_id, username, display_name, status, session_version, revision, created_at, updated_at, actor, deleted_at
             FROM iam.users
             WHERE username = $1 AND deleted_at IS NULL",
        )
        .bind(&username)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        let user = row
            .map(row_to_user)
            .transpose()?
            .ok_or_else(|| PlatformError::new(ErrorCode::NotFound, "user not found"))?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(user)
    }

    async fn create(&self, user: &User, ctx: &RequestContext) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        sqlx::query(
            "INSERT INTO iam.users
             (id, tenant_id, username, display_name, status, session_version, revision, created_at, updated_at, actor, deleted_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NULL)",
        )
        .bind(*user.id.as_uuid())
        .bind(*user.tenant_id.as_uuid())
        .bind(&user.username)
        .bind(&user.display_name)
        .bind(user.status.as_str())
        .bind(user.session_version as i64)
        .bind(user.revision.value() as i64)
        .bind(utc_to_db(user.created_at))
        .bind(utc_to_db(user.updated_at))
        .bind(user.actor.map(|a| *a.as_uuid()))
        .execute(&mut *tx)
        .await
        .map_err(map_create_error)?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn update(
        &self,
        user: &User,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let current: Option<(i64,)> =
            sqlx::query_as("SELECT revision FROM iam.users WHERE id = $1 AND deleted_at IS NULL")
                .bind(*user.id.as_uuid())
                .fetch_optional(&mut *tx)
                .await
                .map_err(db_error)?;

        match current {
            None => return Err(PlatformError::new(ErrorCode::NotFound, "user not found")),
            Some((rev,)) if rev != expected.value() as i64 => {
                return Err(PlatformError::new(
                    ErrorCode::VersionMismatch,
                    "revision conflict",
                ));
            }
            Some(_) => {}
        }

        let deleted_at = user.deleted_at.map(utc_to_db);
        let rows = sqlx::query(
            "UPDATE iam.users
             SET username = $1, display_name = $2, status = $3, session_version = $4,
                 revision = $5, updated_at = $6, actor = $7, deleted_at = $8
             WHERE id = $9 AND revision = $10 AND deleted_at IS NULL",
        )
        .bind(&user.username)
        .bind(&user.display_name)
        .bind(user.status.as_str())
        .bind(user.session_version as i64)
        .bind(user.revision.value() as i64)
        .bind(utc_to_db(user.updated_at))
        .bind(user.actor.map(|a| *a.as_uuid()))
        .bind(deleted_at)
        .bind(*user.id.as_uuid())
        .bind(expected.value() as i64)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?
        .rows_affected();

        if rows == 0 {
            return Err(PlatformError::new(
                ErrorCode::VersionMismatch,
                "revision conflict",
            ));
        }

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn delete(
        &self,
        id: UserId,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let current: Option<(i64,)> =
            sqlx::query_as("SELECT revision FROM iam.users WHERE id = $1 AND deleted_at IS NULL")
                .bind(*id.as_uuid())
                .fetch_optional(&mut *tx)
                .await
                .map_err(db_error)?;

        match current {
            None => return Err(PlatformError::new(ErrorCode::NotFound, "user not found")),
            Some((rev,)) if rev != expected.value() as i64 => {
                return Err(PlatformError::new(
                    ErrorCode::VersionMismatch,
                    "revision conflict",
                ));
            }
            Some(_) => {}
        }

        let now = Utc::now();
        let rows = sqlx::query(
            "UPDATE iam.users
             SET deleted_at = $1, revision = $2
             WHERE id = $3 AND revision = $4 AND deleted_at IS NULL",
        )
        .bind(now)
        .bind(expected.value() as i64 + 1)
        .bind(*id.as_uuid())
        .bind(expected.value() as i64)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?
        .rows_affected();

        if rows == 0 {
            return Err(PlatformError::new(
                ErrorCode::VersionMismatch,
                "revision conflict",
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
    ) -> Result<Page<User>, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let rows = sqlx::query(
            "SELECT id, tenant_id, username, display_name, status, session_version, revision, created_at, updated_at, actor, deleted_at
             FROM iam.users
             WHERE deleted_at IS NULL
             ORDER BY username
             LIMIT $1 OFFSET $2",
        )
        .bind((options.validate().limit as i64) + 1)
        .bind(options.validate().offset as i64)
        .fetch_all(&mut *tx)
        .await
        .map_err(db_error)?;

        let items = rows
            .into_iter()
            .map(row_to_user)
            .collect::<Result<Vec<_>, _>>()?;

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(paginate(items, options))
    }
}

fn row_to_user(row: sqlx::postgres::PgRow) -> Result<User, PlatformError> {
    let id: Uuid = row.try_get("id").map_err(db_error)?;
    let tenant_id: Uuid = row.try_get("tenant_id").map_err(db_error)?;
    let username: String = row.try_get("username").map_err(db_error)?;
    let display_name: String = row.try_get("display_name").map_err(db_error)?;
    let status: String = row.try_get("status").map_err(db_error)?;
    let session_version: i64 = row.try_get("session_version").map_err(db_error)?;
    let revision: i64 = row.try_get("revision").map_err(db_error)?;
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(db_error)?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at").map_err(db_error)?;
    let actor: Option<Uuid> = row.try_get("actor").map_err(db_error)?;
    let deleted_at: Option<DateTime<Utc>> = row.try_get("deleted_at").map_err(db_error)?;

    Ok(User {
        id: UserId::parse_str(&id.to_string())?,
        tenant_id: TenantId::parse_str(&tenant_id.to_string())?,
        username,
        display_name,
        status: UserStatus::parse(&status)?,
        session_version: session_version as u64,
        revision: Revision::new(revision as u64),
        created_at: created_at.into(),
        updated_at: updated_at.into(),
        actor: actor
            .map(|a| UserId::parse_str(&a.to_string()))
            .transpose()?,
        deleted_at: deleted_at.map(|d| d.into()),
        pending_events: Vec::new(),
    })
}

fn utc_to_db(ts: UtcTimestamp) -> DateTime<Utc> {
    ts.into()
}

fn db_error(e: sqlx::Error) -> PlatformError {
    PlatformError::new(ErrorCode::Unavailable, e.to_string())
}

fn map_create_error(e: sqlx::Error) -> PlatformError {
    if let sqlx::Error::Database(ref db) = e
        && db.constraint().is_some()
    {
        return PlatformError::new(ErrorCode::Exists, "user already exists");
    }
    PlatformError::new(ErrorCode::Unavailable, e.to_string())
}
