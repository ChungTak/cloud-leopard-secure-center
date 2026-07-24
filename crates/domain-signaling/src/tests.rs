#![allow(clippy::expect_used, clippy::unwrap_used)]

use foundation::chrono::{DateTime, Duration, Utc};
use foundation::{
    Clock, Deadline, DeviceId, SystemClock, SystemIdGenerator, SystemRandom, TenantId, UtcTimestamp,
};
use futures::executor::block_on;

use crate::{
    CreateMediaSessionRequest, CreateOperationRequest, MediaSessionState, OperationState,
    SignalingErrorKind, SignalingPort, UnsupportedSignalingPort,
};

fn tenant() -> TenantId {
    TenantId::generate(&SystemIdGenerator::new(SystemClock, SystemRandom))
        .expect("generate tenant id")
}

fn device() -> DeviceId {
    DeviceId::generate(&SystemIdGenerator::new(SystemClock, SystemRandom))
        .expect("generate device id")
}

fn deadline() -> Deadline {
    let now: DateTime<Utc> = SystemClock.now().into();
    Deadline::new(UtcTimestamp::from(now + Duration::seconds(30)))
}

#[test]
fn unsupported_get_device_returns_unsupported() {
    let port = UnsupportedSignalingPort;
    match block_on(port.get_device(tenant(), device(), deadline())) {
        Ok(_) => panic!("expected unsupported error"),
        Err(e) => assert_eq!(e.kind, SignalingErrorKind::Unsupported),
    }
}

#[test]
fn unsupported_create_operation_returns_unsupported() {
    let port = UnsupportedSignalingPort;
    match block_on(port.create_operation(CreateOperationRequest {
        tenant_id: tenant(),
        device_id: device(),
        deadline: deadline(),
        parameters: serde_json::Value::Null,
    })) {
        Ok(_) => panic!("expected unsupported error"),
        Err(e) => assert_eq!(e.kind, SignalingErrorKind::Unsupported),
    }
}

#[test]
fn unsupported_create_media_session_returns_unsupported() {
    let port = UnsupportedSignalingPort;
    let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
    match block_on(port.create_media_session(CreateMediaSessionRequest {
        tenant_id: tenant(),
        operation_id: foundation::OperationId::generate(&generator).expect("generate operation id"),
        deadline: deadline(),
        parameters: serde_json::Value::Null,
    })) {
        Ok(_) => panic!("expected unsupported error"),
        Err(e) => assert_eq!(e.kind, SignalingErrorKind::Unsupported),
    }
}

#[test]
fn operation_state_strings_are_stable() {
    assert_eq!(
        super::mapper::operation_to_rest(&super::Operation {
            id: foundation::OperationId::generate(&SystemIdGenerator::new(
                SystemClock,
                SystemRandom
            ))
            .expect("generate operation id"),
            tenant_id: tenant(),
            device_id: device(),
            state: OperationState::Running,
            deadline: deadline(),
        })
        .state,
        "running"
    );
}

#[test]
fn media_session_state_strings_are_stable() {
    let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
    assert_eq!(
        super::mapper::media_session_to_rest(&super::MediaSession {
            id: foundation::MediaSessionId::generate(&generator)
                .expect("generate media session id"),
            tenant_id: tenant(),
            operation_id: foundation::OperationId::generate(&generator)
                .expect("generate operation id"),
            state: MediaSessionState::Active,
            deadline: deadline(),
        })
        .state,
        "active"
    );
}
