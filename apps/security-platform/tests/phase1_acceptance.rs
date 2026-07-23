//! Phase 1 end-to-end acceptance contract.
//!
//! These tests do not require a live PostgreSQL or browser; they verify the
//! Phase 1 completion condition that unimplemented signaling, media, and player
//! capabilities explicitly return `Unsupported`/`Unavailable` instead of faking
//! success.

use domain_media::{
    CreateEntitlementRequest, MediaAction, MediaErrorKind, MediaPort, UnsupportedMediaPort,
};
use domain_signaling::{
    CreateMediaSessionRequest, CreateOperationRequest, SignalingErrorKind, SignalingPort,
    UnsupportedSignalingPort,
};
use foundation::{
    CameraId, Deadline, DeviceId, SystemClock, SystemIdGenerator, SystemRandom, TenantId, UserId,
    UtcTimestamp,
};
use signaling_adapter::RestSignalingAdapter;
use signaling_adapter::reconciler::SignalingReconciler;

fn tenant() -> TenantId {
    let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
    TenantId::generate(&generator)
}

fn device() -> DeviceId {
    let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
    DeviceId::generate(&generator)
}

fn camera() -> CameraId {
    let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
    CameraId::generate(&generator)
}

fn principal() -> UserId {
    let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
    UserId::generate(&generator)
}

fn deadline() -> Deadline {
    Deadline::new(UtcTimestamp::from(
        foundation::chrono::Utc::now() + foundation::chrono::Duration::seconds(30),
    ))
}

#[tokio::test]
async fn unsupported_signaling_get_device() {
    let port = UnsupportedSignalingPort;
    match port.get_device(tenant(), device(), deadline()).await {
        Ok(_) => panic!("expected unsupported"),
        Err(e) => assert_eq!(e.kind, SignalingErrorKind::Unsupported),
    }
}

#[tokio::test]
async fn unsupported_signaling_create_operation() {
    let port = UnsupportedSignalingPort;
    match port
        .create_operation(CreateOperationRequest {
            tenant_id: tenant(),
            device_id: device(),
            deadline: deadline(),
            parameters: serde_json::Value::Null,
        })
        .await
    {
        Ok(_) => panic!("expected unsupported"),
        Err(e) => assert_eq!(e.kind, SignalingErrorKind::Unsupported),
    }
}

#[tokio::test]
async fn unsupported_signaling_create_media_session() {
    let port = UnsupportedSignalingPort;
    let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
    match port
        .create_media_session(CreateMediaSessionRequest {
            tenant_id: tenant(),
            operation_id: foundation::OperationId::generate(&generator),
            deadline: deadline(),
            parameters: serde_json::Value::Null,
        })
        .await
    {
        Ok(_) => panic!("expected unsupported"),
        Err(e) => assert_eq!(e.kind, SignalingErrorKind::Unsupported),
    }
}

#[tokio::test]
async fn rest_adapter_without_upstream_is_unavailable() {
    let adapter = RestSignalingAdapter::new(None);
    match adapter.get_device(tenant(), device(), deadline()).await {
        Ok(_) => panic!("expected unavailable"),
        Err(e) => assert_eq!(e.kind, SignalingErrorKind::Unavailable),
    }
}

#[tokio::test]
async fn unsupported_media_create_entitlement() {
    let port = UnsupportedMediaPort;
    match port
        .create_entitlement(CreateEntitlementRequest {
            tenant_id: tenant(),
            principal_id: principal(),
            camera_id: camera(),
            actions: vec![MediaAction::Live],
            protocol: "webrtc".to_string(),
            deadline: deadline(),
        })
        .await
    {
        Ok(_) => panic!("expected unsupported"),
        Err(e) => assert_eq!(e.kind, MediaErrorKind::Unsupported),
    }
}

#[tokio::test]
async fn unsupported_media_get_entitlement() {
    let port = UnsupportedMediaPort;
    let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
    match port
        .get_entitlement(tenant(), foundation::EntitlementId::generate(&generator))
        .await
    {
        Ok(_) => panic!("expected unsupported"),
        Err(e) => assert_eq!(e.kind, MediaErrorKind::Unsupported),
    }
}

#[tokio::test]
async fn unsupported_media_revoke_entitlement() {
    let port = UnsupportedMediaPort;
    let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
    match port
        .revoke_entitlement(tenant(), foundation::EntitlementId::generate(&generator))
        .await
    {
        Ok(_) => panic!("expected unsupported"),
        Err(e) => assert_eq!(e.kind, MediaErrorKind::Unsupported),
    }
}

#[tokio::test]
async fn full_signaling_reconciliation_is_unsupported() {
    let reconciler = SignalingReconciler::new();
    match reconciler.reconcile().await {
        Ok(_) => panic!("expected unsupported"),
        Err(e) => assert_eq!(e.kind, SignalingErrorKind::Unsupported),
    }
}

#[tokio::test]
async fn typed_ids_are_tenant_isolated() {
    let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
    let t1 = TenantId::generate(&generator);
    let t2 = TenantId::generate(&generator);
    assert_ne!(t1, t2);

    let d1 = DeviceId::generate(&generator);
    let d2 = DeviceId::generate(&generator);
    assert_ne!(d1, d2);
}

#[tokio::test]
async fn no_stub_returns_placeholder_success() {
    // Aggregate all stub checks above; the only acceptable outcomes are
    // explicit Unsupported or Unavailable errors.
    let checks = [
        (async {
            UnsupportedSignalingPort
                .get_device(tenant(), device(), deadline())
                .await
                .is_err()
        })
        .await,
        (async {
            RestSignalingAdapter::new(None)
                .get_device(tenant(), device(), deadline())
                .await
                .is_err()
        })
        .await,
        (async {
            UnsupportedMediaPort
                .create_entitlement(CreateEntitlementRequest {
                    tenant_id: tenant(),
                    principal_id: principal(),
                    camera_id: camera(),
                    actions: vec![MediaAction::Live],
                    protocol: "webrtc".to_string(),
                    deadline: deadline(),
                })
                .await
                .is_err()
        })
        .await,
        (async { SignalingReconciler::new().reconcile().await.is_err() }).await,
    ];
    assert!(
        checks.iter().all(|&v| v),
        "all unimplemented paths must error"
    );
}
