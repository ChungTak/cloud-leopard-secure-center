//! PostgreSQL implementation of the `SessionRepository` port.

use crate::{begin_tenant_transaction, db_error, session_version_from_i64};
use async_trait::async_trait;
use domain_identity::session::RefreshToken;
use foundation::{
    ErrorCode, PlatformError, RequestContext, TenantId, UserId, UtcTimestamp,
    chrono::{DateTime, Utc},
    uuid::Uuid,
};
use sqlx::Row;
use storage_api::SessionRepository;

/// PostgreSQL-backed session/refresh token repository.
#[derive(Debug, Clone)]
pub struct PostgresSessionRepository {
    pool: sqlx::PgPool,
}

impl PostgresSessionRepository {
    /// Create a new repository backed by `pool`.
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SessionRepository for PostgresSessionRepository {
    async fn save_refresh_token(
        &self,
        token: &RefreshToken,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        sqlx::query(
            "INSERT INTO iam.refresh_tokens
             (id, tenant_id, user_id, family_id, token_hash, session_version, used, expires_at, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             ON CONFLICT (tenant_id, token_hash) DO NOTHING",
        )
        .bind(token.id)
        .bind(*token.tenant_id.as_uuid())
        .bind(*token.user_id.as_uuid())
        .bind(token.family_id)
        .bind(&token.token_hash)
        .bind(token.session_version as i64)
        .bind(token.used)
        .bind(utc_to_db(token.expires_at))
        .bind(utc_to_db(token.created_at))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn find_refresh_token_by_hash(
        &self,
        token_hash: &str,
        ctx: &RequestContext,
    ) -> Result<Option<RefreshToken>, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let row = sqlx::query(
            "SELECT id, tenant_id, user_id, family_id, token_hash, session_version, used, expires_at, created_at
             FROM iam.refresh_tokens
             WHERE token_hash = $1",
        )
        .bind(token_hash)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        let token = row.map(row_to_refresh_token).transpose()?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(token)
    }

    async fn mark_refresh_token_used(
        &self,
        token: &RefreshToken,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let rows = sqlx::query(
            "UPDATE iam.refresh_tokens
             SET used = true
             WHERE id = $1 AND used = false",
        )
        .bind(token.id)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?
        .rows_affected();

        if rows == 0 {
            let used: Option<(bool,)> =
                sqlx::query_as("SELECT used FROM iam.refresh_tokens WHERE id = $1")
                    .bind(token.id)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(db_error)?;
            drop(tx);
            tx_managed.rollback().await.map_err(db_error)?;
            return match used {
                Some((true,)) => Err(PlatformError::new(
                    ErrorCode::Conflict,
                    "refresh token already used",
                )),
                _ => Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "refresh token not found",
                )),
            };
        }

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn revoke_family(
        &self,
        family_id: Uuid,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        sqlx::query("DELETE FROM iam.refresh_tokens WHERE family_id = $1")
            .bind(family_id)
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn revoke_user_sessions(
        &self,
        user_id: UserId,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        sqlx::query("DELETE FROM iam.refresh_tokens WHERE user_id = $1")
            .bind(*user_id.as_uuid())
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }
}

fn row_to_refresh_token(row: sqlx::postgres::PgRow) -> Result<RefreshToken, PlatformError> {
    let id: Uuid = row.try_get("id").map_err(db_error)?;
    let tenant_id: Uuid = row.try_get("tenant_id").map_err(db_error)?;
    let user_id: Uuid = row.try_get("user_id").map_err(db_error)?;
    let family_id: Uuid = row.try_get("family_id").map_err(db_error)?;
    let token_hash: String = row.try_get("token_hash").map_err(db_error)?;
    let session_version: i64 = row.try_get("session_version").map_err(db_error)?;
    let used: bool = row.try_get("used").map_err(db_error)?;
    let expires_at: DateTime<Utc> = row.try_get("expires_at").map_err(db_error)?;
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(db_error)?;

    Ok(RefreshToken {
        id,
        tenant_id: TenantId::parse_str(&tenant_id.to_string())?,
        user_id: UserId::parse_str(&user_id.to_string())?,
        family_id,
        token_hash,
        session_version: session_version_from_i64(session_version)?,
        used,
        expires_at: expires_at.into(),
        created_at: created_at.into(),
    })
}

fn utc_to_db(ts: UtcTimestamp) -> DateTime<Utc> {
    ts.into()
}
