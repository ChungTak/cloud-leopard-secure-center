//! PostgreSQL implementation of the `MfaRepository` port.

use crate::db_error;
use async_trait::async_trait;
use domain_identity::mfa::{MfaFactor, MfaFactorType};
use foundation::{
    PlatformError, RequestContext, TenantId, UserId, UtcTimestamp,
    chrono::{DateTime, Utc},
    uuid::Uuid,
};
use sqlx::Row;
use storage_api::MfaRepository;

use crate::begin_tenant_transaction;

/// PostgreSQL-backed MFA repository.
#[derive(Debug, Clone)]
pub struct PostgresMfaRepository {
    pool: sqlx::PgPool,
}

impl PostgresMfaRepository {
    /// Create a new repository backed by `pool`.
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MfaRepository for PostgresMfaRepository {
    async fn save_factor(
        &self,
        factor: &MfaFactor,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        sqlx::query(
            "INSERT INTO iam.mfa_factors
             (id, tenant_id, user_id, factor_type, secret_ref, enabled, verified_at,
              recovery_code_hashes, recovery_code_used, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(factor.id)
        .bind(*factor.tenant_id.as_uuid())
        .bind(*factor.user_id.as_uuid())
        .bind(factor_type_str(factor.factor_type))
        .bind(&factor.secret_ref)
        .bind(factor.enabled)
        .bind(factor.verified_at.map(utc_to_db))
        .bind(factor.recovery_code_hashes())
        .bind(factor.recovery_code_used())
        .bind(utc_to_db(factor.created_at))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn update_factor(
        &self,
        factor: &MfaFactor,
        ctx: &RequestContext,
    ) -> Result<(), PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        sqlx::query(
            "UPDATE iam.mfa_factors
             SET secret_ref = $2, enabled = $3, verified_at = $4,
                 recovery_code_hashes = $5, recovery_code_used = $6,
                 last_used_step = $7, last_used_code = $8
             WHERE id = $1",
        )
        .bind(factor.id)
        .bind(&factor.secret_ref)
        .bind(factor.enabled)
        .bind(factor.verified_at.map(utc_to_db))
        .bind(factor.recovery_code_hashes())
        .bind(factor.recovery_code_used())
        .bind(factor.last_used_step.map(|s| s as i64))
        .bind(factor.last_used_code.as_deref())
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(())
    }

    async fn find_active_factor_by_user(
        &self,
        user_id: UserId,
        ctx: &RequestContext,
    ) -> Result<Option<MfaFactor>, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;
        let row = sqlx::query(
            "SELECT id, tenant_id, user_id, factor_type, secret_ref, enabled, verified_at,
                    recovery_code_hashes, recovery_code_used, created_at,
                    last_used_step, last_used_code
             FROM iam.mfa_factors
             WHERE user_id = $1 AND enabled = true",
        )
        .bind(*user_id.as_uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        let factor = row.map(row_to_factor).transpose()?;
        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;
        Ok(factor)
    }
}

fn factor_type_str(factor_type: MfaFactorType) -> &'static str {
    match factor_type {
        MfaFactorType::Totp => "totp",
    }
}

fn parse_factor_type(s: &str) -> Result<MfaFactorType, PlatformError> {
    match s {
        "totp" => Ok(MfaFactorType::Totp),
        _ => Err(PlatformError::invalid("factor_type", "unknown factor type")),
    }
}

fn row_to_factor(row: sqlx::postgres::PgRow) -> Result<MfaFactor, PlatformError> {
    let id: Uuid = row.try_get("id").map_err(db_error)?;
    let tenant_id: Uuid = row.try_get("tenant_id").map_err(db_error)?;
    let user_id: Uuid = row.try_get("user_id").map_err(db_error)?;
    let factor_type: String = row.try_get("factor_type").map_err(db_error)?;
    let secret_ref: String = row.try_get("secret_ref").map_err(db_error)?;
    let enabled: bool = row.try_get("enabled").map_err(db_error)?;
    let verified_at: Option<DateTime<Utc>> = row.try_get("verified_at").map_err(db_error)?;
    let recovery_code_hashes: Vec<String> =
        row.try_get("recovery_code_hashes").map_err(db_error)?;
    let recovery_code_used: Vec<bool> = row.try_get("recovery_code_used").map_err(db_error)?;
    let created_at: DateTime<Utc> = row.try_get("created_at").map_err(db_error)?;
    let last_used_step: Option<i64> = row.try_get("last_used_step").map_err(db_error)?;
    let last_used_code: Option<String> = row.try_get("last_used_code").map_err(db_error)?;

    let last_used_step = last_used_step
        .map(|s| {
            u64::try_from(s).map_err(|_| {
                PlatformError::invalid(
                    "last_used_step",
                    "stored last_used_step is out of valid range",
                )
            })
        })
        .transpose()?;

    MfaFactor::from_parts(
        id,
        TenantId::parse_str(&tenant_id.to_string())?,
        UserId::parse_str(&user_id.to_string())?,
        parse_factor_type(&factor_type)?,
        secret_ref,
        enabled,
        verified_at.map(Into::into),
        created_at.into(),
        recovery_code_hashes,
        recovery_code_used,
        last_used_step,
        last_used_code,
    )
}

fn utc_to_db(ts: UtcTimestamp) -> DateTime<Utc> {
    ts.into()
}
