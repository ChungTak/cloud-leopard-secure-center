//! PostgreSQL implementation of the `ApiKeyRepository` port.

use async_trait::async_trait;
use domain_identity::api_key::ApiKey;
use foundation::{
    ErrorCode, PlatformError, RequestContext, TenantId, UserId, UtcTimestamp,
    chrono::{DateTime, Utc},
    uuid::Uuid,
};
use sqlx::Row;
use storage_api::{ApiKeyRepository, Page};

use crate::begin_tenant_transaction;

/// PostgreSQL-backed API key repository.
#[derive(Debug, Clone)]
pub struct PostgresApiKeyRepository {
    pool: sqlx::PgPool,
}

impl PostgresApiKeyRepository {
    /// Create a new repository backed by `pool`.
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ApiKeyRepository for PostgresApiKeyRepository {
    async fn create(&self, api_key: &ApiKey, ctx: &RequestContext) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        sqlx::query(
            "INSERT INTO iam.api_keys
             (id, tenant_id, owner_id, name, scopes, allowed_sources, token_hash, expires_at, revoked_at, created_at, last_used_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
             ON CONFLICT (tenant_id, token_hash) DO NOTHING",
        )
        .bind(api_key.id)
        .bind(*api_key.tenant_id.as_uuid())
        .bind(*api_key.owner_id.as_uuid())
        .bind(&api_key.name)
        .bind(&api_key.scopes)
        .bind(&api_key.allowed_sources)
        .bind(&api_key.token_hash)
        .bind(utc_to_db(api_key.expires_at))
        .bind(api_key.revoked_at.map(utc_to_db))
        .bind(utc_to_db(api_key.created_at))
        .bind(api_key.last_used_at.map(utc_to_db))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn by_id(&self, id: Uuid, ctx: &RequestContext) -> Result<ApiKey, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let row = sqlx::query(
            "SELECT id, tenant_id, owner_id, name, scopes, allowed_sources, token_hash, expires_at, revoked_at, created_at, last_used_at
             FROM iam.api_keys
             WHERE id = $1",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => PlatformError::new(ErrorCode::NotFound, "api key not found"),
            other => db_error(other),
        })?;

        let api_key = row_to_api_key(row)?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(api_key)
    }

    async fn by_token_hash(
        &self,
        token_hash: &str,
        ctx: &RequestContext,
    ) -> Result<Option<ApiKey>, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let row = sqlx::query(
            "SELECT id, tenant_id, owner_id, name, scopes, allowed_sources, token_hash, expires_at, revoked_at, created_at, last_used_at
             FROM iam.api_keys
             WHERE token_hash = $1",
        )
        .bind(token_hash)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        let api_key = row.map(row_to_api_key).transpose()?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(api_key)
    }

    async fn update(&self, api_key: &ApiKey, ctx: &RequestContext) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        sqlx::query(
            "UPDATE iam.api_keys
             SET name = $2, scopes = $3, allowed_sources = $4, token_hash = $5,
                 expires_at = $6, revoked_at = $7, created_at = $8, last_used_at = $9
             WHERE id = $1",
        )
        .bind(api_key.id)
        .bind(&api_key.name)
        .bind(&api_key.scopes)
        .bind(&api_key.allowed_sources)
        .bind(&api_key.token_hash)
        .bind(utc_to_db(api_key.expires_at))
        .bind(api_key.revoked_at.map(utc_to_db))
        .bind(utc_to_db(api_key.created_at))
        .bind(api_key.last_used_at.map(utc_to_db))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn list_by_owner(
        &self,
        owner_id: UserId,
        ctx: &RequestContext,
    ) -> Result<Page<ApiKey>, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let rows = sqlx::query(
            "SELECT id, tenant_id, owner_id, name, scopes, allowed_sources, token_hash, expires_at, revoked_at, created_at, last_used_at
             FROM iam.api_keys
             WHERE owner_id = $1
             ORDER BY created_at DESC",
        )
        .bind(*owner_id.as_uuid())
        .fetch_all(&mut *tx)
        .await
        .map_err(db_error)?;

        let items = rows
            .into_iter()
            .map(row_to_api_key)
            .collect::<Result<Vec<_>, _>>()?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(Page {
            items,
            next_cursor: None,
        })
    }
}

fn row_to_api_key(row: sqlx::postgres::PgRow) -> Result<ApiKey, PlatformError> {
    let id: Uuid = row.try_get("id").map_err(db_error)?;
    let tenant_id: Uuid = row.try_get("tenant_id").map_err(db_error)?;
    let owner_id: Uuid = row.try_get("owner_id").map_err(db_error)?;
    let name: String = row.try_get("name").map_err(db_error)?;
    let scopes: Vec<String> = row.try_get("scopes").map_err(db_error)?;
    let allowed_sources: Vec<String> = row.try_get("allowed_sources").map_err(db_error)?;
    let token_hash: String = row.try_get("token_hash").map_err(db_error)?;
    let expires_at: DateTime<Utc> = row.try_get("expires_at").map_err(db_error)?;
    let revoked_at: Option<DateTime<Utc>> = row.try_get("revoked_at").map_err(db_error)?;
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(db_error)?;
    let last_used_at: Option<DateTime<Utc>> = row.try_get("last_used_at").map_err(db_error)?;

    let mut api_key = ApiKey::new(
        id,
        TenantId::parse_str(&tenant_id.to_string())?,
        UserId::parse_str(&owner_id.to_string())?,
        name,
        scopes,
        allowed_sources,
        token_hash,
        expires_at.into(),
        created_at.into(),
    );
    api_key.revoked_at = revoked_at.map(Into::into);
    api_key.last_used_at = last_used_at.map(Into::into);
    Ok(api_key)
}

fn utc_to_db(ts: UtcTimestamp) -> DateTime<Utc> {
    ts.into()
}

fn db_error(e: sqlx::Error) -> PlatformError {
    PlatformError::new(ErrorCode::Unavailable, e.to_string())
}
