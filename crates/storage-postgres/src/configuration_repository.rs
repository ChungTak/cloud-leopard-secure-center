//! PostgreSQL configuration repository.

use crate::db_error;
use async_trait::async_trait;
use domain_configuration::{
    ConfigDefinition, ConfigScope, ConfigValue, ConfigValueId, resolve_config,
};
use foundation::{
    ErrorCode, IdGenerator, PlatformError, RequestContext, Revision, SystemClock,
    SystemIdGenerator, SystemRandom, TenantId,
};
use sqlx::{PgPool, Row};
use storage_api::ConfigurationRepository;

use crate::begin_tenant_transaction;

/// PostgreSQL-backed configuration repository.
#[derive(Debug, Clone)]
pub struct PostgresConfigurationRepository {
    pool: PgPool,
}

impl PostgresConfigurationRepository {
    /// Create a new repository backed by `pool`.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ConfigurationRepository for PostgresConfigurationRepository {
    async fn save_definition(&self, definition: &ConfigDefinition) -> Result<(), PlatformError> {
        let ctx = RequestContext::default();
        let tx_managed = begin_tenant_transaction(&self.pool, &ctx).await?;
        let mut tx = tx_managed.lock().await;

        sqlx::query(
            "INSERT INTO config.definitions (config_key, value_type, schema, default_value, sensitive, dynamic)
             VALUES ($1, $2, $3::jsonb, $4, $5, $6)
             ON CONFLICT (config_key) DO UPDATE SET
                 value_type = EXCLUDED.value_type,
                 schema = EXCLUDED.schema,
                 default_value = EXCLUDED.default_value,
                 sensitive = EXCLUDED.sensitive,
                 dynamic = EXCLUDED.dynamic",
        )
        .bind(&definition.config_key)
        .bind(definition.value_type.as_str())
        .bind(definition.schema.as_deref())
        .bind(&definition.default_value)
        .bind(definition.sensitive)
        .bind(definition.dynamic)
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;

        drop(tx);
        tx_managed.commit().await.map_err(db_error)
    }

    async fn get_definition(
        &self,
        config_key: &str,
    ) -> Result<Option<ConfigDefinition>, PlatformError> {
        let row = sqlx::query(
            "SELECT value_type, schema, default_value, sensitive, dynamic
             FROM config.definitions WHERE config_key = $1",
        )
        .bind(config_key)
        .fetch_optional(&self.pool)
        .await
        .map_err(db_error)?;

        row.map(|r| parse_definition(&r, config_key)).transpose()
    }

    async fn save_value(
        &self,
        value: &ConfigValue,
        ctx: &RequestContext,
    ) -> Result<ConfigValue, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let definition = self
            .get_definition_in_tx(&mut *tx, &value.config_key)
            .await?
            .ok_or_else(|| {
                PlatformError::new(
                    ErrorCode::NotFound,
                    format!("configuration definition not found: {}", value.config_key),
                )
            })?;

        if definition.config_key != value.config_key {
            return Err(PlatformError::invalid(
                "config_key",
                "value config key does not match definition",
            ));
        }

        let validated = if definition.sensitive {
            if value.secret_ref.is_none() {
                return Err(PlatformError::invalid(
                    "secret_ref",
                    "sensitive configuration value requires a secret reference",
                ));
            }
            None
        } else {
            Some(definition.validate_value(&value.raw_value)?)
        };

        let tenant_id = value.scope.tenant_id().map(|t| *t.as_uuid());
        let scope_type = value.scope.scope_type();
        let scope_id = value.scope.scope_id();
        let raw_value = value
            .secret_ref
            .as_deref()
            .unwrap_or(&value.raw_value)
            .to_string();

        let (id, revision): (foundation::uuid::Uuid, i64) = if let Some(id) = value.id {
            let current = value.revision.value() as i64;
            let row = sqlx::query(
                "UPDATE config.values
                 SET tenant_id = $1, scope_type = $2, scope_id = $3, config_key = $4,
                     value = $5::jsonb, raw_value = $6, secret_ref = $7, revision = revision + 1
                 WHERE config_value_id = $8 AND revision = $9
                 RETURNING config_value_id, revision",
            )
            .bind(tenant_id)
            .bind(scope_type)
            .bind(scope_id.as_deref())
            .bind(&value.config_key)
            .bind(validated.as_ref().map(|v| v.to_string()))
            .bind(&raw_value)
            .bind(value.secret_ref.as_deref())
            .bind(id.0)
            .bind(current)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db_error)?;

            row.map(|r| (r.get("config_value_id"), r.get("revision")))
                .ok_or_else(|| {
                    PlatformError::new(
                        ErrorCode::VersionMismatch,
                        "stale configuration value".to_string(),
                    )
                })?
        } else {
            let id = SystemIdGenerator::new(SystemClock, SystemRandom).generate()?;
            let row = sqlx::query(
                "INSERT INTO config.values
                 (config_value_id, tenant_id, scope_type, scope_id, config_key, value, raw_value, secret_ref, revision)
                 VALUES ($1, $2, $3, $4, $5, $6::jsonb, $7, $8, 0)
                 RETURNING config_value_id, revision",
            )
            .bind(id)
            .bind(tenant_id)
            .bind(scope_type)
            .bind(scope_id.as_deref())
            .bind(&value.config_key)
            .bind(validated.as_ref().map(|v| v.to_string()))
            .bind(&raw_value)
            .bind(value.secret_ref.as_deref())
            .fetch_one(&mut *tx)
            .await
            .map_err(db_error)?;
            (row.get("config_value_id"), row.get("revision"))
        };

        drop(tx);
        tx_managed.commit().await.map_err(db_error)?;

        Ok(ConfigValue {
            id: Some(ConfigValueId(id)),
            scope: value.scope.clone(),
            config_key: value.config_key.clone(),
            raw_value: raw_value.clone(),
            secret_ref: value.secret_ref.clone(),
            revision: Revision::new(revision as u64),
        })
    }

    async fn get_value(
        &self,
        config_key: &str,
        scope: &ConfigScope,
        ctx: &RequestContext,
    ) -> Result<Option<ConfigValue>, PlatformError> {
        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let _tenant_id = scope.tenant_id().map(|t| *t.as_uuid());
        let scope_type = scope.scope_type();
        let scope_id = scope.scope_id();

        let row = sqlx::query(
            "SELECT config_value_id, value, raw_value, secret_ref, revision
             FROM config.values
             WHERE config_key = $1 AND scope_type = $2
               AND scope_id IS NOT DISTINCT FROM $3",
        )
        .bind(config_key)
        .bind(scope_type)
        .bind(scope_id.as_deref())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        row.map(|r| parse_value(&r, scope.clone(), config_key))
            .transpose()
    }

    async fn resolve(
        &self,
        config_key: &str,
        tenant_id: Option<TenantId>,
        module: Option<&str>,
        ctx: &RequestContext,
    ) -> Result<Option<String>, PlatformError> {
        let definition = match self.get_definition(config_key).await? {
            Some(d) => d,
            None => return Ok(None),
        };

        let tx_managed = begin_tenant_transaction(&self.pool, ctx).await?;
        let mut tx = tx_managed.lock().await;

        let tenant_uuid = tenant_id.map(|t| *t.as_uuid());
        let rows = sqlx::query(
            "SELECT config_value_id, scope_type, scope_id, raw_value, secret_ref, revision
             FROM config.values
             WHERE config_key = $1 AND (tenant_id IS NULL OR tenant_id = $2)",
        )
        .bind(config_key)
        .bind(tenant_uuid)
        .fetch_all(&mut *tx)
        .await
        .map_err(db_error)?;

        let values: Result<Vec<ConfigValue>, PlatformError> = rows
            .into_iter()
            .map(|r| {
                let scope_type: String = r.get("scope_type");
                let scope_id: Option<String> = r.get("scope_id");
                let scope = parse_scope(&scope_type, scope_id.as_deref(), tenant_id)?;
                parse_value(&r, scope, config_key)
            })
            .collect();

        Ok(resolve_config(&definition, &values?, tenant_id, module))
    }
}

impl PostgresConfigurationRepository {
    async fn get_definition_in_tx(
        &self,
        tx: &mut sqlx::postgres::PgConnection,
        config_key: &str,
    ) -> Result<Option<ConfigDefinition>, PlatformError> {
        let row = sqlx::query(
            "SELECT value_type, schema, default_value, sensitive, dynamic
             FROM config.definitions WHERE config_key = $1",
        )
        .bind(config_key)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db_error)?;

        row.map(|r| parse_definition(&r, config_key)).transpose()
    }
}

fn parse_definition(
    row: &sqlx::postgres::PgRow,
    config_key: &str,
) -> Result<ConfigDefinition, PlatformError> {
    let value_type: String = row.get("value_type");
    let schema: Option<serde_json::Value> = row.get("schema");
    let schema = schema.map(|v| v.to_string());
    let default_value: String = row.get("default_value");
    let sensitive: bool = row.get("sensitive");
    let dynamic: bool = row.get("dynamic");

    ConfigDefinition::new(
        config_key,
        domain_configuration::ConfigValueType::parse(&value_type)?,
        schema,
        default_value,
        sensitive,
        dynamic,
    )
}

fn parse_value(
    row: &sqlx::postgres::PgRow,
    scope: ConfigScope,
    config_key: &str,
) -> Result<ConfigValue, PlatformError> {
    let id: foundation::uuid::Uuid = row.get("config_value_id");
    let raw_value: String = row.get("raw_value");
    let secret_ref: Option<String> = row.get("secret_ref");
    let revision: i64 = row.get("revision");

    Ok(ConfigValue {
        id: Some(ConfigValueId(id)),
        scope,
        config_key: config_key.to_string(),
        raw_value,
        secret_ref,
        revision: Revision::new(revision as u64),
    })
}

fn parse_scope(
    scope_type: &str,
    scope_id: Option<&str>,
    tenant_id: Option<TenantId>,
) -> Result<ConfigScope, PlatformError> {
    match scope_type {
        "platform" => Ok(ConfigScope::Platform),
        "tenant" => {
            let id = scope_id
                .map(TenantId::parse_str)
                .unwrap_or(Err(PlatformError::invalid(
                    "scope_id",
                    "tenant scope requires a tenant id",
                )))?;
            Ok(ConfigScope::Tenant(id))
        }
        "module" => {
            let tenant = tenant_id.ok_or_else(|| {
                PlatformError::invalid("tenant_id", "module scope requires a tenant")
            })?;
            let module = scope_id
                .ok_or_else(|| {
                    PlatformError::invalid("scope_id", "module scope requires a module name")
                })?
                .to_string();
            Ok(ConfigScope::Module {
                tenant_id: tenant,
                module,
            })
        }
        _ => Err(PlatformError::invalid(
            "scope_type",
            format!("unknown scope type: {scope_type}"),
        )),
    }
}
