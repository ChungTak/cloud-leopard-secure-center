//! Plugin manifest, lifecycle, and repository port.
//!
//! Phase 1 freezes the manifest shape and lifecycle states. Real Ed25519
//! signature/ checksum / SBOM / dependency verification is deferred.

use std::collections::{HashMap, HashSet};

use foundation::{PluginId, TenantId};

const MAX_VERSION_LEN: usize = 64;
const MAX_API_RANGE_LEN: usize = 64;
const MAX_DIGEST_LEN: usize = 1024;
const MAX_PUBLISHER_LEN: usize = 256;
const MAX_SIGNATURE_LEN: usize = 4096;
const MAX_CHECKSUM_LEN: usize = 1024;
const MAX_STRING_SET_SIZE: usize = 256;
const MAX_SET_ITEM_LEN: usize = 256;
const MAX_METADATA_ENTRIES: usize = 64;
const MAX_METADATA_KEY_LEN: usize = 64;
const MAX_METADATA_VALUE_LEN: usize = 1024;

/// Plugin kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PluginKind {
    Wasm,
    Process,
}

/// Plugin lifecycle state. Illegal transitions are rejected by the aggregate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum PluginState {
    Uploaded,
    Verified,
    Installed,
    Migrated,
    Enabled,
    Disabled,
    Quarantined,
}

/// Manifest of a plugin.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PluginManifest {
    pub plugin_id: PluginId,
    pub tenant_id: TenantId,
    pub version: String,
    pub kind: PluginKind,
    pub api_range: String,
    pub capabilities: HashSet<String>,
    pub resources: HashSet<String>,
    pub events: HashSet<String>,
    pub config_digest: String,
    pub publisher: String,
    pub signature: String,
    pub checksum: String,
}

impl PluginManifest {
    /// Validate the manifest shape. Real signature/ checksum verification is
    /// deferred to the verifier port.
    pub fn validate(&self) -> Result<(), PluginError> {
        validate_manifest_string(&self.version, "version", MAX_VERSION_LEN)?;
        validate_manifest_string(&self.api_range, "api_range", MAX_API_RANGE_LEN)?;
        validate_manifest_string(&self.config_digest, "config_digest", MAX_DIGEST_LEN)?;
        validate_manifest_string(&self.publisher, "publisher", MAX_PUBLISHER_LEN)?;
        validate_manifest_string(&self.signature, "signature", MAX_SIGNATURE_LEN)?;
        validate_manifest_string(&self.checksum, "checksum", MAX_CHECKSUM_LEN)?;
        validate_manifest_set(
            &self.capabilities,
            "capabilities",
            MAX_STRING_SET_SIZE,
            MAX_SET_ITEM_LEN,
        )?;
        validate_manifest_set(
            &self.resources,
            "resources",
            MAX_STRING_SET_SIZE,
            MAX_SET_ITEM_LEN,
        )?;
        validate_manifest_set(
            &self.events,
            "events",
            MAX_STRING_SET_SIZE,
            MAX_SET_ITEM_LEN,
        )?;
        Ok(())
    }
}

/// Plugin aggregate.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Plugin {
    pub plugin_id: PluginId,
    pub tenant_id: TenantId,
    pub state: PluginState,
    pub manifest: PluginManifest,
    pub metadata: HashMap<String, String>,
}

impl Plugin {
    /// Create a plugin in the `Uploaded` state.
    pub fn upload(manifest: PluginManifest) -> Result<Self, PluginError> {
        manifest.validate()?;
        Ok(Self {
            plugin_id: manifest.plugin_id,
            tenant_id: manifest.tenant_id,
            state: PluginState::Uploaded,
            manifest,
            metadata: HashMap::new(),
        })
    }

    /// Attach bounded metadata to a plugin.
    pub fn with_metadata(mut self, metadata: HashMap<String, String>) -> Result<Self, PluginError> {
        validate_plugin_metadata(&metadata)?;
        self.metadata = metadata;
        Ok(self)
    }

    /// Move to the next state if the transition is legal.
    pub fn transition(&mut self, next: PluginState) -> Result<(), PluginError> {
        let legal = matches!(
            (self.state, next),
            (PluginState::Uploaded, PluginState::Verified)
                | (PluginState::Verified, PluginState::Installed)
                | (PluginState::Installed, PluginState::Migrated)
                | (PluginState::Migrated, PluginState::Enabled)
                | (PluginState::Enabled, PluginState::Disabled)
                | (PluginState::Disabled, PluginState::Enabled)
                | (PluginState::Verified, PluginState::Quarantined)
                | (PluginState::Installed, PluginState::Quarantined)
                | (PluginState::Migrated, PluginState::Quarantined)
                | (PluginState::Enabled, PluginState::Quarantined)
                | (PluginState::Disabled, PluginState::Quarantined)
        );
        if !legal {
            return Err(PluginError::new(
                PluginErrorKind::Invalid,
                "illegal plugin state transition",
            ));
        }
        self.state = next;
        Ok(())
    }
}

fn validate_manifest_string(value: &str, field: &str, max: usize) -> Result<(), PluginError> {
    if value.trim().is_empty() || value.len() > max {
        return Err(PluginError::new(
            PluginErrorKind::Invalid,
            format!("{field} is empty or exceeds maximum length"),
        ));
    }
    Ok(())
}

fn validate_manifest_set(
    set: &HashSet<String>,
    field: &str,
    max_size: usize,
    max_item_len: usize,
) -> Result<(), PluginError> {
    if set.len() > max_size {
        return Err(PluginError::new(
            PluginErrorKind::Invalid,
            format!("{field} exceeds maximum size"),
        ));
    }
    for item in set {
        if item.trim().is_empty() || item.len() > max_item_len {
            return Err(PluginError::new(
                PluginErrorKind::Invalid,
                format!("{field} contains empty or oversized item"),
            ));
        }
    }
    Ok(())
}

fn validate_plugin_metadata(metadata: &HashMap<String, String>) -> Result<(), PluginError> {
    if metadata.len() > MAX_METADATA_ENTRIES {
        return Err(PluginError::new(
            PluginErrorKind::Invalid,
            "plugin metadata exceeds maximum number of entries",
        ));
    }
    for (key, value) in metadata {
        if key.trim().is_empty() || key.len() > MAX_METADATA_KEY_LEN {
            return Err(PluginError::new(
                PluginErrorKind::Invalid,
                "plugin metadata key is empty or too long",
            ));
        }
        if value.len() > MAX_METADATA_VALUE_LEN {
            return Err(PluginError::new(
                PluginErrorKind::Invalid,
                "plugin metadata value is too long",
            ));
        }
    }
    Ok(())
}

/// Plugin domain error.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error("{kind:?}: {message}")]
pub struct PluginError {
    pub kind: PluginErrorKind,
    pub message: String,
}

impl PluginError {
    pub fn new(kind: PluginErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

/// Kinds of plugin failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PluginErrorKind {
    Invalid,
    NotFound,
    Unauthorized,
    SignatureMismatch,
    Quarantined,
    Unsupported,
    Unavailable,
}

/// Verifier for manifest integrity and trust.
#[async_trait::async_trait]
pub trait ManifestVerifier: Send + Sync {
    async fn verify(&self, plugin: &Plugin) -> Result<(), PluginError>;
}

/// Placeholder verifier.
#[derive(Debug, Clone, Default)]
pub struct UnsupportedManifestVerifier;

#[async_trait::async_trait]
impl ManifestVerifier for UnsupportedManifestVerifier {
    async fn verify(&self, _plugin: &Plugin) -> Result<(), PluginError> {
        Err(PluginError::new(
            PluginErrorKind::Unsupported,
            "manifest verification is not implemented in this build",
        ))
    }
}

#[cfg(test)]
mod tests {
    use foundation::{SystemClock, SystemIdGenerator, SystemRandom};

    use super::*;

    fn ok_or_panic<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
        match result {
            Ok(v) => v,
            Err(e) => panic!("{e}"),
        }
    }

    fn err_or_panic<T, E: std::fmt::Debug>(result: Result<T, E>) -> E {
        match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        }
    }

    fn make_manifest() -> PluginManifest {
        let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
        PluginManifest {
            plugin_id: ok_or_panic(PluginId::generate(&generator)),
            tenant_id: ok_or_panic(TenantId::generate(&generator)),
            version: "0.1.0".to_string(),
            kind: PluginKind::Wasm,
            api_range: "v1".to_string(),
            capabilities: HashSet::new(),
            resources: HashSet::new(),
            events: HashSet::new(),
            config_digest: "sha256:abc".to_string(),
            publisher: "publisher".to_string(),
            signature: "sig".to_string(),
            checksum: "sum".to_string(),
        }
    }

    #[test]
    fn plugin_starts_uploaded() {
        let plugin = ok_or_panic(Plugin::upload(make_manifest()));
        assert_eq!(plugin.state, PluginState::Uploaded);
    }

    #[test]
    fn verify_then_install_is_legal() {
        let mut plugin = ok_or_panic(Plugin::upload(make_manifest()));
        ok_or_panic(plugin.transition(PluginState::Verified));
        ok_or_panic(plugin.transition(PluginState::Installed));
    }

    #[test]
    fn enable_from_uploaded_is_illegal() {
        let mut plugin = ok_or_panic(Plugin::upload(make_manifest()));
        let result = err_or_panic(plugin.transition(PluginState::Enabled));
        assert_eq!(result.kind, PluginErrorKind::Invalid);
    }

    #[tokio::test]
    async fn verifier_returns_unsupported() {
        let plugin = ok_or_panic(Plugin::upload(make_manifest()));
        let verifier = UnsupportedManifestVerifier;
        let result = err_or_panic(verifier.verify(&plugin).await);
        assert_eq!(result.kind, PluginErrorKind::Unsupported);
    }
}
