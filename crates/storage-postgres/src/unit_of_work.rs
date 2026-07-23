//! PostgreSQL unit-of-work adapter.

use std::future::Future;
use std::sync::Arc;

use async_trait::async_trait;
use foundation::{PlatformError, RequestContext};
use sqlx::PgPool;
use tokio::sync::Mutex;

use crate::{CURRENT_TX, db_error, set_tenant_context};

/// PostgreSQL-backed unit of work.
#[derive(Debug, Clone)]
pub struct PostgresUnitOfWork {
    pool: PgPool,
}

impl PostgresUnitOfWork {
    /// Create a new unit of work backed by `pool`.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl storage_api::UnitOfWork for PostgresUnitOfWork {
    async fn execute<F, Fut, T>(
        &self,
        ctx: &RequestContext,
        operation: F,
    ) -> Result<T, PlatformError>
    where
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = Result<T, PlatformError>> + Send,
        T: Send,
    {
        let mut conn = self.pool.acquire().await.map_err(db_error)?;
        sqlx::query("BEGIN")
            .execute(&mut *conn)
            .await
            .map_err(db_error)?;
        set_tenant_context(&mut *conn, ctx).await?;

        let conn = Arc::new(Mutex::new(Some(conn)));
        let result = CURRENT_TX.scope(conn.clone(), operation()).await;

        match result {
            Ok(value) => {
                let mut guard = conn.lock().await;
                if let Some(c) = guard.as_mut() {
                    sqlx::query("COMMIT")
                        .execute(&mut **c)
                        .await
                        .map_err(db_error)?;
                }
                Ok(value)
            }
            Err(error) => {
                let mut guard = conn.lock().await;
                if let Some(c) = guard.as_mut() {
                    let _ = sqlx::query("ROLLBACK").execute(&mut **c).await;
                }
                Err(error)
            }
        }
    }
}
