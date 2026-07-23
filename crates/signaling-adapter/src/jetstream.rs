//! JetStream signaling projection consumer.
//!
//! Phase 1 does not include a live NATS client. The consumer keeps the same
//! projection contract as the SSE/REST path (event → Inbox → Projection) but
//! short-circuits to `Unsupported` until `async-nats` is integrated.

use domain_signaling::{SignalingError, SignalingErrorKind};

/// Configuration for the JetStream signaling consumer.
#[derive(Debug, Clone, Default)]
pub struct JetStreamSignalingConsumerConfig {
    /// JetStream stream name for signaling events.
    pub stream_name: String,
    /// Consumer durable name.
    pub durable_name: String,
    /// Subject filter prefix, e.g. `sig.v1.event`.
    pub subject_prefix: String,
}

impl JetStreamSignalingConsumerConfig {
    /// Defaults for the cheetah-signaling JetStream integration.
    pub fn signaling_defaults() -> Self {
        Self {
            stream_name: "SIGNALING_EVENTS".to_string(),
            durable_name: "security-platform-projection".to_string(),
            subject_prefix: "sig.v1.event".to_string(),
        }
    }
}

/// Consumer that would project JetStream signaling events into the local shadow.
#[derive(Debug, Clone, Default)]
pub struct JetStreamSignalingConsumer {
    #[allow(dead_code)]
    config: JetStreamSignalingConsumerConfig,
}

impl JetStreamSignalingConsumer {
    /// Create a consumer from configuration.
    pub fn new(config: JetStreamSignalingConsumerConfig) -> Self {
        Self { config }
    }

    /// Start consuming and applying events to the projection.
    ///
    /// Phase 1: live JetStream consumption is not implemented.
    pub async fn start(&self) -> Result<(), SignalingError> {
        Err(SignalingError::new(
            SignalingErrorKind::Unsupported,
            "JetStream signaling projection consumer is not implemented in this build",
        ))
    }
}

#[cfg(test)]
mod tests {
    use domain_signaling::SignalingErrorKind;

    use super::*;

    #[test]
    fn start_returns_unsupported() {
        let consumer = JetStreamSignalingConsumer::new(
            JetStreamSignalingConsumerConfig::signaling_defaults(),
        );
        match futures::executor::block_on(consumer.start()) {
            Ok(_) => panic!("expected unsupported"),
            Err(e) => assert_eq!(e.kind, SignalingErrorKind::Unsupported),
        }
    }
}
