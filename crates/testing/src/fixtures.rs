//! Shared test fixtures for the security platform.
//!
//! Phase 1 provides in-memory fakes and configuration-driven stubs. Real
//! PostgreSQL/NATS containers are left to the test runner environment.

use foundation::{SystemClock, SystemIdGenerator, SystemRandom, TenantId, UserId};
use message_local::{LocalMessageBus, LocalMessageBusConfig};
use nats_adapter::{NatsMessageBus, NatsMessageBusConfig};
use signaling_adapter::{jetstream::JetStreamSignalingConsumer, RestSignalingAdapter};

/// A tenant/user pair with deterministic IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TenantFixture {
    pub tenant_id: TenantId,
    pub owner_user_id: UserId,
}

impl TenantFixture {
    pub fn generate() -> Self {
        let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
        Self {
            tenant_id: TenantId::generate(&generator),
            owner_user_id: UserId::generate(&generator),
        }
    }
}

/// In-memory message bus fixture.
pub struct BusFixture {
    pub local: LocalMessageBus,
}

impl Default for BusFixture {
    fn default() -> Self {
        Self::new()
    }
}

impl BusFixture {
    pub fn new() -> Self {
        Self {
            local: LocalMessageBus::new(LocalMessageBusConfig::default()),
        }
    }
}

/// NATS message bus fixture.
pub fn nats_bus_with_servers(servers: Option<String>) -> NatsMessageBus {
    let mut config = NatsMessageBusConfig::security_defaults();
    config.servers = servers;
    NatsMessageBus::new(config)
}

/// Signaling fixture with optional upstream base URL.
pub fn signaling_adapter(base_url: Option<String>) -> RestSignalingAdapter {
    RestSignalingAdapter::new(base_url)
}

/// JetStream consumer fixture.
pub fn jetstream_consumer() -> JetStreamSignalingConsumer {
    JetStreamSignalingConsumer::new(Default::default())
}
