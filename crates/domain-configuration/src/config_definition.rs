//! Configuration definition aggregate.

use foundation::PlatformError;
use serde::{Deserialize, Serialize};

/// The runtime type of a configuration value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigValueType {
    String,
    Integer,
    Boolean,
    Duration,
    Secret,
    Json,
}

impl ConfigValueType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Integer => "integer",
            Self::Boolean => "boolean",
            Self::Duration => "duration",
            Self::Secret => "secret",
            Self::Json => "json",
        }
    }

    pub fn parse(input: &str) -> Result<Self, PlatformError> {
        match input {
            "string" => Ok(Self::String),
            "integer" => Ok(Self::Integer),
            "boolean" => Ok(Self::Boolean),
            "duration" => Ok(Self::Duration),
            "secret" => Ok(Self::Secret),
            "json" => Ok(Self::Json),
            _ => Err(PlatformError::invalid(
                "value_type",
                format!("unknown configuration value type: {input}"),
            )),
        }
    }

    /// Validate that `raw` conforms to this type and return a JSON representation.
    pub fn validate(&self, raw: &str) -> Result<serde_json::Value, PlatformError> {
        match self {
            Self::String => Ok(serde_json::Value::String(raw.to_string())),
            Self::Integer => {
                let n: i64 = raw.parse().map_err(|_| {
                    PlatformError::invalid("value", format!("'{raw}' is not an integer"))
                })?;
                Ok(serde_json::Value::Number(n.into()))
            }
            Self::Boolean => match raw {
                "true" => Ok(serde_json::Value::Bool(true)),
                "false" => Ok(serde_json::Value::Bool(false)),
                _ => Err(PlatformError::invalid(
                    "value",
                    format!("'{raw}' is not a boolean"),
                )),
            },
            Self::Duration => {
                let _ = humantime::parse_duration(raw).map_err(|e| {
                    PlatformError::invalid("value", format!("'{raw}' is not a duration: {e}"))
                })?;
                Ok(serde_json::Value::String(raw.to_string()))
            }
            Self::Secret => Ok(serde_json::Value::String(raw.to_string())),
            Self::Json => serde_json::from_str(raw).map_err(|e| {
                PlatformError::invalid("value", format!("'{raw}' is not valid JSON: {e}"))
            }),
        }
    }
}

/// Platform-level definition for a configuration key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigDefinition {
    pub config_key: String,
    pub value_type: ConfigValueType,
    pub schema: Option<String>,
    pub default_value: String,
    pub sensitive: bool,
    pub dynamic: bool,
}

impl ConfigDefinition {
    /// Create a new configuration definition.
    pub fn new(
        config_key: impl AsRef<str>,
        value_type: ConfigValueType,
        schema: Option<String>,
        default_value: impl AsRef<str>,
        sensitive: bool,
        dynamic: bool,
    ) -> Result<Self, PlatformError> {
        validate_config_key(config_key.as_ref())?;
        let default_value = default_value.as_ref();
        validate_config_value(default_value, "default_value")?;
        if sensitive && value_type != ConfigValueType::Secret {
            return Err(PlatformError::invalid(
                "sensitive",
                "sensitive definitions must use the secret value type",
            ));
        }
        let _ = value_type.validate(default_value)?;
        if let Some(ref schema) = schema {
            validate_config_value(schema, "schema")?;
        }
        Ok(Self {
            config_key: config_key.as_ref().to_string(),
            value_type,
            schema,
            default_value: default_value.to_string(),
            sensitive,
            dynamic,
        })
    }

    /// Validate a raw value against this definition.
    pub fn validate_value(&self, raw: &str) -> Result<serde_json::Value, PlatformError> {
        let value = self.value_type.validate(raw)?;
        if let Some(ref schema) = self.schema {
            validate_json_schema(schema, &value)?;
        }
        Ok(value)
    }

    /// Determine whether this definition must use a secret reference instead of a plain value.
    pub const fn is_secret(&self) -> bool {
        self.sensitive
    }
}

fn validate_json_schema(_schema: &str, _value: &serde_json::Value) -> Result<(), PlatformError> {
    // Schema validation is intentionally a no-op in the current domain layer.
    // Concrete JSON Schema validation can be added later without changing the interface.
    Ok(())
}

pub(crate) fn validate_config_key(key: &str) -> Result<(), PlatformError> {
    if key.trim().is_empty() {
        return Err(PlatformError::invalid(
            "config_key",
            "configuration key must not be empty",
        ));
    }
    if key.len() > crate::MAX_CONFIG_KEY_BYTES {
        return Err(PlatformError::invalid(
            "config_key",
            "configuration key exceeds maximum length",
        ));
    }
    Ok(())
}

pub(crate) fn validate_config_value(value: &str, field: &'static str) -> Result<(), PlatformError> {
    if value.trim().is_empty() {
        return Err(PlatformError::invalid(field, "value must not be empty"));
    }
    if value.len() > crate::MAX_CONFIG_VALUE_BYTES {
        return Err(PlatformError::invalid(
            field,
            "value exceeds maximum length",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_definition_is_created() {
        let def = ConfigDefinition::new(
            "http.port",
            ConfigValueType::Integer,
            None,
            "8080",
            false,
            false,
        );
        assert!(def.is_ok());
    }

    #[test]
    fn sensitive_requires_secret_type() {
        assert!(
            ConfigDefinition::new(
                "db.password",
                ConfigValueType::String,
                None,
                "default",
                true,
                false,
            )
            .is_err()
        );
    }

    #[test]
    fn invalid_default_value_is_rejected() {
        assert!(
            ConfigDefinition::new(
                "http.port",
                ConfigValueType::Integer,
                None,
                "not-an-integer",
                false,
                false,
            )
            .is_err()
        );
    }

    #[test]
    fn value_type_validation_works() {
        let def = ConfigDefinition::new(
            "feature.flag",
            ConfigValueType::Boolean,
            None,
            "false",
            false,
            true,
        )
        .unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(
            def.validate_value("true")
                .unwrap_or_else(|e| panic!("{e:?}")),
            serde_json::Value::Bool(true)
        );
        assert!(def.validate_value("nope").is_err());
    }
}
