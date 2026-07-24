//! Security threat model and control matrix stubs.
//!
//! Phase 1 freezes the threat categories and control record shape. Real
//! automated assessment, mTLS identity mapping, and certificate rotation are
//! deferred.

use std::collections::HashMap;

const MAX_CONTROL_ID_LEN: usize = 256;
const MAX_OWNER_LEN: usize = 256;
const MAX_TEST_REF_LEN: usize = 1024;
const MAX_CONTROLS_PER_CATEGORY: usize = 256;
const MAX_CATEGORIES: usize = 64;

/// Threat categories relevant to the security platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ThreatCategory {
    TenantEscalation,
    IdConfusion,
    TokenReplay,
    StaleEpoch,
    PluginEscape,
    UrlLeak,
    Ssrf,
    AuditTampering,
}

/// A security control with owner and test reference.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SecurityControl {
    pub id: String,
    pub category: ThreatCategory,
    pub owner: String,
    pub test_ref: String,
    pub residual_risk: RiskLevel,
}

impl SecurityControl {
    /// Validate the control record fields.
    pub fn validate(&self) -> Result<(), SecurityError> {
        if self.id.trim().is_empty() || self.id.len() > MAX_CONTROL_ID_LEN {
            return Err(SecurityError::new(
                SecurityErrorKind::Invalid,
                "control id is empty or exceeds maximum length",
            ));
        }
        if self.owner.trim().is_empty() || self.owner.len() > MAX_OWNER_LEN {
            return Err(SecurityError::new(
                SecurityErrorKind::Invalid,
                "control owner is empty or exceeds maximum length",
            ));
        }
        if self.test_ref.trim().is_empty() || self.test_ref.len() > MAX_TEST_REF_LEN {
            return Err(SecurityError::new(
                SecurityErrorKind::Invalid,
                "control test_ref is empty or exceeds maximum length",
            ));
        }
        Ok(())
    }
}

/// Residual risk level for a control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Threat/control matrix.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ThreatControlMatrix {
    pub controls: HashMap<ThreatCategory, Vec<SecurityControl>>,
}

impl ThreatControlMatrix {
    /// Add a control to the matrix.
    pub fn add(&mut self, control: SecurityControl) {
        self.controls
            .entry(control.category)
            .or_default()
            .push(control);
    }

    /// Controls for a single category.
    pub fn for_category(&self, category: ThreatCategory) -> &[SecurityControl] {
        self.controls.get(&category).map_or(&[], |v| v.as_slice())
    }

    /// Validate the entire matrix and every control.
    pub fn validate(&self) -> Result<(), SecurityError> {
        if self.controls.len() > MAX_CATEGORIES {
            return Err(SecurityError::new(
                SecurityErrorKind::Invalid,
                "too many threat categories in matrix",
            ));
        }
        for (category, controls) in &self.controls {
            if controls.len() > MAX_CONTROLS_PER_CATEGORY {
                return Err(SecurityError::new(
                    SecurityErrorKind::Invalid,
                    format!("too many controls for category {category:?}"),
                ));
            }
            for control in controls {
                control.validate()?;
            }
        }
        Ok(())
    }
}

/// Security assessment error.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error("{kind:?}: {message}")]
pub struct SecurityError {
    pub kind: SecurityErrorKind,
    pub message: String,
}

impl SecurityError {
    pub fn new(kind: SecurityErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

/// Kinds of security assessment failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SecurityErrorKind {
    Unsupported,
    Unavailable,
    Unauthorized,
    Invalid,
}

/// Port for security assessment.
#[async_trait::async_trait]
pub trait SecurityAssessor: Send + Sync {
    async fn assess(&self, matrix: &ThreatControlMatrix) -> Result<Vec<String>, SecurityError>;
    async fn mtls_identity_matches(
        &self,
        node_id: foundation::NodeId,
        plugin_id: Option<foundation::PluginId>,
    ) -> Result<bool, SecurityError>;
}

/// Placeholder security assessor.
#[derive(Debug, Clone, Default)]
pub struct UnsupportedSecurityAssessor {
    enabled: bool,
}

impl UnsupportedSecurityAssessor {
    /// Create the assessor. When `enabled` is true it reports `Unsupported`;
    /// otherwise `Unavailable`.
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

#[async_trait::async_trait]
impl SecurityAssessor for UnsupportedSecurityAssessor {
    async fn assess(&self, matrix: &ThreatControlMatrix) -> Result<Vec<String>, SecurityError> {
        matrix.validate()?;
        if self.enabled {
            Err(SecurityError::new(
                SecurityErrorKind::Unsupported,
                "security assessment is not implemented in this build",
            ))
        } else {
            Err(SecurityError::new(
                SecurityErrorKind::Unavailable,
                "security assessor is not configured",
            ))
        }
    }

    async fn mtls_identity_matches(
        &self,
        _node_id: foundation::NodeId,
        _plugin_id: Option<foundation::PluginId>,
    ) -> Result<bool, SecurityError> {
        if self.enabled {
            Err(SecurityError::new(
                SecurityErrorKind::Unsupported,
                "mTLS identity matching is not implemented in this build",
            ))
        } else {
            Err(SecurityError::new(
                SecurityErrorKind::Unavailable,
                "mTLS identity matching is not configured",
            ))
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use foundation::{SystemClock, SystemIdGenerator, SystemRandom};

    use super::*;

    fn err_or_panic<T, E: std::fmt::Debug>(result: Result<T, E>) -> E {
        match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        }
    }

    #[test]
    fn matrix_can_hold_controls() {
        let mut matrix = ThreatControlMatrix::default();
        matrix.add(SecurityControl {
            id: "tenant-rls".to_string(),
            category: ThreatCategory::TenantEscalation,
            owner: "security".to_string(),
            test_ref: "tenant_escalation_test".to_string(),
            residual_risk: RiskLevel::Low,
        });
        assert_eq!(
            matrix.for_category(ThreatCategory::TenantEscalation).len(),
            1
        );
    }

    #[tokio::test]
    async fn disabled_assessor_returns_unavailable() {
        let assessor = UnsupportedSecurityAssessor::new(false);
        let matrix = ThreatControlMatrix::default();
        let result = assessor.assess(&matrix).await;
        assert_eq!(err_or_panic(result).kind, SecurityErrorKind::Unavailable);
    }

    #[tokio::test]
    async fn enabled_assessor_returns_unsupported() {
        let assessor = UnsupportedSecurityAssessor::new(true);
        let matrix = ThreatControlMatrix::default();
        let result = assessor.assess(&matrix).await;
        assert_eq!(err_or_panic(result).kind, SecurityErrorKind::Unsupported);
    }

    #[tokio::test]
    async fn mtls_disabled_returns_unavailable() {
        let assessor = UnsupportedSecurityAssessor::new(false);
        let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
        let result = assessor
            .mtls_identity_matches(
                foundation::NodeId::generate(&generator).expect("generate node id"),
                None,
            )
            .await;
        assert_eq!(err_or_panic(result).kind, SecurityErrorKind::Unavailable);
    }
}
