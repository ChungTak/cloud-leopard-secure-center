//! Layered configuration and secret resolution.

use crate::PlatformError;
use serde::Deserialize;
use std::path::Path;
use zeroize::{Zeroize, ZeroizeOnDrop};

const MAX_SECRET_VALUE_BYTES: usize = 64 * 1024;

/// Reference to a secret stored outside the configuration file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(transparent)]
pub struct SecretRef(String);

impl SecretRef {
    /// Parse a non-empty secret reference.
    pub fn parse(input: &str) -> Result<Self, PlatformError> {
        if input.is_empty() {
            return Err(PlatformError::invalid(
                "secret_ref",
                "secret reference must not be empty".to_string(),
            ));
        }
        Ok(Self(input.to_string()))
    }

    /// Borrow the inner reference string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A secret value that is zeroized on drop and cannot be accidentally logged.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SecretValue(String);

impl SecretValue {
    /// Create a secret value from a string, capping its byte length to prevent
    /// oversized secrets from being carried through the configuration layer.
    pub fn new(value: impl AsRef<str>) -> Result<Self, PlatformError> {
        let value = value.as_ref();
        if value.len() > MAX_SECRET_VALUE_BYTES {
            return Err(PlatformError::invalid(
                "secret",
                "secret value exceeds maximum length",
            ));
        }
        Ok(Self(value.to_string()))
    }

    /// Expose the secret as a string slice.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}

/// Port for resolving secrets. Adapters provide concrete implementations.
pub trait SecretPort: Send + Sync {
    /// Resolve a secret reference to its value.
    fn resolve(&self, reference: &SecretRef) -> Result<SecretValue, PlatformError>;
}

/// Environment-backed secret provider.
#[derive(Debug, Default, Clone, Copy)]
pub struct EnvSecretProvider;

impl SecretPort for EnvSecretProvider {
    fn resolve(&self, reference: &SecretRef) -> Result<SecretValue, PlatformError> {
        let key = reference.as_str().to_ascii_uppercase().replace(".", "__");
        let var = format!("CLSC_SECRET_{key}");
        let value = std::env::var(&var)
            .map_err(|_| PlatformError::invalid("secret", "secret not found".to_string()))?;
        SecretValue::new(value)
    }
}

/// Worker count with a sensible upper bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "u16")]
pub struct WorkerCount(u16);

impl WorkerCount {
    /// Create a worker count, enforcing bounds.
    pub fn new(value: u16) -> Result<Self, PlatformError> {
        if value == 0 {
            return Err(PlatformError::invalid(
                "worker_count",
                "worker count must be at least 1".to_string(),
            ));
        }
        if value > 4096 {
            return Err(PlatformError::invalid(
                "worker_count",
                "worker count must not exceed 4096".to_string(),
            ));
        }
        Ok(Self(value))
    }

    /// Access the value.
    pub const fn value(&self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for WorkerCount {
    type Error = PlatformError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// Network port with validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "u16")]
pub struct Port(u16);

impl Port {
    /// Create a port, rejecting privileged ports below 1024.
    pub fn new(value: u16) -> Result<Self, PlatformError> {
        if value < 1024 {
            return Err(PlatformError::invalid(
                "port",
                "port must be >= 1024".to_string(),
            ));
        }
        Ok(Self(value))
    }

    /// Access the value.
    pub const fn value(&self) -> u16 {
        self.0
    }
}

impl TryFrom<u16> for Port {
    type Error = PlatformError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// System-wide settings.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SystemConfig {
    /// Log level.
    pub log_level: String,
}

/// HTTP server settings.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpConfig {
    /// Bind address.
    pub host: String,
    /// Bind port.
    pub port: Port,
    /// Trusted proxy CIDRs. Forwarded `X-Forwarded-For` is only accepted from these addresses.
    #[serde(default)]
    pub trusted_proxies: Vec<String>,
}

/// Storage settings.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageConfig {
    /// Connection URL. May be a secret reference.
    #[serde(with = "secret_or_plain")]
    pub url: SecretValue,
    /// Maximum connection pool size.
    pub max_connections: u32,
}

/// Rate limit window configuration.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RateLimitConfig {
    /// Maximum number of requests allowed in the window.
    #[serde(default = "default_rate_limit_requests")]
    pub requests: u32,
    /// Window size in seconds.
    #[serde(default = "default_rate_limit_window")]
    pub window_seconds: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests: default_rate_limit_requests(),
            window_seconds: default_rate_limit_window(),
        }
    }
}

fn default_rate_limit_requests() -> u32 {
    10
}

fn default_rate_limit_window() -> u64 {
    60
}

/// Security settings.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecurityConfig {
    /// Token expiry in seconds.
    pub token_expiry_seconds: u64,
    /// Issuer string.
    pub issuer: String,
    /// Token audience.
    #[serde(default)]
    pub audience: String,
    /// Rate limit for pre-login endpoints.
    #[serde(default)]
    pub login_rate_limit: RateLimitConfig,
    /// Rate limit for authenticated API endpoints.
    #[serde(default)]
    pub api_rate_limit: RateLimitConfig,
}

/// Observability settings.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservabilityConfig {
    /// Trace collector endpoint, if any.
    pub trace_collector: Option<String>,
    /// Metrics prefix.
    pub metrics_prefix: String,
}

/// Runtime settings.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeConfig {
    /// Number of async worker threads.
    pub worker_threads: WorkerCount,
    /// Max blocking pool size.
    pub max_blocking_threads: WorkerCount,
}

/// Full application configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppConfig {
    /// System section.
    pub system: SystemConfig,
    /// HTTP section.
    pub http: HttpConfig,
    /// Storage section.
    pub storage: StorageConfig,
    /// Security section.
    pub security: SecurityConfig,
    /// Observability section.
    pub observability: ObservabilityConfig,
    /// Runtime section.
    pub runtime: RuntimeConfig,
}

impl AppConfig {
    /// Default configuration with safe placeholder values.
    pub fn default_config() -> Self {
        Self {
            system: SystemConfig {
                log_level: "info".to_string(),
            },
            http: HttpConfig {
                host: "127.0.0.1".to_string(),
                port: Port(8080),
                trusted_proxies: Vec::new(),
            },
            storage: StorageConfig {
                url: SecretValue::new("placeholder")
                    .unwrap_or_else(|e| panic!("placeholder secret is within bounds: {e}")),
                max_connections: 10,
            },
            security: SecurityConfig {
                token_expiry_seconds: 3600,
                issuer: "clsc".to_string(),
                audience: "clsc-api".to_string(),
                login_rate_limit: RateLimitConfig::default(),
                api_rate_limit: RateLimitConfig::default(),
            },
            observability: ObservabilityConfig {
                trace_collector: None,
                metrics_prefix: "clsc".to_string(),
            },
            runtime: RuntimeConfig {
                worker_threads: WorkerCount(4),
                max_blocking_threads: WorkerCount(16),
            },
        }
    }

    /// Load configuration with precedence: defaults, then file, then environment overrides.
    pub fn load<P: AsRef<Path>>(
        path: Option<P>,
        secret_port: &dyn SecretPort,
    ) -> Result<Self, PlatformError> {
        let mut config = Self::default_config();

        if let Some(p) = path {
            let text = std::fs::read_to_string(p.as_ref())
                .map_err(|e| PlatformError::invalid("config_file", e.to_string()))?;
            let file_config: FileConfig = toml::from_str(&text)
                .map_err(|e| PlatformError::invalid("config_file", e.to_string()))?;
            config.merge(file_config);
        }

        if let Ok(overrides) = std::env::var("CLSC_OVERRIDES") {
            let env_config: FileConfig = toml::from_str(&overrides)
                .map_err(|e| PlatformError::invalid("CLSC_OVERRIDES", e.to_string()))?;
            config.merge(env_config);
        }

        config.storage.url = resolve_secret(&config.storage.url, secret_port)?;

        Ok(config)
    }

    fn merge(&mut self, file: FileConfig) {
        if let Some(v) = file.system {
            self.system = v;
        }
        if let Some(v) = file.http {
            self.http = v;
        }
        if let Some(v) = file.storage {
            self.storage = v;
        }
        if let Some(v) = file.security {
            self.security = v;
        }
        if let Some(v) = file.observability {
            self.observability = v;
        }
        if let Some(v) = file.runtime {
            self.runtime = v;
        }
    }
}

/// Partial config used for file/env overrides.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    system: Option<SystemConfig>,
    http: Option<HttpConfig>,
    storage: Option<StorageConfig>,
    security: Option<SecurityConfig>,
    observability: Option<ObservabilityConfig>,
    runtime: Option<RuntimeConfig>,
}

fn resolve_secret(
    value: &SecretValue,
    secret_port: &dyn SecretPort,
) -> Result<SecretValue, PlatformError> {
    let text = value.expose();
    if let Some(ref_) = text.strip_prefix("${secret:")
        && let Some(key) = ref_.strip_suffix("}")
    {
        return secret_port.resolve(&SecretRef::parse(key)?);
    }
    Ok(value.clone())
}

mod secret_or_plain {
    use super::SecretValue;
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<SecretValue, D::Error> {
        let text = String::deserialize(deserializer)?;
        SecretValue::new(text).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let config = AppConfig::default_config();
        assert_eq!(config.http.port.value(), 8080);
    }

    #[test]
    fn file_override_preserves_defaults() -> Result<(), PlatformError> {
        let text = r#"
[http]
host = "0.0.0.0"
port = 3000
"#;
        std::fs::write("/tmp/clsc-test-config.toml", text)
            .map_err(|e| PlatformError::invalid("test_write", e.to_string()))?;
        let config = AppConfig::load(Some("/tmp/clsc-test-config.toml"), &EnvSecretProvider)?;
        assert_eq!(config.http.host, "0.0.0.0");
        assert_eq!(config.http.port.value(), 3000);
        assert_eq!(config.system.log_level, "info");
        Ok(())
    }

    #[test]
    fn unknown_field_rejected() {
        let text = r#"
[http]
unknown_field = 1
"#;
        let result: Result<AppConfig, _> = toml::from_str(text);
        assert!(result.is_err());
    }

    #[test]
    fn secret_reference_resolved() -> Result<(), PlatformError> {
        let text = r#"
[storage]
url = "${secret:db.password}"
max_connections = 5
"#;
        let file_config: FileConfig = toml::from_str(text)
            .map_err(|e| PlatformError::invalid("test_parse", e.to_string()))?;
        let mut base = AppConfig::default_config();
        base.merge(file_config);

        let provider = StaticSecretProvider::new([(
            "db.password".to_string(),
            "resolved-password".to_string(),
        )]);
        let resolved = resolve_secret(&base.storage.url, &provider)?;
        assert_eq!(resolved.expose(), "resolved-password");
        Ok(())
    }

    #[derive(Default)]
    struct StaticSecretProvider(std::collections::HashMap<String, String>);

    impl StaticSecretProvider {
        fn new(entries: impl IntoIterator<Item = (String, String)>) -> Self {
            Self(entries.into_iter().collect())
        }
    }

    impl SecretPort for StaticSecretProvider {
        fn resolve(&self, reference: &SecretRef) -> Result<SecretValue, PlatformError> {
            let value =
                self.0.get(reference.as_str()).cloned().ok_or_else(|| {
                    PlatformError::invalid("secret", "secret not found".to_string())
                })?;
            SecretValue::new(value)
        }
    }

    #[test]
    fn worker_count_bounds() {
        assert!(WorkerCount::new(0).is_err());
        assert!(WorkerCount::new(4097).is_err());
        assert!(WorkerCount::new(4).is_ok());
    }
}
