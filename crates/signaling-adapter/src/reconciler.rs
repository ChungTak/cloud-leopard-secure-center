//! Full reconciliation from upstream signaling to the shadow projection.
//!
//! Phase 1 leaves the upstream fetch unimplemented; the reconciler explicitly
//! returns `Unsupported` so callers do not treat the projection as complete.

use domain_signaling::{SignalingError, SignalingErrorKind};

/// Reconciliation cursor for paginated upstream fetches.
#[derive(Debug, Clone, Default)]
pub struct ReconciliationCursor {
    pub value: Option<String>,
    pub limit: usize,
}

impl ReconciliationCursor {
    /// Create an initial cursor with the requested page size.
    pub fn initial(limit: usize) -> Self {
        Self { value: None, limit }
    }
}

/// Reconciler that rebuilds the shadow projection from an upstream signaling system.
#[derive(Debug, Clone, Default)]
pub struct SignalingReconciler;

impl SignalingReconciler {
    /// Create a new reconciler.
    pub fn new() -> Self {
        Self
    }

    /// Reconcile the full device/channel set into the shadow projection.
    ///
    /// Phase 1: full upstream reconciliation is not implemented.
    pub async fn reconcile(&self) -> Result<ReconciliationCursor, SignalingError> {
        Err(SignalingError::new(
            SignalingErrorKind::Unsupported,
            "full signaling reconciliation is not implemented in this build",
        ))
    }
}

#[cfg(test)]
mod tests {
    use futures::executor::block_on;

    use super::*;
    use domain_signaling::SignalingErrorKind;

    #[test]
    fn reconcile_returns_unsupported() {
        let reconciler = SignalingReconciler::new();
        match block_on(reconciler.reconcile()) {
            Ok(_) => panic!("expected unsupported"),
            Err(e) => assert_eq!(e.kind, SignalingErrorKind::Unsupported),
        }
    }
}
