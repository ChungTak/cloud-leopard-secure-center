//! PostgreSQL implementation of the `CredentialRepository` port.

use crate::{begin_tenant_transaction, db_error, revision_from_i64};
use async_trait::async_trait;
use domain_identity::credential::{Credential, CredentialType};
use foundation::{
    ErrorCode, PlatformError, RequestContext, Revision, TenantId, UserId, UtcTimestamp,
    chrono::{DateTime, Utc},
    uuid::Uuid,
};
use sqlx::Row;
use storage_api::CredentialRepository;

/// PostgreSQL-backed credential repository.
#[derive(Debug, Clone)]
pub struct PostgresCredentialRepository {
    pool: sqlx::PgPool,
}

impl PostgresCredentialRepository {
    /// Create a new repository backed by `pool`.
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CredentialRepository for PostgresCredentialRepository {
    async fn by_user_and_type(
        &self,
        user_id: UserId,
        credential_type: &str,
        ctx: &RequestContext,
    ) -> Result<Credential, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let row = sqlx::query(
            "SELECT user_id, tenant_id, credential_type, value, parameters, revision, created_at, updated_at
             FROM iam.credentials
             WHERE user_id = $1 AND credential_type = $2",
        )
        .bind(*user_id.as_uuid())
        .bind(credential_type)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        let credential = row
            .map(row_to_credential)
            .transpose()?
            .ok_or_else(|| PlatformError::new(ErrorCode::NotFound, "credential not found"))?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(credential)
    }

    async fn create(
        &self,
        credential: &Credential,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        sqlx::query(
            "INSERT INTO iam.credentials
             (user_id, tenant_id, credential_type, value, parameters, revision, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(*credential.user_id.as_uuid())
        .bind(*credential.tenant_id.as_uuid())
        .bind(credential.credential_type.as_str())
        .bind(&credential.value)
        .bind(&credential.parameters)
        .bind(credential.revision.value() as i64)
        .bind(utc_to_db(credential.created_at))
        .bind(utc_to_db(credential.updated_at))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn update(
        &self,
        credential: &Credential,
        expected: Revision,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let current: Option<(i64,)> = sqlx::query_as(
            "SELECT revision FROM iam.credentials WHERE user_id = $1 AND credential_type = $2",
        )
        .bind(*credential.user_id.as_uuid())
        .bind(credential.credential_type.as_str())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        match current {
            None => {
                return Err(PlatformError::new(
                    ErrorCode::NotFound,
                    "credential not found",
                ));
            }
            Some((rev,)) if rev != expected.value() as i64 => {
                return Err(PlatformError::new(
                    ErrorCode::VersionMismatch,
                    "revision conflict",
                ));
            }
            Some(_) => {}
        }

        let rows = sqlx::query(
            "UPDATE iam.credentials
             SET value = $1, parameters = $2, revision = $3, updated_at = $4
             WHERE user_id = $5 AND credential_type = $6 AND revision = $7",
        )
        .bind(&credential.value)
        .bind(&credential.parameters)
        .bind(credential.revision.value() as i64)
        .bind(utc_to_db(credential.updated_at))
        .bind(*credential.user_id.as_uuid())
        .bind(credential.credential_type.as_str())
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
        user_id: UserId,
        credential_type: &str,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let rows =
            sqlx::query("DELETE FROM iam.credentials WHERE user_id = $1 AND credential_type = $2")
                .bind(*user_id.as_uuid())
                .bind(credential_type)
                .execute(&mut *tx)
                .await
                .map_err(db_error)?
                .rows_affected();

        if rows == 0 {
            return Err(PlatformError::new(
                ErrorCode::NotFound,
                "credential not found",
            ));
        }

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }
}

fn row_to_credential(row: sqlx::postgres::PgRow) -> Result<Credential, PlatformError> {
    let user_id: Uuid = row.try_get("user_id").map_err(db_error)?;
    let tenant_id: Uuid = row.try_get("tenant_id").map_err(db_error)?;
    let credential_type: String = row.try_get("credential_type").map_err(db_error)?;
    let value: String = row.try_get("value").map_err(db_error)?;
    let parameters: String = row.try_get("parameters").map_err(db_error)?;
    let revision: i64 = row.try_get("revision").map_err(db_error)?;
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(db_error)?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at").map_err(db_error)?;

    Credential::from_parts(
        TenantId::parse_str(&tenant_id.to_string())?,
        UserId::parse_str(&user_id.to_string())?,
        CredentialType::parse(&credential_type)?,
        value,
        parameters,
        revision_from_i64(revision)?,
        created_at.into(),
        updated_at.into(),
    )
}

fn utc_to_db(ts: UtcTimestamp) -> DateTime<Utc> {
    ts.into()
}
