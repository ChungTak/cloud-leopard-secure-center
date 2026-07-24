//! Message bus port traits and typed command/event envelope.
//!
//! `message-api` is runtime-neutral: it defines envelopes, errors, and the
//! `MessageBus` port. Concrete adapters (local/NATS) live in separate crates.

use std::collections::HashMap;

/// Return the crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

use foundation::{Clock, Deadline, MessageId, TenantId, UtcTimestamp};

const MAX_TOPIC_LEN: usize = 256;
const MAX_PAYLOAD_BYTES: usize = 1024 * 1024;
const MAX_HEADER_KEY_LEN: usize = 256;
const MAX_HEADER_VALUE_LEN: usize = 4096;
const MAX_HEADERS: usize = 64;
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
        topic: impl AsRef<str>,
        payload: impl AsRef<[u8]>,
        clock: &dyn Clock,
    ) -> Result<Self, MessageError> {
        let topic = topic.as_ref();
        validate_topic(topic)?;
        let payload = payload.as_ref();
        validate_payload(payload)?;
        Ok(Self {
            id,
            kind: EnvelopeKind::Command,
            tenant_id,
            topic: topic.to_string(),
            payload: payload.to_vec(),
            headers: HashMap::new(),
            timestamp: clock.now(),
            deadline: None,
        })
    }

    /// Create a new event envelope.
    pub fn event(
        id: MessageId,
        tenant_id: TenantId,
        topic: impl AsRef<str>,
        payload: impl AsRef<[u8]>,
        clock: &dyn Clock,
    ) -> Result<Self, MessageError> {
        let topic = topic.as_ref();
        validate_topic(topic)?;
        let payload = payload.as_ref();
        validate_payload(payload)?;
        Ok(Self {
            id,
            kind: EnvelopeKind::Event,
            tenant_id,
            topic: topic.to_string(),
            payload: payload.to_vec(),
            headers: HashMap::new(),
            timestamp: clock.now(),
            deadline: None,
        })
    }

    /// Set the optional processing deadline.
    pub fn with_deadline(mut self, deadline: Deadline) -> Self {
        self.deadline = Some(deadline);
        self
    }

    /// Set a header.
    pub fn with_header(
        mut self,
        key: impl AsRef<str>,
        value: impl AsRef<str>,
    ) -> Result<Self, MessageError> {
        let key = key.as_ref();
        let value = value.as_ref();
        if key.trim().is_empty() || key.len() > MAX_HEADER_KEY_LEN {
            return Err(MessageError::new(
                MessageErrorKind::Invalid,
                "message header key is empty or too long",
            ));
        }
        if value.len() > MAX_HEADER_VALUE_LEN {
            return Err(MessageError::new(
                MessageErrorKind::Invalid,
                "message header value is too long",
            ));
        }
        if self.headers.len() >= MAX_HEADERS && !self.headers.contains_key(key) {
            return Err(MessageError::new(
                MessageErrorKind::Invalid,
                "message has too many headers",
            ));
        }
        self.headers.insert(key.to_string(), value.to_string());
        Ok(self)
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
        topic: impl AsRef<str>,
        payload: &T,
        clock: &dyn Clock,
    ) -> Result<Self, MessageError> {
        let mut envelope = Envelope::command(id, tenant_id, topic, vec![], clock)?;
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
        topic: impl AsRef<str>,
        payload: &T,
        clock: &dyn Clock,
    ) -> Result<Self, MessageError> {
        let mut envelope = Envelope::event(id, tenant_id, topic, vec![], clock)?;
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

fn validate_topic(topic: &str) -> Result<(), MessageError> {
    if topic.trim().is_empty() {
        return Err(MessageError::new(
            MessageErrorKind::Invalid,
            "message topic must not be empty",
        ));
    }
    if topic.len() > MAX_TOPIC_LEN {
        return Err(MessageError::new(
            MessageErrorKind::Invalid,
            "message topic exceeds maximum length",
        ));
    }
    Ok(())
}

fn validate_payload(payload: &[u8]) -> Result<(), MessageError> {
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err(MessageError::new(
            MessageErrorKind::Invalid,
            "message payload exceeds maximum size",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use foundation::SystemClock;

    #[test]
    fn command_and_event_envelopes_have_distinct_kinds() {
        let generator =
            foundation::SystemIdGenerator::new(foundation::SystemClock, foundation::SystemRandom);
        let id = MessageId::generate(&generator).expect("generate message id");
        let tenant_id = TenantId::generate(&generator).expect("generate tenant id");

        let cmd = Envelope::command(
            id,
            tenant_id,
            "security.v1.command.0.test",
            b"{}",
            &SystemClock,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let evt = Envelope::event(
            id,
            tenant_id,
            "security.v1.event.0.test",
            b"{}",
            &SystemClock,
        )
        .unwrap_or_else(|e| panic!("{e}"));

        assert_eq!(cmd.kind, EnvelopeKind::Command);
        assert_eq!(evt.kind, EnvelopeKind::Event);
        assert_eq!(cmd.payload, b"{}");
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
        let envelope = match CommandEnvelope::new(id, tenant_id, "test.command", &cmd, &SystemClock)
        {
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
        let envelope =
            Envelope::command(id, tenant_id, "test", b"x", &SystemClock).expect("valid envelope");

        let mut runtime = futures::executor::LocalPool::new();
        match runtime.run_until(bus.publish(envelope)) {
            Ok(_) => panic!("expected unsupported"),
            Err(e) => assert_eq!(e.kind, MessageErrorKind::Unsupported),
        }
    }
}
