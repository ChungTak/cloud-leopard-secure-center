//! PostgreSQL storage adapter.
//
// Explicit `&mut *tx` derefs are needed because `ManagedConnection` does not
// implement `sqlx::Executor` and deref coercion cannot satisfy the generic
// trait bound.
#![allow(clippy::explicit_auto_deref)]

pub mod api_key_repository;
pub mod audit_writer;
pub mod camera_repository;
pub mod configuration_repository;
pub mod credential_repository;
pub mod device_repository;
pub mod external_binding_repository;
pub mod idempotency_repository;
pub mod inbox_repository;
pub mod job_repository;
pub mod login_attempt_repository;
pub mod mfa_repository;
pub mod organization_unit_repository;
pub mod outbox_repository;
pub mod projection_repository;
pub mod retention_repository;
pub mod role_binding_repository;
pub mod role_repository;
pub mod session_repository;
pub mod spatial_repository;
pub mod tag_repository;
pub mod tenant_repository;
pub mod unit_of_work;
pub mod user_repository;

pub use storage_api::ListOptions;

use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use foundation::{ErrorCode, PlatformError, RequestContext};
use sqlx::pool::PoolConnection;
use sqlx::postgres::{PgConnection, PgPoolOptions};
use sqlx::{PgPool, Postgres};
use storage_api::{ListOptions as ListOptionsInner, Page};
use tokio::sync::{Mutex, MutexGuard};

/// Maximum number of items a single page can return. Requests with larger
/// limits are silently clamped to prevent OOM and query failures.
const MAX_PAGE_LIMIT: u32 = 10_000;

/// Trim `items` to `options.limit` and produce a cursor for the next page when
/// the query returned one extra row.
pub(crate) fn paginate<T>(mut items: Vec<T>, options: ListOptionsInner) -> Page<T> {
    let limit = options.limit.clamp(1, MAX_PAGE_LIMIT) as usize;
    let has_more = items.len() > limit;
    if has_more {
        items.truncate(limit);
        let next_offset = options.offset.saturating_add(limit as u64);
        Page {
            items,
            next_cursor: Some(next_offset.to_string()),
        }
    } else {
        Page {
            items,
            next_cursor: None,
        }
    }
}

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

// Task-local storage for the connection currently managed by an active
// unit of work. Repository methods will reuse this connection (and its
// transaction) instead of acquiring a new one when it is present.
tokio::task_local! {
    static CURRENT_TX: Arc<Mutex<PoolConnection<Postgres>>>;
}

/// A database connection that is either owned by the caller or borrowed from
/// an active unit of work.
#[derive(Clone)]
pub struct ManagedTransaction {
    inner: Arc<Mutex<PoolConnection<Postgres>>>,
    owned: bool,
    finalized: Arc<AtomicBool>,
}

impl ManagedTransaction {
    /// Acquire the underlying PostgreSQL connection for the duration of the
    /// returned connection handle.
    pub async fn lock(&self) -> ManagedConnection<'_> {
        ManagedConnection {
            guard: self.inner.lock().await,
        }
    }

    /// Returns `true` if this transaction was started by the caller and must
    /// be committed/rolled back explicitly.
    pub const fn is_owned(&self) -> bool {
        self.owned
    }

    /// Commit an owned transaction. Does nothing for connections borrowed from a
    /// unit of work.
    pub async fn commit(self) -> Result<(), sqlx::Error> {
        if !self.owned {
            return Ok(());
        }
        let mut guard = self.inner.lock().await;
        sqlx::query("COMMIT").execute(&mut **guard).await?;
        self.finalized.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Roll back an owned transaction. Does nothing for connections borrowed
    /// from a unit of work.
    pub async fn rollback(self) -> Result<(), sqlx::Error> {
        if !self.owned {
            return Ok(());
        }
        let mut guard = self.inner.lock().await;
        sqlx::query("ROLLBACK").execute(&mut **guard).await?;
        self.finalized.store(true, Ordering::SeqCst);
        Ok(())
    }
}

impl Drop for ManagedTransaction {
    fn drop(&mut self) {
        if !self.owned || self.finalized.load(Ordering::SeqCst) {
            return;
        }
        // The transaction was not explicitly finalized; roll it back
        // asynchronously before the connection is returned to the pool.
        let inner = self.inner.clone();
        tokio::spawn(async move {
            let mut guard = inner.lock().await;
            let _ = sqlx::query("ROLLBACK").execute(&mut **guard).await;
        });
    }
}

/// RAII handle to a connection currently held by a [`ManagedTransaction`].
pub struct ManagedConnection<'a> {
    guard: MutexGuard<'a, PoolConnection<Postgres>>,
}

impl Deref for ManagedConnection<'_> {
    type Target = PgConnection;

    fn deref(&self) -> &Self::Target {
        &*self.guard
    }
}

impl DerefMut for ManagedConnection<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.guard
    }
}

/// Begin a PostgreSQL transaction and bind it to the tenant in `RequestContext`.
///
/// When called inside a [`storage_api::UnitOfWork`], the active connection is
/// reused. Otherwise a new connection and transaction are started. Tenant
/// binding is set with `SET LOCAL` so it is automatically cleared when the
/// connection is returned to the pool.
pub async fn begin_tenant_transaction(
    pool: &PgPool,
    context: &RequestContext,
) -> Result<ManagedTransaction, PlatformError> {
    if let Ok(conn) = CURRENT_TX.try_with(|c| c.clone()) {
        return Ok(ManagedTransaction {
            inner: conn,
            owned: false,
            finalized: Arc::new(AtomicBool::new(true)),
        });
    }

    let mut conn = pool.acquire().await.map_err(db_error)?;
    sqlx::query("BEGIN")
        .execute(&mut *conn)
        .await
        .map_err(db_error)?;
    set_tenant_context(&mut *conn, context).await?;

    Ok(ManagedTransaction {
        inner: Arc::new(Mutex::new(conn)),
        owned: true,
        finalized: Arc::new(AtomicBool::new(false)),
    })
}

/// Run the tenant and role configuration required for RLS on `conn`.
async fn set_tenant_context(
    conn: &mut PgConnection,
    context: &RequestContext,
) -> Result<(), PlatformError> {
    let value = context
        .tenant_id
        .map(|id| id.to_hyphenated())
        .unwrap_or_default();
    sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
        .bind(&value)
        .execute(&mut *conn)
        .await
        .map_err(db_error)?;

    // Ensure RLS policies are evaluated even when connected as a superuser
    // (e.g. in local development or tests). In production the app role is
    // already clsc_app, so this is a no-op.
    sqlx::query("SET LOCAL ROLE clsc_app")
        .execute(&mut *conn)
        .await
        .map_err(db_error)?;

    Ok(())
}

/// Clear the tenant context for the current connection. Used when returning a
/// connection to the pool to prevent cross-tenant leakage.
pub async fn clear_tenant_context(pool: &PgPool) -> Result<(), PlatformError> {
    sqlx::query("SELECT set_config('app.tenant_id', '', false)")
        .execute(pool)
        .await
        .map_err(db_error)?;
    Ok(())
}

fn db_error(e: sqlx::Error) -> PlatformError {
    match e {
        sqlx::Error::Database(ref db) if db.is_unique_violation() => {
            PlatformError::new(ErrorCode::Conflict, "resource already exists")
        }
        sqlx::Error::RowNotFound => PlatformError::new(ErrorCode::NotFound, "resource not found"),
        other => PlatformError::new(ErrorCode::Unavailable, other.to_string()),
    }
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
        let tx_managed = match begin_tenant_transaction(&pool, &context).await {
            Ok(tx) => tx,
            Err(e) => panic!("{e}"),
        };

        let mut tx = tx_managed.lock().await;
        let row: (String,) = sqlx::query_as("SELECT current_setting('app.tenant_id', true)")
            .fetch_one(&mut *tx)
            .await?;
        if let Some(tenant_id) = context.tenant_id {
            assert_eq!(row.0, tenant_id.to_hyphenated());
        } else {
            panic!("missing tenant id in context");
        }

        drop(tx);
        tx_managed.rollback().await?;
        Ok(())
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn tenant_context_is_cleared_when_empty(pool: PgPool) -> sqlx::Result<()> {
        let context = RequestContext::default();
        let tx_managed = match begin_tenant_transaction(&pool, &context).await {
            Ok(tx) => tx,
            Err(e) => panic!("{e}"),
        };

        let mut tx = tx_managed.lock().await;
        let row: (Option<String>,) =
            sqlx::query_as("SELECT nullif(current_setting('app.tenant_id', true), '')")
                .fetch_one(&mut *tx)
                .await?;
        assert!(row.0.is_none());

        drop(tx);
        tx_managed.rollback().await?;
        Ok(())
    }
}
