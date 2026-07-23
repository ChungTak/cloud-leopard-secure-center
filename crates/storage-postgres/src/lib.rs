//! PostgreSQL storage adapter.

pub mod api_key_repository;
pub mod audit_writer;
pub mod camera_repository;
pub mod configuration_repository;
pub mod credential_repository;
pub mod device_repository;
pub mod external_binding_repository;
pub mod login_attempt_repository;
pub mod mfa_repository;
pub mod organization_unit_repository;
pub mod projection_repository;
pub mod role_binding_repository;
pub mod role_repository;
pub mod session_repository;
pub mod spatial_repository;
pub mod tag_repository;
pub mod tenant_repository;
pub mod user_repository;

use foundation::{PlatformError, RequestContext};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};

/// Run all SQLx migrations in `migrations/` against `database_url`.
pub async fn run_migrations(database_url: &str) -> Result<(), PlatformError> {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await
        .map_err(|e| PlatformError::invalid("database_url", e.to_string()))?;

    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .map_err(|e| PlatformError::invalid("migration", e.to_string()))?;

    pool.close().await;
    Ok(())
}

/// Begin a PostgreSQL transaction and bind it to the tenant in `RequestContext`.
///
/// The tenant binding is set with `SET LOCAL` so it is automatically cleared
/// when the connection is returned to the pool.
pub async fn begin_tenant_transaction<'a>(
    pool: &PgPool,
    context: &RequestContext,
) -> Result<Transaction<'a, Postgres>, PlatformError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| PlatformError::invalid("storage", e.to_string()))?;

    let value = context
        .tenant_id
        .map(|id| id.to_hyphenated())
        .unwrap_or_default();
    sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
        .bind(&value)
        .execute(&mut *tx)
        .await
        .map_err(|e| PlatformError::invalid("storage", e.to_string()))?;

    // Ensure RLS policies are evaluated even when connected as a superuser
    // (e.g. in local development or tests). In production the app role is
    // already clsc_app, so this is a no-op.
    let _ = sqlx::query("SET LOCAL ROLE clsc_app")
        .execute(&mut *tx)
        .await;

    Ok(tx)
}

/// Clear the tenant context for the current connection. Used when returning a
/// connection to the pool to prevent cross-tenant leakage.
pub async fn clear_tenant_context(pool: &PgPool) -> Result<(), PlatformError> {
    sqlx::query("SELECT set_config('app.tenant_id', '', false)")
        .execute(pool)
        .await
        .map_err(|e| PlatformError::invalid("storage", e.to_string()))?;
    Ok(())
}

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[allow(dead_code)]
fn touch_dependencies() {
    let _v_storage_api = storage_api::version();
    let _v_domain_identity = domain_identity::version();
    let _v_foundation = foundation::version();
}

#[cfg(test)]
mod tests {
    use super::*;
    use foundation::TenantId;

    fn parse_tenant(s: &str) -> TenantId {
        match TenantId::parse_str(s) {
            Ok(v) => v,
            Err(e) => panic!("{e}"),
        }
    }

    fn ctx_for(tenant: &str) -> RequestContext {
        RequestContext {
            tenant_id: Some(parse_tenant(tenant)),
            ..Default::default()
        }
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn tenant_context_is_set_in_transaction(pool: PgPool) -> sqlx::Result<()> {
        let context = ctx_for("018e1234-5678-7abc-8def-0123456789ab");
        let mut tx = match begin_tenant_transaction(&pool, &context).await {
            Ok(tx) => tx,
            Err(e) => panic!("{e}"),
        };

        let row: (String,) = sqlx::query_as("SELECT current_setting('app.tenant_id', true)")
            .fetch_one(&mut *tx)
            .await?;
        if let Some(tenant_id) = context.tenant_id {
            assert_eq!(row.0, tenant_id.to_hyphenated());
        } else {
            panic!("missing tenant id in context");
        }

        tx.rollback().await?;
        Ok(())
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn tenant_context_is_cleared_when_empty(pool: PgPool) -> sqlx::Result<()> {
        let context = RequestContext::default();
        let mut tx = match begin_tenant_transaction(&pool, &context).await {
            Ok(tx) => tx,
            Err(e) => panic!("{e}"),
        };

        let row: (Option<String>,) =
            sqlx::query_as("SELECT nullif(current_setting('app.tenant_id', true), '')")
                .fetch_one(&mut *tx)
                .await?;
        assert!(row.0.is_none());

        tx.rollback().await?;
        Ok(())
    }
}
