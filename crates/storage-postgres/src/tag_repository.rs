//! PostgreSQL implementation of the `TagRepository` port.

use async_trait::async_trait;
use domain_resource::tag::{ResourceType, Tag};
use foundation::{
    ErrorCode, PlatformError, RequestContext, Revision, TagId, TenantId, UserId, UtcTimestamp,
    chrono::{DateTime, Utc},
    uuid::Uuid,
};
use sqlx::{PgPool, Row};
use storage_api::{ListOptions, Page, TagRepository};

use crate::{begin_tenant_transaction, paginate};

/// PostgreSQL-backed tag repository.
#[derive(Debug, Clone)]
pub struct PostgresTagRepository {
    pool: PgPool,
}

impl PostgresTagRepository {
    /// Create a new repository backed by `pool`.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TagRepository for PostgresTagRepository {
    async fn by_id(&self, id: TagId, ctx: &RequestContext) -> Result<Tag, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let row = sqlx::query(
            "SELECT id, tenant_id, resource_type, resource_id, key, value,
                    revision, created_at, updated_at, actor
             FROM resource.tags
             WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        let tag = match row {
            Some(row) => row_to_tag(row)?,
            None => {
                return Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "tag not found".to_string(),
                ));
            }
        };
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(tag)
    }

    async fn create(&self, tag: &Tag, ctx: &RequestContext) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let tenant_uuid = ctx
            .tenant_id
            .map(|t| *t.as_uuid())
            .ok_or_else(missing_tenant)?;

        if *tag.tenant_id.as_uuid() != tenant_uuid {
            return Err(PlatformError::new(
                ErrorCode::Invalid,
                "tag tenant does not match context".to_string(),
            ));
        }

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM resource.tags
             WHERE tenant_id = $1 AND resource_type = $2 AND resource_id = $3 AND deleted_at IS NULL",
        )
        .bind(tenant_uuid)
        .bind(tag.resource_type.as_str())
        .bind(tag.resource_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(db_error)?;

        if count as usize >= domain_resource::tag::MAX_TAGS_PER_RESOURCE {
            return Err(PlatformError::new(
                ErrorCode::Invalid,
                format!(
                    "resource cannot have more than {} tags",
                    domain_resource::tag::MAX_TAGS_PER_RESOURCE
                ),
            ));
        }

        sqlx::query(
            "INSERT INTO resource.tags
             (id, tenant_id, resource_type, resource_id, key, value,
              revision, created_at, updated_at, actor, deleted_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NULL)",
        )
        .bind(tag.id.as_uuid())
        .bind(tenant_uuid)
        .bind(tag.resource_type.as_str())
        .bind(tag.resource_id)
        .bind(&tag.key)
        .bind(&tag.value)
        .bind(tag.revision.value() as i64)
        .bind(utc_to_db(tag.created_at))
        .bind(utc_to_db(tag.updated_at))
        .bind(tag.actor.map(|a| *a.as_uuid()))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn update(
        &self,
        tag: &Tag,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let current: Option<i64> = sqlx::query_scalar(
            "SELECT revision FROM resource.tags WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(tag.id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        match current {
            None => {
                return Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "tag not found".to_string(),
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
            "UPDATE resource.tags
             SET value = $1, revision = $2, updated_at = $3, actor = $4
             WHERE id = $5 AND revision = $6 AND deleted_at IS NULL",
        )
        .bind(&tag.value)
        .bind(tag.revision.value() as i64)
        .bind(utc_to_db(tag.updated_at))
        .bind(tag.actor.map(|a| *a.as_uuid()))
        .bind(tag.id.as_uuid())
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
        id: TagId,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let current: Option<i64> = sqlx::query_scalar(
            "SELECT revision FROM resource.tags WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        match current {
            None => {
                return Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "tag not found".to_string(),
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
            "UPDATE resource.tags
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

    async fn list_by_resource(
        &self,
        resource_type: ResourceType,
        resource_id: Uuid,
        ctx: &RequestContext,
        options: ListOptions,
    ) -> Result<Page<Tag>, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let rows = sqlx::query(
            "SELECT id, tenant_id, resource_type, resource_id, key, value,
                    revision, created_at, updated_at, actor
             FROM resource.tags
             WHERE resource_type = $1 AND resource_id = $2 AND deleted_at IS NULL
             ORDER BY key
             LIMIT $3 OFFSET $4",
        )
        .bind(resource_type.as_str())
        .bind(resource_id)
        .bind((options.limit as i64) + 1)
        .bind(options.offset as i64)
        .fetch_all(&mut *tx)
        .await
        .map_err(db_error)?;

        let items = rows
            .into_iter()
            .map(row_to_tag)
            .collect::<Result<Vec<_>, _>>()?;

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(paginate(items, options))
    }
}

fn row_to_tag(row: sqlx::postgres::PgRow) -> Result<Tag, PlatformError> {
    let id: Uuid = row.try_get("id").map_err(db_error)?;
    let tenant_id: Uuid = row.try_get("tenant_id").map_err(db_error)?;
    let resource_type: String = row.try_get("resource_type").map_err(db_error)?;
    let resource_id: Uuid = row.try_get("resource_id").map_err(db_error)?;
    let key: String = row.try_get("key").map_err(db_error)?;
    let value: String = row.try_get("value").map_err(db_error)?;
    let revision: i64 = row.try_get("revision").map_err(db_error)?;
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(db_error)?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at").map_err(db_error)?;
    let actor: Option<Uuid> = row.try_get("actor").map_err(db_error)?;

    Tag::from_parts(
        TagId::parse_str(&id.to_string())?,
        TenantId::parse_str(&tenant_id.to_string())?,
        ResourceType::parse(&resource_type)?,
        resource_id,
        key,
        value,
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
