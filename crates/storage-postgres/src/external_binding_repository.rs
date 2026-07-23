//! PostgreSQL implementation of the `ExternalBindingRepository` port.

use async_trait::async_trait;
use domain_resource::external_binding::{ExternalBinding, ExternalBindingState};
use domain_resource::tag::ResourceType;
use foundation::{
    ErrorCode, ExternalBindingId, PlatformError, RequestContext, Revision, TenantId, UserId,
    UtcTimestamp,
    chrono::{DateTime, Utc},
    uuid::Uuid,
};
use sqlx::{PgPool, Row};
use storage_api::{ExternalBindingRepository as ExternalBindingRepositoryPort, Page};

use crate::begin_tenant_transaction;

/// PostgreSQL-backed external binding repository.
#[derive(Debug, Clone)]
pub struct PostgresExternalBindingRepository {
    pool: PgPool,
}

impl PostgresExternalBindingRepository {
    /// Create a new repository backed by `pool`.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ExternalBindingRepositoryPort for PostgresExternalBindingRepository {
    async fn by_id(
        &self,
        id: ExternalBindingId,
        ctx: &RequestContext,
    ) -> Result<ExternalBinding, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let row = sqlx::query(
            "SELECT id, tenant_id, resource_type, resource_id, external_ref, external_kind,
                    state, activated_at, revision, created_at, updated_at, actor
             FROM resource.external_bindings
             WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        let binding = match row {
            Some(row) => row_to_binding(row)?,
            None => {
                return Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "external binding not found".to_string(),
                ));
            }
        };
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(binding)
    }

    async fn create(
        &self,
        binding: &ExternalBinding,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let tenant_uuid = ctx
            .tenant_id
            .map(|t| *t.as_uuid())
            .ok_or_else(missing_tenant)?;

        if *binding.tenant_id.as_uuid() != tenant_uuid {
            return Err(PlatformError::new(
                ErrorCode::Invalid,
                "external binding tenant does not match context".to_string(),
            ));
        }

        sqlx::query(
            "INSERT INTO resource.external_bindings
             (id, tenant_id, resource_type, resource_id, external_ref, external_kind,
              state, activated_at, revision, created_at, updated_at, actor, deleted_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, NULL)",
        )
        .bind(binding.id.as_uuid())
        .bind(tenant_uuid)
        .bind(binding.resource_type.as_str())
        .bind(binding.resource_id)
        .bind(&binding.external_ref)
        .bind(&binding.external_kind)
        .bind(binding.state.as_str())
        .bind(binding.activated_at.map(utc_to_db))
        .bind(binding.revision.value() as i64)
        .bind(utc_to_db(binding.created_at))
        .bind(utc_to_db(binding.updated_at))
        .bind(binding.actor.map(|a| *a.as_uuid()))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn update(
        &self,
        binding: &ExternalBinding,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        check_revision(&mut *tx, binding.id, expected).await?;

        let rows = sqlx::query(
            "UPDATE resource.external_bindings
             SET resource_type = $1, resource_id = $2, external_ref = $3, external_kind = $4,
                 state = $5, activated_at = $6, revision = $7, updated_at = $8, actor = $9
             WHERE id = $10 AND revision = $11 AND deleted_at IS NULL",
        )
        .bind(binding.resource_type.as_str())
        .bind(binding.resource_id)
        .bind(&binding.external_ref)
        .bind(&binding.external_kind)
        .bind(binding.state.as_str())
        .bind(binding.activated_at.map(utc_to_db))
        .bind(binding.revision.value() as i64)
        .bind(utc_to_db(binding.updated_at))
        .bind(binding.actor.map(|a| *a.as_uuid()))
        .bind(binding.id.as_uuid())
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

    async fn activate(
        &self,
        id: ExternalBindingId,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<ExternalBinding, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let binding_row = sqlx::query(
            "SELECT id, tenant_id, resource_type, resource_id, external_ref, external_kind,
                    state, activated_at, revision, created_at, updated_at, actor
             FROM resource.external_bindings
             WHERE id = $1 AND deleted_at IS NULL
             FOR UPDATE",
        )
        .bind(id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        let mut binding = match binding_row {
            Some(row) => row_to_binding(row)?,
            None => {
                return Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "external binding not found".to_string(),
                ));
            }
        };

        if binding.revision.value() as i64 != expected.value() as i64 {
            return Err(PlatformError::new(
                ErrorCode::VersionMismatch,
                "revision conflict".to_string(),
            ));
        }

        if binding.state != ExternalBindingState::Pending {
            return Err(PlatformError::new(
                ErrorCode::Conflict,
                "only pending bindings can be activated".to_string(),
            ));
        }

        let conflicting: Option<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM resource.external_bindings
             WHERE external_kind = $1 AND external_ref = $2 AND state = 'active'
               AND deleted_at IS NULL AND id != $3
             LIMIT 1
             FOR UPDATE",
        )
        .bind(&binding.external_kind)
        .bind(&binding.external_ref)
        .bind(binding.id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        let now = Utc::now();
        let state = if conflicting.is_some() {
            "conflict"
        } else {
            "active"
        };

        let next_revision = binding.revision.next();

        sqlx::query(
            "UPDATE resource.external_bindings
             SET state = $1, activated_at = $2, revision = $3, updated_at = $4
             WHERE id = $5 AND revision = $6 AND deleted_at IS NULL",
        )
        .bind(state)
        .bind(now)
        .bind(next_revision.value() as i64)
        .bind(now)
        .bind(binding.id.as_uuid())
        .bind(expected.value() as i64)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

        if state == "active" {
            binding.state = ExternalBindingState::Active;
            binding.activated_at = Some(now.into());
        } else {
            binding.state = ExternalBindingState::Conflict;
        }
        binding.revision = next_revision;
        binding.updated_at = now.into();

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(binding)
    }

    async fn disable(
        &self,
        id: ExternalBindingId,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        check_revision(&mut *tx, id, expected).await?;

        let rows = sqlx::query(
            "UPDATE resource.external_bindings
             SET state = 'disabled', revision = $1, updated_at = $2
             WHERE id = $3 AND revision = $4 AND deleted_at IS NULL",
        )
        .bind(expected.value() as i64 + 1)
        .bind(Utc::now())
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
    ) -> Result<Page<ExternalBinding>, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let rows = sqlx::query(
            "SELECT id, tenant_id, resource_type, resource_id, external_ref, external_kind,
                    state, activated_at, revision, created_at, updated_at, actor
             FROM resource.external_bindings
             WHERE resource_type = $1 AND resource_id = $2 AND deleted_at IS NULL
             ORDER BY created_at",
        )
        .bind(resource_type.as_str())
        .bind(resource_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(db_error)?;

        let items = rows
            .into_iter()
            .map(row_to_binding)
            .collect::<Result<Vec<_>, _>>()?;

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(Page {
            items,
            next_cursor: None,
        })
    }

    async fn list_by_external_ref(
        &self,
        external_kind: &str,
        external_ref: &str,
        ctx: &RequestContext,
    ) -> Result<Page<ExternalBinding>, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let rows = sqlx::query(
            "SELECT id, tenant_id, resource_type, resource_id, external_ref, external_kind,
                    state, activated_at, revision, created_at, updated_at, actor
             FROM resource.external_bindings
             WHERE external_kind = $1 AND external_ref = $2 AND deleted_at IS NULL
             ORDER BY created_at",
        )
        .bind(external_kind)
        .bind(external_ref)
        .fetch_all(&mut *tx)
        .await
        .map_err(db_error)?;

        let items = rows
            .into_iter()
            .map(row_to_binding)
            .collect::<Result<Vec<_>, _>>()?;

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(Page {
            items,
            next_cursor: None,
        })
    }
}

async fn check_revision(
    tx: &mut sqlx::postgres::PgConnection,
    id: ExternalBindingId,
    expected: Revision,
) -> Result<(), PlatformError> {
    let current: Option<i64> = sqlx::query_scalar(
        "SELECT revision FROM resource.external_bindings WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id.as_uuid())
    .fetch_optional(&mut *tx)
    .await
    .map_err(db_error)?;

    match current {
        None => Err(PlatformError::new(
            ErrorCode::NotFound,
            "external binding not found".to_string(),
        )),
        Some(rev) if rev != expected.value() as i64 => Err(PlatformError::new(
            ErrorCode::VersionMismatch,
            "revision conflict".to_string(),
        )),
        Some(_) => Ok(()),
    }
}

fn row_to_binding(row: sqlx::postgres::PgRow) -> Result<ExternalBinding, PlatformError> {
    let id: Uuid = row.try_get("id").map_err(db_error)?;
    let tenant_id: Uuid = row.try_get("tenant_id").map_err(db_error)?;
    let resource_type: String = row.try_get("resource_type").map_err(db_error)?;
    let resource_id: Uuid = row.try_get("resource_id").map_err(db_error)?;
    let external_ref: String = row.try_get("external_ref").map_err(db_error)?;
    let external_kind: String = row.try_get("external_kind").map_err(db_error)?;
    let state: String = row.try_get("state").map_err(db_error)?;
    let activated_at: Option<DateTime<Utc>> = row.try_get("activated_at").map_err(db_error)?;
    let revision: i64 = row.try_get("revision").map_err(db_error)?;
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(db_error)?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at").map_err(db_error)?;
    let actor: Option<Uuid> = row.try_get("actor").map_err(db_error)?;

    ExternalBinding::from_parts(
        ExternalBindingId::parse_str(&id.to_string())?,
        TenantId::parse_str(&tenant_id.to_string())?,
        ResourceType::parse(&resource_type)?,
        resource_id,
        external_ref,
        external_kind,
        ExternalBindingState::parse(&state)?,
        activated_at.map(Into::into),
        Revision::new(revision as u64),
        created_at.into(),
        updated_at.into(),
        actor
            .map(|a| UserId::parse_str(&a.to_string()))
            .transpose()?,
    )
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

fn missing_tenant() -> PlatformError {
    PlatformError::new(
        ErrorCode::Invalid,
        "tenant_id is required in request context".to_string(),
    )
}
