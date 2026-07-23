//! Contract suite that runs the same checks against fake and stub adapters.
//!
//! Phase 1 runs the suite against the in-memory `LocalMessageBus` and the
//! `NatsMessageBus`/`RestSignalingAdapter`/`JetStreamSignalingConsumer` stubs.
//! Real PostgreSQL/NATS-backed runs are left to the test environment.

use domain_signaling::{SignalingError, SignalingPort};
use foundation::{Deadline, DeviceId, MessageId, SystemClock, SystemIdGenerator, SystemRandom, TenantId, UtcTimestamp};
use message_api::{Envelope, MessageBus};
use signaling_adapter::jetstream::JetStreamSignalingConsumer;

/// Run message bus contract checks against any `MessageBus` implementation.
pub async fn run_message_bus_contract<B: MessageBus + Sync>(bus: &B) -> Result<(), String> {
    let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
    let id = MessageId::generate(&generator);
    let tenant = TenantId::generate(&generator);
    let envelope = Envelope::event(
        id,
        tenant,
        "security.v1.test".to_string(),
        vec![1, 2, 3],
    );
    let id = bus.publish(envelope).await.map_err(|e| e.to_string())?;
    if id.to_string().is_empty() {
        return Err("message id is empty".to_string());
    }
    Ok(())
}

/// Run signaling contract checks against any `SignalingPort` implementation.
pub async fn run_signaling_contract<S: SignalingPort + Sync>(
    adapter: &S,
) -> Result<(), String> {
    let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
    let tenant = TenantId::generate(&generator);
    let device = DeviceId::generate(&generator);
    let result = adapter
        .get_device(tenant, device, Deadline::new(UtcTimestamp::now()))
        .await;
    if result.is_ok() {
        return Err("signaling adapter should not return a live device in Phase 1".to_string());
    }
    Ok(())
}

/// Run JetStream projection consumer contract checks.
pub async fn run_jetstream_projection_contract(
    consumer: &JetStreamSignalingConsumer,
) -> Result<(), SignalingError> {
    consumer.start().await
}

#[cfg(test)]
mod tests {
    use domain_signaling::SignalingErrorKind;

    use super::*;
    use crate::fixtures::{jetstream_consumer, nats_bus_with_servers, BusFixture};

    fn ok_or_panic<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
        match result {
            Ok(v) => v,
            Err(e) => panic!("{e}"),
        }
    }

    fn err_or_panic<T, E: std::fmt::Display>(result: Result<T, E>) -> E {
        match result {
            Err(e) => e,
            Ok(_) => panic!("expected Err"),
        }
    }

    #[tokio::test]
    async fn local_bus_delivers_event() {
        let fixture = BusFixture::new();
        ok_or_panic(run_message_bus_contract(&fixture.local).await);
    }

    #[tokio::test]
    async fn nats_bus_stubs_return_expected_errors() {
        let unavailable = nats_bus_with_servers(None);
        let err = err_or_panic(run_message_bus_contract(&unavailable).await);
        assert!(err.contains("Unavailable"));

        let unsupported = nats_bus_with_servers(Some("nats://localhost:4222".to_string()));
        let err = err_or_panic(run_message_bus_contract(&unsupported).await);
        assert!(err.contains("Unsupported"));
    }

    #[tokio::test]
    async fn rest_signaling_unavailable_without_base_url() {
        let adapter = signaling_adapter::RestSignalingAdapter::new(None);
        let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
        let tenant = TenantId::generate(&generator);
        let device = DeviceId::generate(&generator);
        let result = adapter
            .get_device(tenant, device, Deadline::new(UtcTimestamp::now()))
            .await;
        let err = err_or_panic(result);
        assert_eq!(err.kind, SignalingErrorKind::Unavailable);
    }

    #[tokio::test]
    async fn jetstream_projection_unsupported() {
        let consumer = jetstream_consumer();
        let err = err_or_panic(run_jetstream_projection_contract(&consumer).await);
        assert_eq!(err.kind, SignalingErrorKind::Unsupported);
    }
}
