//! In-memory message bus adapter.
//!
//! `LocalMessageBus` provides at-least-once delivery semantics, ack/nack, and
//! bounded backpressure for local testing and development. It is not
//! persistent: shutdown loses in-flight messages.

use std::collections::HashMap;

/// Return the crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Return the `message-api` version used by this adapter.
pub fn message_api_version() -> &'static str {
    message_api::version()
}
use std::sync::Arc;

use async_trait::async_trait;
use foundation::MessageId;
use futures::stream::{BoxStream, StreamExt};
use message_api::{Envelope, MessageBus, MessageError, MessageErrorKind};
use tokio::sync::{Mutex, Notify, RwLock};

/// In-flight metadata for an unacknowledged message.
#[derive(Debug, Clone)]
struct InFlight {
    envelope: Envelope,
    nack_count: u32,
}

/// Configuration for the local message bus.
#[derive(Debug, Clone, Copy)]
pub struct LocalMessageBusConfig {
    /// Maximum number of messages buffered in the broadcast channel.
    pub broadcast_capacity: usize,
    /// Maximum number of unacknowledged messages kept in memory.
    pub max_in_flight: usize,
    /// Maximum number of negative acknowledgements before a message is dead-lettered.
    pub max_nack_count: u32,
}

impl Default for LocalMessageBusConfig {
    fn default() -> Self {
        Self {
            broadcast_capacity: 256,
            max_in_flight: 4096,
            max_nack_count: 3,
        }
    }
}

/// In-memory, non-persistent message bus.
#[derive(Debug)]
pub struct LocalMessageBus {
    config: LocalMessageBusConfig,
    /// Wake subscribers when a message is (re-)published.
    notify: Notify,
    /// Broadcast channel for live delivery.
    sender: tokio::sync::broadcast::Sender<Envelope>,
    /// Unacknowledged messages indexed by message id.
    in_flight: Arc<RwLock<HashMap<MessageId, InFlight>>>,
    /// Total in-flight count to enforce backpressure.
    in_flight_count: Arc<Mutex<usize>>,
}

impl LocalMessageBus {
    /// Create a new local bus with the supplied configuration.
    pub fn new(config: LocalMessageBusConfig) -> Self {
        let (sender, _) = tokio::sync::broadcast::channel(config.broadcast_capacity);
        Self {
            config,
            notify: Notify::new(),
            sender,
            in_flight: Arc::new(RwLock::new(HashMap::new())),
            in_flight_count: Arc::new(Mutex::new(0)),
        }
    }

    /// Create a local bus with default configuration.
    pub fn default_bus() -> Self {
        Self::new(LocalMessageBusConfig::default())
    }

    fn topic_matches(topic: &str, filter: &str) -> bool {
        // Simple wildcard: '*' matches one segment, or all remaining segments if it
        // is the last part; '**' matches zero or more remaining segments. Adapted for
        // local testing; real adapters use the upstream subject model.
        if filter == "*" || filter == "**" {
            return true;
        }
        let filter_parts: Vec<&str> = filter.split('.').collect();
        let topic_parts: Vec<&str> = topic.split('.').collect();
        Self::topic_matches_inner(&filter_parts, &topic_parts, 0, 0)
    }

    fn topic_matches_inner(
        filter: &[&str],
        topic: &[&str],
        fi: usize,
        ti: usize,
    ) -> bool {
        if fi == filter.len() {
            return ti == topic.len();
        }
        match filter[fi] {
            "**" => {
                // '**' matches zero or more topic segments, including all remaining
                // segments when it is the last filter part.
                for k in ti..=topic.len() {
                    if Self::topic_matches_inner(filter, topic, fi + 1, k) {
                        return true;
                    }
                }
                false
            }
            "*" => {
                if fi == filter.len() - 1 {
                    // Trailing '*' matches zero or more remaining segments.
                    return true;
                }
                if ti == topic.len() {
                    return false;
                }
                Self::topic_matches_inner(filter, topic, fi + 1, ti + 1)
            }
            part if ti < topic.len() && part == topic[ti] => {
                Self::topic_matches_inner(filter, topic, fi + 1, ti + 1)
            }
            _ => false,
        }
    }

    async fn redeliver_unacked(&self, id: MessageId) -> Result<(), MessageError> {
        let envelope = {
            let read = self.in_flight.read().await;
            read.get(&id).map(|f| f.envelope.clone())
        };
        let Some(envelope) = envelope else {
            return Err(MessageError::new(
                MessageErrorKind::Invalid,
                "message id is not in flight",
            ));
        };
        // Re-broadcast so all subscribers can see it again.
        let _ = self.sender.send(envelope);
        self.notify.notify_waiters();
        Ok(())
    }

    /// Manually trigger redelivery of all unacknowledged messages.
    /// Used by tests; real adapters handle redelivery internally.
    pub async fn flush_redeliveries(&self) {
        let ids: Vec<MessageId> = {
            let read = self.in_flight.read().await;
            read.keys().copied().collect()
        };
        for id in ids {
            let _ = self.redeliver_unacked(id).await;
        }
    }
}

#[async_trait]
impl MessageBus for LocalMessageBus {
    async fn publish(&self, envelope: Envelope) -> Result<MessageId, MessageError> {
        let deadline_exceeded = envelope
            .deadline
            .is_some_and(|d| d.is_expired(&foundation::SystemClock));
        if deadline_exceeded {
            return Err(MessageError::new(
                MessageErrorKind::Timeout,
                "message deadline already exceeded",
            ));
        }

        {
            let mut count = self.in_flight_count.lock().await;
            if *count >= self.config.max_in_flight {
                return Err(MessageError::new(
                    MessageErrorKind::Backpressure,
                    "local bus has reached max in-flight messages",
                ));
            }
            *count += 1;
        }

        {
            let mut flights = self.in_flight.write().await;
            if flights.contains_key(&envelope.id) {
                // Restore the count we just incremented.
                let mut count = self.in_flight_count.lock().await;
                *count -= 1;
                return Err(MessageError::new(
                    MessageErrorKind::Duplicate,
                    "message id already published",
                ));
            }
            flights.insert(
                envelope.id,
                InFlight {
                    envelope: envelope.clone(),
                    nack_count: 0,
                },
            );
        }

        // Best-effort broadcast. If there are no active subscribers, the local
        // bus still keeps the message in-flight so it can be redelivered later.
        let _ = self.sender.send(envelope.clone());
        self.notify.notify_waiters();
        Ok(envelope.id)
    }

    async fn subscribe(
        &self,
        topic_filter: &str,
    ) -> Result<BoxStream<'static, Envelope>, MessageError> {
        let receiver = self.sender.subscribe();
        let filter = topic_filter.to_owned();
        let stream = futures::stream::unfold(receiver, move |mut rx| {
            let filter = filter.clone();
            async move {
                loop {
                    match rx.recv().await {
                        Ok(envelope) if Self::topic_matches(&envelope.topic, &filter) => {
                            return Some((envelope, rx));
                        }
                        Ok(_) => continue,
                        Err(_) => return None,
                    }
                }
            }
        });
        Ok(stream.boxed())
    }

    async fn ack(&self, message_id: MessageId) -> Result<(), MessageError> {
        let removed = {
            let mut flights = self.in_flight.write().await;
            flights.remove(&message_id).is_some()
        };
        if removed {
            let mut count = self.in_flight_count.lock().await;
            *count -= 1;
            Ok(())
        } else {
            Err(MessageError::new(
                MessageErrorKind::Invalid,
                "message id is not in flight",
            ))
        }
    }

    async fn nack(&self, message_id: MessageId) -> Result<(), MessageError> {
        let should_redeliver = {
            let mut flights = self.in_flight.write().await;
            match flights.get_mut(&message_id) {
                Some(f) if f.nack_count < self.config.max_nack_count => {
                    f.nack_count += 1;
                    true
                }
                Some(_) => {
                    // Max nack reached: dead-letter by removing from in-flight.
                    flights.remove(&message_id);
                    let mut count = self.in_flight_count.lock().await;
                    *count -= 1;
                    false
                }
                None => {
                    return Err(MessageError::new(
                        MessageErrorKind::Invalid,
                        "message id is not in flight",
                    ));
                }
            }
        };

        if should_redeliver {
            self.redeliver_unacked(message_id).await
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use foundation::{SystemClock, SystemIdGenerator, SystemRandom, TenantId, UtcTimestamp};
    use futures::StreamExt;
    use message_api::EnvelopeKind;

    fn ok_or_panic<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
        match result {
            Ok(v) => v,
            Err(e) => panic!("{e}"),
        }
    }

    fn some_or_panic<T>(option: Option<T>) -> T {
        match option {
            Some(v) => v,
            None => panic!("expected Some, got None"),
        }
    }

    #[test]
    fn topic_matches_literal_segment() {
        assert!(LocalMessageBus::topic_matches("security.v1.event.0", "security.v1.event.0"));
        assert!(!LocalMessageBus::topic_matches("security.v1.event.0", "security.v1.event.1"));
    }

    #[test]
    fn topic_matches_single_wildcard() {
        assert!(LocalMessageBus::topic_matches("security.v1.event.0", "security.v1.event.*"));
        // Trailing '*' matches zero or more remaining segments.
        assert!(LocalMessageBus::topic_matches(
            "security.v1.event.0.test",
            "security.v1.event.*"
        ));
        assert!(!LocalMessageBus::topic_matches(
            "security.v1.command.0",
            "security.v1.event.*"
        ));
    }

    #[test]
    fn topic_matches_trailing_star() {
        assert!(LocalMessageBus::topic_matches("security.v1.event.0", "security.v1.*"));
        assert!(LocalMessageBus::topic_matches(
            "security.v1.event.0.test",
            "security.v1.*"
        ));
    }

    #[test]
    fn topic_matches_double_wildcard_infix() {
        assert!(LocalMessageBus::topic_matches(
            "security.v1.event.0",
            "security.**.event.0"
        ));
        assert!(!LocalMessageBus::topic_matches(
            "security.v1.command.0",
            "security.**.event.0"
        ));
    }

    #[test]
    fn topic_matches_double_wildcard_suffix() {
        assert!(LocalMessageBus::topic_matches(
            "security.v1.event.0",
            "security.**.0"
        ));
    }

    #[test]
    fn topic_mismatch_on_empty_remaining_filter() {
        assert!(!LocalMessageBus::topic_matches("security.v1", "security.v1.event"));
    }

    fn make_envelope(kind: EnvelopeKind, topic: &str, payload: &[u8]) -> Envelope {
        let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
        Envelope {
            id: MessageId::generate(&generator).expect("generate message id"),
            kind,
            tenant_id: TenantId::generate(&generator).expect("generate tenant id"),
            topic: topic.to_string(),
            payload: payload.to_vec(),
            headers: HashMap::new(),
            timestamp: UtcTimestamp::now(),
            deadline: None,
        }
    }

    #[tokio::test]
    async fn local_bus_publishes_and_subscribes() {
        let bus = LocalMessageBus::default_bus();
        let mut stream = ok_or_panic(bus.subscribe("security.v1.event.*").await);

        let envelope = make_envelope(EnvelopeKind::Event, "security.v1.event.0.test", b"hello");
        let id = ok_or_panic(bus.publish(envelope.clone()).await);
        assert_eq!(id, envelope.id);

        let received = some_or_panic(stream.next().await);
        assert_eq!(received.id, envelope.id);
        assert_eq!(received.payload, b"hello".to_vec());
    }

    #[tokio::test]
    async fn unacked_message_can_be_redelivered() {
        let bus = LocalMessageBus::default_bus();
        let mut stream = ok_or_panic(bus.subscribe("security.v1.command.*").await);

        let envelope = make_envelope(EnvelopeKind::Command, "security.v1.command.0.test", b"do");
        ok_or_panic(bus.publish(envelope.clone()).await);

        let first = some_or_panic(stream.next().await);
        assert_eq!(first.id, envelope.id);

        ok_or_panic(bus.nack(envelope.id).await);
        let second = some_or_panic(stream.next().await);
        assert_eq!(second.id, envelope.id);

        ok_or_panic(bus.ack(envelope.id).await);
        bus.flush_redeliveries().await;

        // After ack there should be no more redeliveries; the stream eventually closes
        // only on bus shutdown, so just confirm the in-flight map is empty.
        let count = bus.in_flight_count.lock().await;
        assert_eq!(*count, 0);
    }

    #[tokio::test]
    async fn max_nack_dead_letters_message() {
        let config = LocalMessageBusConfig {
            max_nack_count: 2,
            ..Default::default()
        };
        let bus = LocalMessageBus::new(config);
        let envelope = make_envelope(EnvelopeKind::Command, "security.v1.command.0.test", b"x");
        ok_or_panic(bus.publish(envelope.clone()).await);

        for _ in 0..2 {
            ok_or_panic(bus.nack(envelope.id).await);
        }
        // Third nack exceeds the limit and dead-letters the message.
        ok_or_panic(bus.nack(envelope.id).await);

        let count = bus.in_flight_count.lock().await;
        assert_eq!(*count, 0);
    }

    #[tokio::test]
    async fn duplicate_publish_is_rejected() {
        let bus = LocalMessageBus::default_bus();
        let envelope = make_envelope(EnvelopeKind::Event, "test", b"");
        ok_or_panic(bus.publish(envelope.clone()).await);
        let result = bus.publish(envelope.clone()).await;
        match result {
            Ok(_) => panic!("expected duplicate"),
            Err(e) => assert_eq!(e.kind, MessageErrorKind::Duplicate),
        }
    }

    #[tokio::test]
    async fn backpressure_when_max_in_flight_reached() {
        // With max_in_flight set to 1, a second unacked publish is rejected.
        let config = LocalMessageBusConfig {
            max_in_flight: 1,
            ..Default::default()
        };
        let bus = LocalMessageBus::new(config);

        let e1 = make_envelope(EnvelopeKind::Event, "test", b"a");
        let e2 = make_envelope(EnvelopeKind::Event, "test", b"b");

        ok_or_panic(bus.publish(e1).await);
        let result = bus.publish(e2).await;
        match result {
            Ok(_) => panic!("expected backpressure"),
            Err(e) => assert_eq!(e.kind, MessageErrorKind::Backpressure),
        }
    }
}
