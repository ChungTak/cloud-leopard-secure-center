//! Release artifacts, versioning, and verification.
//!
//! Phase 1 freezes the artifact manifest shape and verification port. Real build
//! pipelines, OCI image creation, SBOM generation, signing, and offline package
//! validation are deferred.

const MAX_PRERELEASE_LEN: usize = 64;
const MAX_ARTIFACT_NAME_LEN: usize = 256;
const MAX_DIGEST_LEN: usize = 1024;
const MAX_SIGNATURE_LEN: usize = 4096;
const MAX_CHECKSUM_LEN: usize = 1024;
const MAX_ARTIFACT_PATH_LEN: usize = 4096;
const MAX_ARTIFACTS: usize = 256;

/// Semantic version for platform/API/proto/WIT/plugins.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SemanticVersion {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub prerelease: Option<String>,
}

impl SemanticVersion {
    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
            prerelease: None,
        }
    }

    /// Whether `self` is backwards-compatible with `other` for v1 public API.
    pub fn is_compatible_with(&self, other: &Self) -> bool {
        self.major == other.major && self.major >= 1
    }

    /// Validate the version, including prerelease string length.
    pub fn validate(&self) -> Result<(), ReleaseError> {
        if let Some(prerelease) = &self.prerelease
            && (prerelease.trim().is_empty() || prerelease.len() > MAX_PRERELEASE_LEN)
        {
            return Err(ReleaseError::new(
                ReleaseErrorKind::Invalid,
                "prerelease is empty or exceeds maximum length",
            ));
        }
        Ok(())
    }
}

/// Artifact kind produced by the release pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ArtifactKind {
    PlatformBinary,
    WebBundle,
    OciImage,
    Migration,
    Config,
    Sbom,
    Signature,
    Checksum,
    Plugin,
}

/// A single release artifact.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReleaseArtifact {
    pub name: String,
    pub version: SemanticVersion,
    pub kind: ArtifactKind,
    pub digest: String,
    pub signature: Option<String>,
    pub checksum: Option<String>,
    /// Absolute or relative path; in CI this is the artifact URL.
    pub path: String,
}

impl ReleaseArtifact {
    /// Validate the artifact fields and version.
    pub fn validate(&self) -> Result<(), ReleaseError> {
        if self.name.trim().is_empty() || self.name.len() > MAX_ARTIFACT_NAME_LEN {
            return Err(ReleaseError::new(
                ReleaseErrorKind::Invalid,
                "artifact name is empty or exceeds maximum length",
            ));
        }
        self.version.validate()?;
        if self.digest.trim().is_empty() || self.digest.len() > MAX_DIGEST_LEN {
            return Err(ReleaseError::new(
                ReleaseErrorKind::Invalid,
                "artifact digest is empty or exceeds maximum length",
            ));
        }
        if let Some(signature) = &self.signature
            && signature.len() > MAX_SIGNATURE_LEN
        {
            return Err(ReleaseError::new(
                ReleaseErrorKind::Invalid,
                "artifact signature exceeds maximum length",
            ));
        }
        if let Some(checksum) = &self.checksum
            && checksum.len() > MAX_CHECKSUM_LEN
        {
            return Err(ReleaseError::new(
                ReleaseErrorKind::Invalid,
                "artifact checksum exceeds maximum length",
            ));
        }
        if self.path.trim().is_empty() || self.path.len() > MAX_ARTIFACT_PATH_LEN {
            return Err(ReleaseError::new(
                ReleaseErrorKind::Invalid,
                "artifact path is empty or exceeds maximum length",
            ));
        }
        Ok(())
    }
}

/// Release manifest for an offline installable package.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReleaseManifest {
    pub platform_version: SemanticVersion,
    pub api_version: SemanticVersion,
    pub proto_version: SemanticVersion,
    pub wit_version: SemanticVersion,
    pub artifacts: Vec<ReleaseArtifact>,
    /// Whether the installer must not download anything at runtime.
    pub offline_capable: bool,
}

impl ReleaseManifest {
    /// Validate that all required artifact kinds are present and versioned.
    pub fn validate(&self) -> Result<(), ReleaseError> {
        self.platform_version.validate()?;
        self.api_version.validate()?;
        self.proto_version.validate()?;
        self.wit_version.validate()?;
        if self.artifacts.len() > MAX_ARTIFACTS {
            return Err(ReleaseError::new(
                ReleaseErrorKind::Invalid,
                "too many release artifacts",
            ));
        }
        let required = [
            ArtifactKind::PlatformBinary,
            ArtifactKind::Migration,
            ArtifactKind::Config,
        ];
        let present: std::collections::HashSet<_> = self.artifacts.iter().map(|a| a.kind).collect();
        for kind in required {
            if !present.contains(&kind) {
                return Err(ReleaseError::new(
                    ReleaseErrorKind::Invalid,
                    format!("missing required artifact kind: {kind:?}"),
                ));
            }
        }
        for artifact in &self.artifacts {
            artifact.validate()?;
        }
        if !self.offline_capable {
            return Err(ReleaseError::new(
                ReleaseErrorKind::Invalid,
                "release manifest is not offline capable",
            ));
        }
        Ok(())
    }
}

/// Release error.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error("{kind:?}: {message}")]
pub struct ReleaseError {
    pub kind: ReleaseErrorKind,
    pub message: String,
}

impl ReleaseError {
    pub fn new(kind: ReleaseErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

/// Kinds of release failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReleaseErrorKind {
    Invalid,
    SignatureMismatch,
    ChecksumMismatch,
    Unsupported,
    Unavailable,
}

/// Verifies artifact signatures/checksums and installer requirements.
#[async_trait::async_trait]
pub trait ArtifactVerifier: Send + Sync {
    async fn verify(&self, manifest: &ReleaseManifest) -> Result<(), ReleaseError>;
}

/// Placeholder artifact verifier.
#[derive(Debug, Clone, Default)]
pub struct UnsupportedArtifactVerifier {
    enabled: bool,
}

impl UnsupportedArtifactVerifier {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

#[async_trait::async_trait]
impl ArtifactVerifier for UnsupportedArtifactVerifier {
    async fn verify(&self, manifest: &ReleaseManifest) -> Result<(), ReleaseError> {
        manifest.validate()?;
        if self.enabled {
            Err(ReleaseError::new(
                ReleaseErrorKind::Unsupported,
                "artifact verification is not implemented in this build",
            ))
        } else {
            Err(ReleaseError::new(
                ReleaseErrorKind::Unavailable,
                "artifact verifier is not configured",
            ))
        }
    }
}

/// Port for building release artifacts.
#[async_trait::async_trait]
pub trait ReleaseBuilder: Send + Sync {
    async fn build(&self, version: &SemanticVersion) -> Result<ReleaseManifest, ReleaseError>;
}

/// Placeholder release builder.
#[derive(Debug, Clone, Default)]
pub struct UnsupportedReleaseBuilder {
    enabled: bool,
}

impl UnsupportedReleaseBuilder {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

#[async_trait::async_trait]
impl ReleaseBuilder for UnsupportedReleaseBuilder {
    async fn build(&self, _version: &SemanticVersion) -> Result<ReleaseManifest, ReleaseError> {
        if self.enabled {
            Err(ReleaseError::new(
                ReleaseErrorKind::Unsupported,
                "release build pipeline is not implemented in this build",
            ))
        } else {
            Err(ReleaseError::new(
                ReleaseErrorKind::Unavailable,
                "release builder is not configured",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
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

    fn sample_manifest() -> ReleaseManifest {
        ReleaseManifest {
            platform_version: SemanticVersion::new(0, 1, 0),
            api_version: SemanticVersion::new(1, 0, 0),
            proto_version: SemanticVersion::new(1, 0, 0),
            wit_version: SemanticVersion::new(1, 0, 0),
            artifacts: vec![
                ReleaseArtifact {
                    name: "security-platform".to_string(),
                    version: SemanticVersion::new(0, 1, 0),
                    kind: ArtifactKind::PlatformBinary,
                    digest: "sha256:abc".to_string(),
                    signature: Some("sig".to_string()),
                    checksum: Some("sum".to_string()),
                    path: "/opt/bin/security-platform".to_string(),
                },
                ReleaseArtifact {
                    name: "migrations".to_string(),
                    version: SemanticVersion::new(0, 1, 0),
                    kind: ArtifactKind::Migration,
                    digest: "sha256:def".to_string(),
                    signature: None,
                    checksum: None,
                    path: "/opt/migrations".to_string(),
                },
                ReleaseArtifact {
                    name: "config".to_string(),
                    version: SemanticVersion::new(0, 1, 0),
                    kind: ArtifactKind::Config,
                    digest: "sha256:ghi".to_string(),
                    signature: None,
                    checksum: None,
                    path: "/opt/config".to_string(),
                },
            ],
            offline_capable: true,
        }
    }

    #[test]
    fn manifest_validation_requires_offline_and_artifacts() {
        let manifest = sample_manifest();
        ok_or_panic(manifest.validate());
    }

    #[test]
    fn manifest_missing_migration_fails() {
        let mut manifest = sample_manifest();
        manifest
            .artifacts
            .retain(|a| a.kind != ArtifactKind::Migration);
        let err = err_or_panic(manifest.validate());
        assert_eq!(err.kind, ReleaseErrorKind::Invalid);
    }

    #[tokio::test]
    async fn disabled_builder_returns_unavailable() {
        let builder = UnsupportedReleaseBuilder::new(false);
        let err = err_or_panic(builder.build(&SemanticVersion::new(0, 1, 0)).await);
        assert_eq!(err.kind, ReleaseErrorKind::Unavailable);
    }

    #[tokio::test]
    async fn disabled_verifier_returns_unavailable() {
        let verifier = UnsupportedArtifactVerifier::new(false);
        let err = err_or_panic(verifier.verify(&sample_manifest()).await);
        assert_eq!(err.kind, ReleaseErrorKind::Unavailable);
    }
}
