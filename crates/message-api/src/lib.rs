//! Message bus port traits and typed command/event envelope.
//!
//! `message-api` is runtime-neutral: it defines envelopes, errors, and the
//! `MessageBus` port. Concrete adapters (local/NATS) live in separate crates.

use std::collections::HashMap;

/// Return the crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

use foundation::{Deadline, MessageId, TenantId, UtcTimestamp};
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};

/// Errors that can occur when publishing or consuming messages.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error("{kind:?}: {message}")]
pub struct MessageError {
    pub kind: MessageErrorKind,
    pub message: String,
}

impl MessageError {
    /// Create a new message bus error.
    pub fn new(kind: MessageErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

/// Taxonomy of message bus failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum MessageErrorKind {
    /// The capability is not implemented in this build.
    Unsupported,
    /// The transport is unreachable or unavailable.
    Unavailable,
    /// The bus is saturated and cannot accept the message.
    Backpressure,
    /// A message with this id was already published.
    Duplicate,
    /// The request deadline was exceeded.
    Timeout,
    /// The envelope was rejected as invalid.
    Invalid,
    /// The caller is not authorized.
    Unauthorized,
}

/// Classification of a message envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum EnvelopeKind {
    /// A command instructing a handler to perform an action.
    Command,
    /// An event describing something that already happened.
    Event,
    /// Unspecified/unknown kind; reserved for forward compatibility.
    Unspecified,
}

/// A message envelope on the bus.
///
/// The payload is opaque bytes; adapters are responsible for any codec
/// (JSON, protobuf, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Envelope {
    pub id: MessageId,
    pub kind: EnvelopeKind,
    pub tenant_id: TenantId,
    pub topic: String,
    pub payload: Vec<u8>,
    pub headers: HashMap<String, String>,
    pub timestamp: UtcTimestamp,
    pub deadline: Option<Deadline>,
}

impl Envelope {
    /// Create a new command envelope.
    pub fn command(
        id: MessageId,
        tenant_id: TenantId,
        topic: impl Into<String>,
        payload: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            id,
            kind: EnvelopeKind::Command,
            tenant_id,
            topic: topic.into(),
            payload: payload.into(),
            headers: HashMap::new(),
            timestamp: UtcTimestamp::now(),
            deadline: None,
        }
    }

    /// Create a new event envelope.
    pub fn event(
        id: MessageId,
        tenant_id: TenantId,
        topic: impl Into<String>,
        payload: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            id,
            kind: EnvelopeKind::Event,
            tenant_id,
            topic: topic.into(),
            payload: payload.into(),
            headers: HashMap::new(),
            timestamp: UtcTimestamp::now(),
            deadline: None,
        }
    }

    /// Set the optional processing deadline.
    pub fn with_deadline(mut self, deadline: Deadline) -> Self {
        self.deadline = Some(deadline);
        self
    }

    /// Set a header.
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Decode the payload as JSON.
    pub fn json_payload<T: for<'de> Deserialize<'de>>(&self) -> Result<T, MessageError> {
        serde_json::from_slice(&self.payload).map_err(|e| {
            MessageError::new(
                MessageErrorKind::Invalid,
                format!("failed to decode payload as JSON: {e}"),
            )
        })
    }

    /// Encode a JSON value into the payload.
    pub fn set_json_payload<T: Serialize>(&mut self, value: &T) -> Result<(), MessageError> {
        self.payload = serde_json::to_vec(value).map_err(|e| {
            MessageError::new(
                MessageErrorKind::Invalid,
                format!("failed to encode payload as JSON: {e}"),
            )
        })?;
        Ok(())
    }
}

/// Typed command envelope wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandEnvelope(pub Envelope);

impl CommandEnvelope {
    /// Create a command envelope from a serializable payload.
    pub fn new<T: Serialize>(
        id: MessageId,
        tenant_id: TenantId,
        topic: impl Into<String>,
        payload: &T,
    ) -> Result<Self, MessageError> {
        let mut envelope = Envelope::command(id, tenant_id, topic, vec![]);
        envelope.set_json_payload(payload)?;
        Ok(Self(envelope))
    }
}

/// Typed event envelope wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventEnvelope(pub Envelope);

impl EventEnvelope {
    /// Create an event envelope from a serializable payload.
    pub fn new<T: Serialize>(
        id: MessageId,
        tenant_id: TenantId,
        topic: impl Into<String>,
        payload: &T,
    ) -> Result<Self, MessageError> {
        let mut envelope = Envelope::event(id, tenant_id, topic, vec![]);
        envelope.set_json_payload(payload)?;
        Ok(Self(envelope))
    }
}

/// Port abstraction for a publish/subscribe message bus.
#[async_trait::async_trait]
pub trait MessageBus: Send + Sync {
    /// Publish an envelope to the bus.
    async fn publish(&self, envelope: Envelope) -> Result<MessageId, MessageError>;

    /// Subscribe to a topic filter and receive a stream of envelopes.
    ///
    /// The stream ends if the bus is shut down or the subscriber is cancelled.
    async fn subscribe(
        &self,
        topic_filter: &str,
    ) -> Result<BoxStream<'static, Envelope>, MessageError>;

    /// Acknowledge a delivered message so it will not be redelivered.
    async fn ack(&self, message_id: MessageId) -> Result<(), MessageError>;

    /// Negative-acknowledge a message, requesting redelivery.
    async fn nack(&self, message_id: MessageId) -> Result<(), MessageError>;
}

/// A stub bus that always returns `Unsupported`.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnsupportedMessageBus;

#[async_trait::async_trait]
impl MessageBus for UnsupportedMessageBus {
    async fn publish(&self, _envelope: Envelope) -> Result<MessageId, MessageError> {
        Err(MessageError::new(
            MessageErrorKind::Unsupported,
            "message bus is not enabled in this build",
        ))
    }

    async fn subscribe(
        &self,
        _topic_filter: &str,
    ) -> Result<BoxStream<'static, Envelope>, MessageError> {
        Err(MessageError::new(
            MessageErrorKind::Unsupported,
            "message bus subscription is not enabled in this build",
        ))
    }

    async fn ack(&self, _message_id: MessageId) -> Result<(), MessageError> {
        Err(MessageError::new(
            MessageErrorKind::Unsupported,
            "message bus ack is not enabled in this build",
        ))
    }

    async fn nack(&self, _message_id: MessageId) -> Result<(), MessageError> {
        Err(MessageError::new(
            MessageErrorKind::Unsupported,
            "message bus nack is not enabled in this build",
        ))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn command_and_event_envelopes_have_distinct_kinds() {
        let generator =
            foundation::SystemIdGenerator::new(foundation::SystemClock, foundation::SystemRandom);
        let id = MessageId::generate(&generator).expect("generate message id");
        let tenant_id = TenantId::generate(&generator).expect("generate tenant id");

        let cmd = Envelope::command(id, tenant_id, "security.v1.command.0.test", b"{}".to_vec());
        let evt = Envelope::event(id, tenant_id, "security.v1.event.0.test", b"{}".to_vec());

        assert_eq!(cmd.kind, EnvelopeKind::Command);
        assert_eq!(evt.kind, EnvelopeKind::Event);
        assert_eq!(cmd.payload, b"{}".to_vec());
    }

    #[test]
    fn typed_command_envelope_round_trips_json_payload() {
        let generator =
            foundation::SystemIdGenerator::new(foundation::SystemClock, foundation::SystemRandom);
        let id = MessageId::generate(&generator).expect("generate message id");
        let tenant_id = TenantId::generate(&generator).expect("generate tenant id");

        #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        struct TestCommand {
            value: u64,
        }

        let cmd = TestCommand { value: 42 };
        let envelope = match CommandEnvelope::new(id, tenant_id, "test.command", &cmd) {
            Ok(e) => e,
            Err(e) => panic!("{e}"),
        };
        let decoded: TestCommand = match envelope.0.json_payload() {
            Ok(v) => v,
            Err(e) => panic!("{e}"),
        };
        assert_eq!(decoded, cmd);
    }

    #[test]
    fn unsupported_bus_returns_unsupported() {
        let bus = UnsupportedMessageBus;
        let generator =
            foundation::SystemIdGenerator::new(foundation::SystemClock, foundation::SystemRandom);
        let id = MessageId::generate(&generator).expect("generate message id");
        let tenant_id = TenantId::generate(&generator).expect("generate tenant id");
        let envelope = Envelope::command(id, tenant_id, "test", b"x".to_vec());

        let mut runtime = futures::executor::LocalPool::new();
        match runtime.run_until(bus.publish(envelope)) {
            Ok(_) => panic!("expected unsupported"),
            Err(e) => assert_eq!(e.kind, MessageErrorKind::Unsupported),
        }
    }
}
