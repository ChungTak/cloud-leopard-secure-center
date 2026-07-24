//! PostgreSQL implementation of the `RoleBindingRepository` port.

use async_trait::async_trait;
use domain_authorization::role_binding::{ResourceRef, RoleBinding, Scope};
use foundation::{
    AreaId, BindingId, BuildingId, CameraId, DeviceId, ErrorCode, FloorId, OrganizationId,
    PlatformError, RequestContext, Revision, RoleId, SiteId, TenantId, UserId, UtcTimestamp,
    chrono::{DateTime, Utc},
    uuid::Uuid,
};
use sqlx::{PgPool, Row};
use storage_api::{ListOptions, Page, RoleBindingRepository};

use crate::{begin_tenant_transaction, db_error, paginate, revision_from_i64};

/// PostgreSQL-backed role binding repository.
#[derive(Debug, Clone)]
pub struct PostgresRoleBindingRepository {
    pool: PgPool,
}

impl PostgresRoleBindingRepository {
    /// Create a new repository backed by `pool`.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RoleBindingRepository for PostgresRoleBindingRepository {
    async fn by_id(
        &self,
        id: BindingId,
        ctx: &RequestContext,
    ) -> Result<RoleBinding, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let row = sqlx::query(
            "SELECT id, tenant_id, principal_id, role_id, scope_type, scope_ref,
                    valid_from, valid_until, revision, created_at, updated_at, actor
             FROM authz.role_bindings
             WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        let mut binding = match row {
            Some(row) => row_to_binding(row)?,
            None => {
                return Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "role binding not found".to_string(),
                ));
            }
        };
        if let Scope::ResourceSet(_) = binding.scope {
            let resources = fetch_resources(&mut *tx, id).await?;
            binding.scope = Scope::ResourceSet(resources);
        }
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(binding)
    }

    async fn create(
        &self,
        binding: &RoleBinding,
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
                "binding tenant does not match context".to_string(),
            ));
        }

        let (scope_type, scope_ref) = scope_to_db(&binding.scope);

        sqlx::query(
            "INSERT INTO authz.role_bindings
             (id, tenant_id, principal_id, role_id, scope_type, scope_ref,
              valid_from, valid_until, revision, created_at, updated_at, actor, deleted_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, NULL)",
        )
        .bind(binding.id.as_uuid())
        .bind(tenant_uuid)
        .bind(binding.principal_id.as_uuid())
        .bind(binding.role_id.as_uuid())
        .bind(scope_type)
        .bind(scope_ref)
        .bind(utc_to_db(binding.valid_from))
        .bind(binding.valid_until.map(utc_to_db))
        .bind(binding.revision.value() as i64)
        .bind(utc_to_db(binding.created_at))
        .bind(utc_to_db(binding.updated_at))
        .bind(binding.actor.map(|a| *a.as_uuid()))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

        insert_resources(&mut *tx, binding).await?;

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn update(
        &self,
        binding: &RoleBinding,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let current: Option<i64> = sqlx::query_scalar(
            "SELECT revision FROM authz.role_bindings WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(binding.id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        match current {
            None => {
                return Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "role binding not found".to_string(),
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

        let (scope_type, scope_ref) = scope_to_db(&binding.scope);

        let rows = sqlx::query(
            "UPDATE authz.role_bindings
             SET principal_id = $1, role_id = $2, scope_type = $3, scope_ref = $4,
                 valid_from = $5, valid_until = $6, revision = $7, updated_at = $8, actor = $9
             WHERE id = $10 AND revision = $11 AND deleted_at IS NULL",
        )
        .bind(binding.principal_id.as_uuid())
        .bind(binding.role_id.as_uuid())
        .bind(scope_type)
        .bind(scope_ref)
        .bind(utc_to_db(binding.valid_from))
        .bind(binding.valid_until.map(utc_to_db))
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

        sqlx::query("DELETE FROM authz.role_binding_resources WHERE role_binding_id = $1")
            .bind(binding.id.as_uuid())
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        insert_resources(&mut *tx, binding).await?;

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn delete(
        &self,
        id: BindingId,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let current: Option<i64> = sqlx::query_scalar(
            "SELECT revision FROM authz.role_bindings WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        match current {
            None => {
                return Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "role binding not found".to_string(),
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

        let now = Utc::now();
        let rows = sqlx::query(
            "UPDATE authz.role_bindings
             SET deleted_at = $1, updated_at = $1, revision = $2
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

    async fn list_by_principal(
        &self,
        principal_id: UserId,
        ctx: &RequestContext,
        options: ListOptions,
    ) -> Result<Page<RoleBinding>, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let rows = sqlx::query(
            "SELECT id, tenant_id, principal_id, role_id, scope_type, scope_ref,
                    valid_from, valid_until, revision, created_at, updated_at, actor
             FROM authz.role_bindings
             WHERE principal_id = $1 AND deleted_at IS NULL
             ORDER BY valid_from DESC
             LIMIT $2 OFFSET $3",
        )
        .bind(principal_id.as_uuid())
        .bind((options.validate().limit as i64) + 1)
        .bind(options.validate().offset as i64)
        .fetch_all(&mut *tx)
        .await
        .map_err(db_error)?;

        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let mut binding = row_to_binding(row)?;
            if let Scope::ResourceSet(_) = binding.scope {
                let resources = fetch_resources(&mut *tx, binding.id).await?;
                binding.scope = Scope::ResourceSet(resources);
            }
            items.push(binding);
        }

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(paginate(items, options))
    }
}

async fn fetch_resources(
    executor: impl sqlx::Executor<'_, Database = sqlx::Postgres>,
    binding_id: BindingId,
) -> Result<Vec<ResourceRef>, PlatformError> {
    let rows: Vec<(String, Uuid)> = sqlx::query_as(
        "SELECT resource_type, resource_id
         FROM authz.role_binding_resources
         WHERE role_binding_id = $1
         ORDER BY resource_type, resource_id",
    )
    .bind(binding_id.as_uuid())
    .fetch_all(executor)
    .await
    .map_err(db_error)?;

    rows.into_iter()
        .map(|(resource_type, resource_id)| resource_ref_from_db(&resource_type, resource_id))
        .collect()
}

async fn insert_resources(
    tx: &mut sqlx::postgres::PgConnection,
    binding: &RoleBinding,
) -> Result<(), PlatformError> {
    if let Scope::ResourceSet(resources) = &binding.scope {
        for resource in resources {
            sqlx::query(
                "INSERT INTO authz.role_binding_resources
                 (tenant_id, role_binding_id, resource_type, resource_id)
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(binding.tenant_id.as_uuid())
            .bind(binding.id.as_uuid())
            .bind(resource.resource_type())
            .bind(resource.as_uuid())
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;
        }
    }
    Ok(())
}

fn scope_to_db(scope: &Scope) -> (&'static str, Option<Uuid>) {
    match scope {
        Scope::Tenant => ("tenant", None),
        Scope::OrganizationSubtree(id) => ("organization_subtree", Some(*id.as_uuid())),
        Scope::AreaSubtree(id) => ("area_subtree", Some(*id.as_uuid())),
        Scope::ResourceSet(_) => ("resource_set", None),
    }
}

fn scope_from_db(scope_type: &str, scope_ref: Option<Uuid>) -> Result<Scope, PlatformError> {
    match scope_type {
        "tenant" => Ok(Scope::Tenant),
        "organization_subtree" => {
            let id = scope_ref.ok_or_else(|| {
                PlatformError::invalid("scope_ref", "organization_subtree requires a scope_ref")
            })?;
            Ok(Scope::OrganizationSubtree(OrganizationId::parse_str(
                &id.to_string(),
            )?))
        }
        "area_subtree" => {
            let id = scope_ref.ok_or_else(|| {
                PlatformError::invalid("scope_ref", "area_subtree requires a scope_ref")
            })?;
            Ok(Scope::AreaSubtree(AreaId::parse_str(&id.to_string())?))
        }
        "resource_set" => Ok(Scope::ResourceSet(vec![])),
        _ => Err(PlatformError::invalid(
            "scope_type",
            format!("unknown scope type: {scope_type}"),
        )),
    }
}

fn resource_ref_from_db(
    resource_type: &str,
    resource_id: Uuid,
) -> Result<ResourceRef, PlatformError> {
    let id = resource_id.to_string();
    match resource_type {
        "user" => Ok(ResourceRef::User(UserId::parse_str(&id)?)),
        "organization" => Ok(ResourceRef::Organization(OrganizationId::parse_str(&id)?)),
        "site" => Ok(ResourceRef::Site(SiteId::parse_str(&id)?)),
        "building" => Ok(ResourceRef::Building(BuildingId::parse_str(&id)?)),
        "floor" => Ok(ResourceRef::Floor(FloorId::parse_str(&id)?)),
        "area" => Ok(ResourceRef::Area(AreaId::parse_str(&id)?)),
        "device" => Ok(ResourceRef::Device(DeviceId::parse_str(&id)?)),
        "camera" => Ok(ResourceRef::Camera(CameraId::parse_str(&id)?)),
        _ => Err(PlatformError::invalid(
            "resource_type",
            format!("unknown resource type: {resource_type}"),
        )),
    }
}

fn row_to_binding(row: sqlx::postgres::PgRow) -> Result<RoleBinding, PlatformError> {
    let id: Uuid = row.try_get("id").map_err(db_error)?;
    let tenant_id: Uuid = row.try_get("tenant_id").map_err(db_error)?;
    let principal_id: Uuid = row.try_get("principal_id").map_err(db_error)?;
    let role_id: Uuid = row.try_get("role_id").map_err(db_error)?;
    let scope_type: String = row.try_get("scope_type").map_err(db_error)?;
    let scope_ref: Option<Uuid> = row.try_get("scope_ref").map_err(db_error)?;
    let valid_from: DateTime<Utc> = row.try_get("valid_from").map_err(db_error)?;
    let valid_until: Option<DateTime<Utc>> = row.try_get("valid_until").map_err(db_error)?;
    let revision: i64 = row.try_get("revision").map_err(db_error)?;
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(db_error)?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at").map_err(db_error)?;
    let actor: Option<Uuid> = row.try_get("actor").map_err(db_error)?;

    RoleBinding::from_parts(
        BindingId::parse_str(&id.to_string())?,
        TenantId::parse_str(&tenant_id.to_string())?,
        UserId::parse_str(&principal_id.to_string())?,
        RoleId::parse_str(&role_id.to_string())?,
        scope_from_db(&scope_type, scope_ref)?,
        valid_from.into(),
        valid_until.map(|d| d.into()),
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
