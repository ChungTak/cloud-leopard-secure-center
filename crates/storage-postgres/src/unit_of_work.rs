//! PostgreSQL unit-of-work adapter.

use std::future::Future;

use async_trait::async_trait;
use foundation::{PlatformError, RequestContext};
use sqlx::PgPool;

use crate::{CURRENT_TX, begin_tenant_transaction, db_error};

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
        let tx = begin_tenant_transaction(&self.pool, ctx).await?;
        let conn = tx.connection();
        let result = CURRENT_TX.scope(conn, operation()).await;

        match result {
            Ok(value) => {
                tx.commit().await.map_err(db_error)?;
                Ok(value)
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error)
            }
        }
    }
}
