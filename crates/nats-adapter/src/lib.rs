//! NATS Core / JetStream message bus adapter.
//!
//! Phase 1 does not include a live NATS client. When a connection URL is supplied
//! the adapter returns `Unsupported`; when it is absent the adapter returns
//! `Unavailable`. This preserves the `MessageBus` port contract and gives a
//! single place to plug in `async-nats` later.

use async_trait::async_trait;
use foundation::MessageId;
use futures::stream::BoxStream;
use message_api::{Envelope, MessageBus, MessageError, MessageErrorKind};

/// Configuration for the NATS message bus.
#[derive(Debug, Clone, Default)]
pub struct NatsMessageBusConfig {
    /// Comma-separated NATS server URLs, e.g. `nats://localhost:4222`.
    pub servers: Option<String>,
    /// JetStream stream name for events.
    pub events_stream: String,
    /// JetStream stream name for commands.
    pub commands_stream: String,
    /// Consumer durable name.
    pub durable_name: String,
    /// Max delivery attempts before dead-lettering.
    pub max_deliver: u32,
}

impl NatsMessageBusConfig {
    /// Sensible defaults for the security platform subject namespace.
    pub fn security_defaults() -> Self {
        Self {
            servers: None,
            events_stream: "SECURITY_EVENTS".to_string(),
            commands_stream: "SECURITY_COMMANDS".to_string(),
            durable_name: "security-platform".to_string(),
            max_deliver: 3,
        }
    }
}

/// NATS message bus adapter.
#[derive(Debug, Clone, Default)]
pub struct NatsMessageBus {
    config: NatsMessageBusConfig,
}

impl NatsMessageBus {
    /// Create a new adapter from configuration.
    pub fn new(config: NatsMessageBusConfig) -> Self {
        Self { config }
    }

    fn unsupported() -> MessageError {
        MessageError::new(
            MessageErrorKind::Unsupported,
            "NATS transport is not implemented in this build",
        )
    }

    fn unavailable() -> MessageError {
        MessageError::new(
            MessageErrorKind::Unavailable,
            "NATS servers are not configured",
        )
    }

    fn maybe_error(&self) -> Result<(), MessageError> {
        if self.config.servers.is_some() {
            Err(Self::unsupported())
        } else {
            Err(Self::unavailable())
        }
    }
}

#[async_trait]
impl MessageBus for NatsMessageBus {
    async fn publish(&self, _envelope: Envelope) -> Result<MessageId, MessageError> {
        self.maybe_error()?;
        unreachable!("error always returned above")
    }

    async fn subscribe(&self, _topic_filter: &str) -> Result<BoxStream<'static, Envelope>, MessageError> {
        self.maybe_error()?;
        unreachable!("error always returned above")
    }

    async fn ack(&self, _message_id: MessageId) -> Result<(), MessageError> {
        self.maybe_error()?;
        unreachable!("error always returned above")
    }

    async fn nack(&self, _message_id: MessageId) -> Result<(), MessageError> {
        self.maybe_error()?;
        unreachable!("error always returned above")
    }
}

#[cfg(test)]
mod tests {
    use foundation::{SystemClock, SystemIdGenerator, SystemRandom, TenantId};
    use message_api::Envelope;

    use super::*;

    #[tokio::test]
    async fn unconfigured_nats_returns_unavailable() {
        let bus = NatsMessageBus::new(NatsMessageBusConfig::security_defaults());
        let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
        let envelope = Envelope::command(
            MessageId::generate(&generator),
            TenantId::generate(&generator),
            "security.v1.command.0.test",
            b"{}".to_vec(),
        );
        match bus.publish(envelope).await {
            Ok(_) => panic!("expected unavailable"),
            Err(e) => assert_eq!(e.kind, MessageErrorKind::Unavailable),
        }
    }

    #[tokio::test]
    async fn configured_nats_returns_unsupported() {
        let mut config = NatsMessageBusConfig::security_defaults();
        config.servers = Some("nats://localhost:4222".to_string());
        let bus = NatsMessageBus::new(config);
        let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
        let envelope = Envelope::command(
            MessageId::generate(&generator),
            TenantId::generate(&generator),
            "security.v1.command.0.test",
            b"{}".to_vec(),
        );
        match bus.publish(envelope).await {
            Ok(_) => panic!("expected unsupported"),
            Err(e) => assert_eq!(e.kind, MessageErrorKind::Unsupported),
        }
    }
}
