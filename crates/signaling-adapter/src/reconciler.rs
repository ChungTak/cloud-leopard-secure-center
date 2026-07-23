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

    /// Validate that the page size is within allowed bounds.
    pub fn validate(&self) -> Result<(), SignalingError> {
        if self.limit == 0 {
            return Err(SignalingError::new(
                SignalingErrorKind::Invalid,
                "reconciliation cursor limit must be greater than zero",
            ));
        }
        const MAX_LIMIT: usize = 10_000;
        if self.limit > MAX_LIMIT {
            return Err(SignalingError::new(
                SignalingErrorKind::Invalid,
                format!("reconciliation cursor limit exceeds {MAX_LIMIT}"),
            ));
        }
        Ok(())
    }
}

/// A buffered incremental event received during a rebuild.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InboundEvent {
    pub event_id: String,
    pub sequence: i64,
    pub payload: String,
}

/// Configuration for a full reconciliation run.
#[derive(Debug, Clone, Default)]
pub struct ReconciliationOptions {
    pub validate_before_switch: bool,
    pub missing_window_seconds: u32,
    pub bounded_cache_size: usize,
    pub cursor: ReconciliationCursor,
}

impl ReconciliationOptions {
    /// Validate bounds and return an error for invalid configuration.
    pub fn validate(&self) -> Result<(), SignalingError> {
        self.cursor.validate()?;
        if self.missing_window_seconds == 0 {
            return Err(SignalingError::new(
                SignalingErrorKind::Invalid,
                "missing retention window must be greater than zero",
            ));
        }
        if self.bounded_cache_size == 0 {
            return Err(SignalingError::new(
                SignalingErrorKind::Invalid,
                "bounded cache size must be greater than zero",
            ));
        }
        Ok(())
    }
}

/// Result of a full reconciliation run.
#[derive(Debug, Clone, Default)]
pub struct ReconciliationReport {
    pub switched: bool,
    pub missing_ids: Vec<String>,
    pub cached_events: Vec<InboundEvent>,
    pub next_cursor: ReconciliationCursor,
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

    /// Full reconciliation with validation, atomic switch, bounded event cache,
    /// and missing-device retention window.
    ///
    /// Phase 1: the upstream fetch is not implemented; validated options still
    /// return `Unsupported`.
    pub async fn reconcile_full(
        &self,
        options: &ReconciliationOptions,
    ) -> Result<ReconciliationReport, SignalingError> {
        options.validate()?;
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

    #[test]
    fn reconcile_full_valid_options_returns_unsupported() {
        let reconciler = SignalingReconciler::new();
        let options = ReconciliationOptions {
            validate_before_switch: true,
            missing_window_seconds: 3600,
            bounded_cache_size: 1000,
            cursor: ReconciliationCursor::initial(100),
        };
        match block_on(reconciler.reconcile_full(&options)) {
            Ok(_) => panic!("expected unsupported"),
            Err(e) => assert_eq!(e.kind, SignalingErrorKind::Unsupported),
        }
    }

    #[test]
    fn reconcile_full_rejects_invalid_cursor() {
        let reconciler = SignalingReconciler::new();
        let options = ReconciliationOptions {
            validate_before_switch: true,
            missing_window_seconds: 3600,
            bounded_cache_size: 1000,
            cursor: ReconciliationCursor::initial(0),
        };
        match block_on(reconciler.reconcile_full(&options)) {
            Ok(_) => panic!("expected invalid"),
            Err(e) => assert_eq!(e.kind, SignalingErrorKind::Invalid),
        }
    }

    #[test]
    fn reconcile_full_rejects_zero_cache() {
        let reconciler = SignalingReconciler::new();
        let options = ReconciliationOptions {
            validate_before_switch: true,
            missing_window_seconds: 3600,
            bounded_cache_size: 0,
            cursor: ReconciliationCursor::initial(100),
        };
        match block_on(reconciler.reconcile_full(&options)) {
            Ok(_) => panic!("expected invalid"),
            Err(e) => assert_eq!(e.kind, SignalingErrorKind::Invalid),
        }
    }
}
