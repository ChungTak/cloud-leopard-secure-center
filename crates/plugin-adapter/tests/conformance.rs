//! Plugin conformance suite.
//!
//! Phase 1 conformance freezes the contract surface: manifest fields, lifecycle
//! transitions, unsupported verifier, and unsupported host ports. Real examples
//! (Wasm and process) are deferred to the SDK implementation.

use std::collections::HashSet;

use foundation::{SystemClock, SystemIdGenerator, SystemRandom, TenantId};
use plugin_adapter::grpc::{PluginFrame, PluginHello, ProcessPluginHost, UnsupportedProcessPluginHost};
use plugin_adapter::manifest::{
    ManifestVerifier, Plugin, PluginErrorKind, PluginKind, PluginManifest, PluginState,
    UnsupportedManifestVerifier,
};
use plugin_adapter::wit::{UnsupportedWitHost, WitHost};

fn make_manifest() -> PluginManifest {
    let generator = SystemIdGenerator::new(SystemClock, SystemRandom);
    PluginManifest {
        plugin_id: foundation::PluginId::generate(&generator),
        tenant_id: TenantId::generate(&generator),
        version: "0.1.0".to_string(),
        kind: PluginKind::Wasm,
        api_range: "v1".to_string(),
        capabilities: ["read".to_string()].into_iter().collect::<HashSet<_>>(),
        resources: ["camera".to_string()].into_iter().collect::<HashSet<_>>(),
        events: ["alarm".to_string()].into_iter().collect::<HashSet<_>>(),
        config_digest: "sha256:abc".to_string(),
        publisher: "publisher".to_string(),
        signature: "sig".to_string(),
        checksum: "sum".to_string(),
    }
}

fn ok_or_panic<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
    match result {
        Ok(v) => v,
        Err(e) => panic!("{e}"),
    }
}

fn err_or_panic<T, E: std::fmt::Debug>(result: Result<T, E>) -> E {
    match result {
        Err(e) => e,
        Ok(_) => panic!("expected Err"),
    }
}

#[test]
fn manifest_contains_required_fields() {
    let manifest = make_manifest();
    assert!(!manifest.version.is_empty());
    assert!(!manifest.api_range.is_empty());
    assert!(!manifest.config_digest.is_empty());
    assert!(!manifest.signature.is_empty());
    assert!(!manifest.checksum.is_empty());
}

#[test]
fn illegal_lifecycle_transition_fails() {
    let mut plugin = ok_or_panic(Plugin::upload(make_manifest()));
    let err = err_or_panic(plugin.transition(PluginState::Enabled));
    assert_eq!(err.kind, PluginErrorKind::Invalid);
}

#[test]
fn quarantine_is_reachable_from_multiple_states() {
    let mut plugin = ok_or_panic(Plugin::upload(make_manifest()));
    ok_or_panic(plugin.transition(PluginState::Verified));
    ok_or_panic(plugin.transition(PluginState::Installed));
    ok_or_panic(plugin.transition(PluginState::Migrated));
    ok_or_panic(plugin.transition(PluginState::Enabled));
    ok_or_panic(plugin.transition(PluginState::Quarantined));
    assert_eq!(plugin.state, PluginState::Quarantined);
}

#[tokio::test]
async fn manifest_verifier_is_unsupported() {
    let plugin = ok_or_panic(Plugin::upload(make_manifest()));
    let verifier = UnsupportedManifestVerifier;
    let err = err_or_panic(verifier.verify(&plugin).await);
    assert_eq!(err.kind, PluginErrorKind::Unsupported);
}

#[tokio::test]
async fn wit_host_ports_are_unsupported_when_enabled() {
    let host = UnsupportedWitHost::new(true);
    let plugin_id = foundation::PluginId::generate(&SystemIdGenerator::new(
        SystemClock,
        SystemRandom,
    ));
    let err = err_or_panic(
        host.read_config(plugin_id, "key").await,
    );
    assert_eq!(err.kind, PluginErrorKind::Unsupported);
}

#[tokio::test]
async fn grpc_host_ports_are_unsupported_when_enabled() {
    let host = UnsupportedProcessPluginHost::new(true);
    let hello = PluginHello {
        plugin_id: foundation::PluginId::generate(&SystemIdGenerator::new(
            SystemClock,
            SystemRandom,
        )),
        version: "0.1.0".to_string(),
        instance: "i1".to_string(),
        scope: vec!["read".to_string()],
        credits: 1,
    };
    let err = err_or_panic(host.handshake(&hello).await);
    assert_eq!(err.kind, PluginErrorKind::Unsupported);
}

#[test]
fn plugin_frame_roundtrips() {
    let frame = PluginFrame::Event {
        seq: 1,
        event_type: "alarm".to_string(),
        payload: serde_json::json!({"camera_id": "c1"}),
    };
    let serialized = ok_or_panic(serde_json::to_string(&frame));
    let deserialized: PluginFrame = ok_or_panic(serde_json::from_str(&serialized));
    assert_eq!(frame, deserialized);
}
