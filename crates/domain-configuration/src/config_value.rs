//! Configuration value aggregate scoped by platform, tenant, or module.

use crate::config_definition::{ConfigDefinition, ConfigValueType};
use foundation::{PlatformError, Revision, TenantId};
use serde::{Deserialize, Serialize};

/// Scope at which a configuration value applies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigScope {
    /// Platform-wide default. No tenant is attached.
    Platform,
    /// Tenant-scoped override.
    Tenant(TenantId),
    /// Module-scoped override within a tenant.
    Module { tenant_id: TenantId, module: String },
}

impl ConfigScope {
    /// Scope discriminator used for storage.
    pub fn scope_type(&self) -> &'static str {
        match self {
            Self::Platform => "platform",
            Self::Tenant(_) => "tenant",
            Self::Module { .. } => "module",
        }
    }

    /// Optional identifier for `Tenant` or `Module` scope.
    pub fn scope_id(&self) -> Option<String> {
        match self {
            Self::Platform => None,
            Self::Tenant(t) => Some(t.as_uuid().to_string()),
            Self::Module { module, .. } => Some(module.clone()),
        }
    }

    pub fn tenant_id(&self) -> Option<TenantId> {
        match self {
            Self::Platform => None,
            Self::Tenant(t) => Some(*t),
            Self::Module { tenant_id, .. } => Some(*tenant_id),
        }
    }
}

/// A concrete value assigned to a configuration key at a given scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigValue {
    pub id: Option<ConfigValueId>,
    pub scope: ConfigScope,
    pub config_key: String,
    pub raw_value: String,
    pub secret_ref: Option<String>,
    pub revision: Revision,
}

/// Strongly typed identifier for a configuration value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigValueId(pub foundation::uuid::Uuid);

impl ConfigValue {
    /// Create a new configuration value validated against its definition.
    pub fn new(
        id: Option<ConfigValueId>,
        scope: ConfigScope,
        definition: &ConfigDefinition,
        raw_value: impl Into<String>,
        secret_ref: Option<String>,
        revision: Revision,
    ) -> Result<Self, PlatformError> {
        let raw_value = raw_value.into();
        let config_key = definition.config_key.clone();

        if raw_value.trim().is_empty() {
            return Err(PlatformError::invalid(
                "value",
                "configuration value must not be empty",
            ));
        }

        if definition.sensitive {
            if secret_ref.is_none() {
                return Err(PlatformError::invalid(
                    "secret_ref",
                    "sensitive configuration values must use a secret reference",
                ));
            }
            if definition.value_type != ConfigValueType::Secret {
                return Err(PlatformError::invalid(
                    "value_type",
                    "sensitive definitions must have value type secret",
                ));
            }
        } else if secret_ref.is_some() {
            return Err(PlatformError::invalid(
                "secret_ref",
                "non-sensitive configuration values cannot use a secret reference",
            ));
        }

        let _ = definition.validate_value(&raw_value)?;

        Ok(Self {
            id,
            scope,
            config_key,
            raw_value,
            secret_ref,
            revision,
        })
    }

    /// Returns the effective value to be stored or returned.
    ///
    /// For sensitive definitions this is the secret reference, never the resolved secret.
    pub fn effective_value(&self) -> &str {
        self.secret_ref.as_deref().unwrap_or(&self.raw_value)
    }
}

/// Resolution precedence: module > tenant > platform > definition default.
pub fn resolve_config(
    definition: &ConfigDefinition,
    values: &[ConfigValue],
    tenant_id: Option<TenantId>,
    module: Option<&str>,
) -> Option<String> {
    let module_scope = module.and_then(|m| {
        tenant_id.map(|t| ConfigScope::Module {
            tenant_id: t,
            module: m.to_string(),
        })
    });

    if let Some(scope) = module_scope
        && let Some(v) = values
            .iter()
            .find(|v| v.scope == scope && v.config_key == definition.config_key)
    {
        return Some(v.effective_value().to_string());
    }

    if let Some(t) = tenant_id {
        let scope = ConfigScope::Tenant(t);
        if let Some(v) = values
            .iter()
            .find(|v| v.scope == scope && v.config_key == definition.config_key)
        {
            return Some(v.effective_value().to_string());
        }
    }

    if let Some(v) = values
        .iter()
        .find(|v| v.scope == ConfigScope::Platform && v.config_key == definition.config_key)
    {
        return Some(v.effective_value().to_string());
    }

    Some(definition.default_value.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_or_panic<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
        match result {
            Ok(v) => v,
            Err(e) => panic!("{e:?}"),
        }
    }

    fn def() -> ConfigDefinition {
        ok_or_panic(ConfigDefinition::new(
            "http.port",
            ConfigValueType::Integer,
            None,
            "8080",
            false,
            false,
        ))
    }

    fn tenant() -> TenantId {
        ok_or_panic(TenantId::parse_str("018e1234-5678-7abc-8def-0123456789ab"))
    }

    #[test]
    fn module_overrides_tenant_overrides_platform() {
        let platform = ConfigValue {
            id: None,
            scope: ConfigScope::Platform,
            config_key: "http.port".to_string(),
            raw_value: "8080".to_string(),
            secret_ref: None,
            revision: Revision::new(1),
        };
        let tenant_val = ConfigValue {
            id: None,
            scope: ConfigScope::Tenant(tenant()),
            config_key: "http.port".to_string(),
            raw_value: "3000".to_string(),
            secret_ref: None,
            revision: Revision::new(1),
        };
        let module_val = ConfigValue {
            id: None,
            scope: ConfigScope::Module {
                tenant_id: tenant(),
                module: "api".to_string(),
            },
            config_key: "http.port".to_string(),
            raw_value: "4000".to_string(),
            secret_ref: None,
            revision: Revision::new(1),
        };

        let values = vec![platform, tenant_val, module_val];
        assert_eq!(
            resolve_config(&def(), &values, Some(tenant()), Some("api")),
            Some("4000".to_string())
        );
        assert_eq!(
            resolve_config(&def(), &values, Some(tenant()), None),
            Some("3000".to_string())
        );
        assert_eq!(
            resolve_config(&def(), &values, None, None),
            Some("8080".to_string())
        );
    }

    #[test]
    fn sensitive_value_stores_secret_ref() {
        let def = ok_or_panic(ConfigDefinition::new(
            "db.password",
            ConfigValueType::Secret,
            None,
            "${secret:db.password}",
            true,
            false,
        ));
        let value = ok_or_panic(ConfigValue::new(
            None,
            ConfigScope::Platform,
            &def,
            "${secret:db.password}",
            Some("${secret:db.password}".to_string()),
            Revision::new(1),
        ));
        assert_eq!(value.effective_value(), "${secret:db.password}");
    }
}
