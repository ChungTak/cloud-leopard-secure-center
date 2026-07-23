//! SSE event worker: receives signaling events, deduplicates via the inbox, and
//! updates the projection read model.

use domain_resource::projection::{ChannelEvent, DeviceEvent};
use domain_signaling::{SignalingError, SignalingErrorKind};
use foundation::{
    Clock, Deadline, RequestContext, SystemClock, SystemRandom, UtcTimestamp, chrono,
};
use futures::{Stream, StreamExt};
use storage_api::{InboxMessage, InboxRepository, InboxStatus, ProjectionRepository};

use crate::event::{SignalingEvent, SignalingEventPayload};

/// Processes a single signaling event into the inbox and projection repositories.
#[derive(Debug, Clone, Default)]
pub struct SignalingEventProcessor;

impl SignalingEventProcessor {
    /// Process an event: deduplicate via inbox, then apply the projection update.
    pub async fn process<P, I>(
        &self,
        event: &SignalingEvent,
        projection: &P,
        inbox: &I,
        ctx: &RequestContext,
    ) -> Result<(), SignalingError>
    where
        P: ProjectionRepository,
        I: InboxRepository,
    {
        let message_id = parse_or_generate_event_id(&event.last_event_id);
        let consumer_id = "signaling-sse".to_string();
        let received = inbox
            .receive(
                &InboxMessage {
                    message_id,
                    tenant_id: Some(event.tenant_id),
                    consumer_id,
                    status: InboxStatus::Pending,
                    result_digest: None,
                    attempts: 0,
                    expires_at: UtcTimestamp::from(chrono::Utc::now() + chrono::Duration::hours(1)),
                },
                ctx,
            )
            .await
            .map_err(map_storage_error)?;

        if received.status == InboxStatus::Completed {
            return Ok(());
        }

        let payload_json = serde_json::to_string(&event.payload).map_err(|e| {
            SignalingError::new(
                SignalingErrorKind::Invalid,
                format!("failed to serialize event payload: {e}"),
            )
        })?;

        match &event.payload {
            SignalingEventPayload::DeviceOnline
            | SignalingEventPayload::DeviceOffline
            | SignalingEventPayload::Gap => {
                projection
                    .apply_device_event(
                        DeviceEvent {
                            external_ref: event.device_id.to_string(),
                            sequence: 0,
                            source_event_id: event.last_event_id.clone(),
                            observed_at: event.observed_at,
                            payload: payload_json.clone(),
                        },
                        ctx,
                    )
                    .await
                    .map_err(map_storage_error)?;
            }
            SignalingEventPayload::ChannelState { channel_id, .. } => {
                projection
                    .apply_channel_event(
                        ChannelEvent {
                            external_ref: format!("{}:{}", event.device_id, channel_id),
                            sequence: 0,
                            source_event_id: event.last_event_id.clone(),
                            observed_at: event.observed_at,
                            payload: payload_json.clone(),
                        },
                        ctx,
                    )
                    .await
                    .map_err(map_storage_error)?;
            }
        }

        inbox
            .complete("signaling-sse", message_id, &payload_json, ctx)
            .await
            .map_err(map_storage_error)?;

        Ok(())
    }
}

/// Worker that consumes an SSE event stream and drives the projection.
pub struct SignalingEventWorker {
    processor: SignalingEventProcessor,
    clock: Box<dyn Clock>,
}

impl SignalingEventWorker {
    /// Create a worker with the system clock.
    pub fn new() -> Self {
        Self {
            processor: SignalingEventProcessor,
            clock: Box::new(SystemClock),
        }
    }

    /// Run the worker until the stream ends or the deadline expires.
    pub async fn run<P, I, S>(
        &self,
        mut stream: S,
        projection: &P,
        inbox: &I,
        ctx: &RequestContext,
        deadline: Deadline,
    ) -> Result<(), SignalingError>
    where
        P: ProjectionRepository,
        I: InboxRepository,
        S: Stream<Item = SignalingEvent> + Unpin,
    {
        while let Some(event) = stream.next().await {
            if deadline.is_expired(&*self.clock) {
                return Err(SignalingError::new(
                    SignalingErrorKind::Timeout,
                    "signaling event worker deadline expired",
                ));
            }

            if matches!(event.payload, SignalingEventPayload::Gap) {
                // Gaps make the projection stale; continue so the caller can reconcile.
            }

            self.processor
                .process(&event, projection, inbox, ctx)
                .await?;
        }

        Ok(())
    }
}

impl Default for SignalingEventWorker {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_or_generate_event_id(input: &str) -> foundation::uuid::Uuid {
    foundation::uuid::Uuid::parse_str(input).unwrap_or_else(|_| {
        let generator = foundation::SystemIdGenerator::new(SystemClock, SystemRandom);
        foundation::MessageId::generate(&generator).into()
    })
}

fn map_storage_error(e: foundation::PlatformError) -> SignalingError {
    SignalingError::new(
        SignalingErrorKind::UnknownOutcome,
        format!("projection/inbox storage error: {e}"),
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, MutexGuard};

    use futures::executor::block_on;

    use super::*;
    use domain_resource::projection::{ChannelProjection, DeviceProjection, ProjectionFailure};
    use foundation::{
        DeviceId, SystemClock, SystemIdGenerator, SystemRandom, TenantId, uuid::Uuid,
    };

    fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
        match m.lock() {
            Ok(g) => g,
            Err(_) => panic!("mutex poisoned"),
        }
    }

    struct FakeProjection {
        devices: Arc<Mutex<Vec<DeviceProjection>>>,
        channels: Arc<Mutex<Vec<ChannelProjection>>>,
        failures: Arc<Mutex<Vec<ProjectionFailure>>>,
    }

    #[async_trait::async_trait]
    impl ProjectionRepository for FakeProjection {
        async fn apply_device_event(
            &self,
            event: DeviceEvent,
            _ctx: &RequestContext,
        ) -> Result<(), foundation::PlatformError> {
            lock(&self.devices).push(DeviceProjection {
                external_ref: event.external_ref,
                sequence: event.sequence,
                source_event_id: event.source_event_id,
                observed_at: event.observed_at,
                payload: event.payload,
                stale: false,
            });
            Ok(())
        }

        async fn get_device(
            &self,
            _external_ref: &str,
            _ctx: &RequestContext,
        ) -> Result<DeviceProjection, foundation::PlatformError> {
            unimplemented!()
        }

        async fn apply_channel_event(
            &self,
            event: ChannelEvent,
            _ctx: &RequestContext,
        ) -> Result<(), foundation::PlatformError> {
            lock(&self.channels).push(ChannelProjection {
                external_ref: event.external_ref,
                sequence: event.sequence,
                source_event_id: event.source_event_id,
                observed_at: event.observed_at,
                payload: event.payload,
                stale: false,
            });
            Ok(())
        }

        async fn get_channel(
            &self,
            _external_ref: &str,
            _ctx: &RequestContext,
        ) -> Result<ChannelProjection, foundation::PlatformError> {
            unimplemented!()
        }

        async fn rebuild_shadow(
            &self,
            _device_events: Vec<DeviceEvent>,
            _channel_events: Vec<ChannelEvent>,
            _ctx: &RequestContext,
        ) -> Result<(), foundation::PlatformError> {
            Ok(())
        }

        async fn checkpoint(
            &self,
            _worker_id: &str,
            _last_event_id: &str,
            _observed_at: UtcTimestamp,
            _ctx: &RequestContext,
        ) -> Result<(), foundation::PlatformError> {
            Ok(())
        }

        async fn record_failure(
            &self,
            failure: ProjectionFailure,
            _ctx: &RequestContext,
        ) -> Result<(), foundation::PlatformError> {
            lock(&self.failures).push(failure);
            Ok(())
        }
    }

    struct FakeInbox {
        messages: Arc<Mutex<HashMap<(String, Uuid), InboxMessage>>>,
    }

    #[async_trait::async_trait]
    impl InboxRepository for FakeInbox {
        async fn receive(
            &self,
            message: &InboxMessage,
            _ctx: &RequestContext,
        ) -> Result<InboxMessage, foundation::PlatformError> {
            let mut m = lock(&self.messages);
            let key = (message.consumer_id.clone(), message.message_id);
            if let Some(existing) = m.get(&key) {
                return Ok(existing.clone());
            }
            m.insert(key, message.clone());
            Ok(message.clone())
        }

        async fn complete(
            &self,
            consumer_id: &str,
            message_id: Uuid,
            _result_digest: &str,
            _ctx: &RequestContext,
        ) -> Result<InboxMessage, foundation::PlatformError> {
            let mut m = lock(&self.messages);
            let key = (consumer_id.to_string(), message_id);
            match m.get_mut(&key) {
                Some(msg) => {
                    msg.status = InboxStatus::Completed;
                    Ok(msg.clone())
                }
                None => Err(foundation::PlatformError::invalid(
                    "message_id",
                    "unknown inbox message",
                )),
            }
        }
    }

    fn event(payload: SignalingEventPayload) -> SignalingEvent {
        let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
        SignalingEvent {
            last_event_id: foundation::MessageId::generate(&generator).to_string(),
            tenant_id: TenantId::generate(&generator),
            device_id: DeviceId::generate(&generator),
            observed_at: UtcTimestamp::from(chrono::Utc::now()),
            payload,
        }
    }

    #[test]
    fn device_event_is_idempotent_through_inbox() {
        let projection = FakeProjection {
            devices: Arc::new(Mutex::new(vec![])),
            channels: Arc::new(Mutex::new(vec![])),
            failures: Arc::new(Mutex::new(vec![])),
        };
        let inbox = FakeInbox {
            messages: Arc::new(Mutex::new(HashMap::new())),
        };
        let processor = SignalingEventProcessor;
        let ctx = RequestContext::default();
        let e = event(SignalingEventPayload::DeviceOnline);

        match block_on(processor.process(&e, &projection, &inbox, &ctx)) {
            Ok(_) => {}
            Err(_) => panic!("first process failed"),
        }
        match block_on(processor.process(&e, &projection, &inbox, &ctx)) {
            Ok(_) => {}
            Err(_) => panic!("second process failed"),
        }

        assert_eq!(lock(&projection.devices).len(), 1);
    }

    #[test]
    fn channel_event_updates_projection() {
        let projection = FakeProjection {
            devices: Arc::new(Mutex::new(vec![])),
            channels: Arc::new(Mutex::new(vec![])),
            failures: Arc::new(Mutex::new(vec![])),
        };
        let inbox = FakeInbox {
            messages: Arc::new(Mutex::new(HashMap::new())),
        };
        let processor = SignalingEventProcessor;
        let ctx = RequestContext::default();
        let e = event(SignalingEventPayload::ChannelState {
            channel_id: "ch1".to_string(),
            is_enabled: true,
        });

        match block_on(processor.process(&e, &projection, &inbox, &ctx)) {
            Ok(_) => {}
            Err(_) => panic!("process failed"),
        }

        assert_eq!(lock(&projection.channels).len(), 1);
    }

    #[test]
    fn rest_adapter_returns_unavailable_without_base_url() {
        use crate::RestSignalingAdapter;
        use domain_signaling::SignalingPort;

        let adapter = RestSignalingAdapter::new(None);
        let result = block_on(adapter.get_device(
            TenantId::generate(&SystemIdGenerator::new(SystemClock, SystemRandom)),
            DeviceId::generate(&SystemIdGenerator::new(SystemClock, SystemRandom)),
            Deadline::new(UtcTimestamp::from(
                chrono::Utc::now() + chrono::Duration::seconds(30),
            )),
        ));
        match result {
            Ok(_) => panic!("expected unavailable"),
            Err(e) => assert_eq!(e.kind, SignalingErrorKind::Unavailable),
        }
    }
}
