//! Integration tests for API-003: ETags, idempotency keys, cursor pagination, and SSE.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{Extension, Query},
    http::{Request, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, put},
};
use foundation::Revision;
use futures::StreamExt;
use http_api::{
    error::AppError,
    etag::{ETag, IfMatch},
    idempotency::{IdempotencyState, idempotency},
    pagination::{Pagination, PaginationConfig},
    sse::{EventBus, LastEventId, sse_response},
};
use http_body_util::BodyExt;
use serde::Deserialize;
use serde_json::json;
use tokio::time::timeout;
use tower::util::ServiceExt;

struct TestApp {
    router: Router,
    bus: EventBus,
}

fn build_request(method: &str, uri: &str, body: Body) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .body(body)
        .expect("failed to build request")
}

fn test_app() -> TestApp {
    let config =
        Arc::new(PaginationConfig::new(3, "a".repeat(32).into_bytes()).expect("valid config"));
    let counter = Arc::new(AtomicU64::new(0));
    let idempotency_state = Arc::new(IdempotencyState::new(Duration::from_secs(60)));
    let revision = Arc::new(tokio::sync::Mutex::new(Revision::initial()));
    let bus = EventBus::new(4, 4);

    let router = Router::new()
        .route("/items", get(get_items).post(create_item))
        .route("/items/{id}", put(update_item))
        .route("/events", get(events))
        .route_layer(axum::middleware::from_fn(idempotency))
        .layer(Extension(config))
        .layer(Extension(counter))
        .layer(Extension(idempotency_state))
        .layer(Extension(revision))
        .layer(Extension(bus.clone()));

    TestApp { router, bus }
}

async fn get_items(
    pagination: Pagination,
    Extension(config): Extension<Arc<PaginationConfig>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let all: Vec<serde_json::Value> = (0..10).map(|i| json!({"id": i.to_string()})).collect();
    let offset = pagination.offset as usize;
    let limit = pagination.limit as usize;
    let has_more = offset + limit < all.len();
    let items = all
        .iter()
        .skip(offset)
        .take(limit)
        .cloned()
        .collect::<Vec<_>>();
    let next_cursor = pagination.next_cursor(has_more, &config.cursor_secret)?;
    Ok(Json(
        json!({"items": items, "next_cursor": next_cursor, "limit": pagination.limit}),
    ))
}

async fn create_item(Extension(counter): Extension<Arc<AtomicU64>>) -> Json<serde_json::Value> {
    let id = counter.fetch_add(1, Ordering::SeqCst);
    Json(json!({"id": id, "created": true}))
}

async fn update_item(
    if_match: IfMatch,
    Extension(revision): Extension<Arc<tokio::sync::Mutex<Revision>>>,
) -> Result<impl IntoResponse, AppError> {
    let mut rev = revision.lock().await;
    if_match.verify(*rev)?;
    *rev = rev.next();
    let etag = ETag(*rev).header_value()?;
    Ok((
        StatusCode::OK,
        [(header::ETAG, etag)],
        Json(json!({"revision": rev.value()})),
    ))
}

#[derive(Deserialize)]
struct EventQuery {
    filter: Option<String>,
}

async fn events(
    Query(query): Query<EventQuery>,
    last_event_id: LastEventId,
    Extension(bus): Extension<EventBus>,
) -> Response {
    sse_response(bus.subscribe(query.filter, last_event_id.0).await)
}

#[tokio::test]
async fn pagination_cursor_is_opaque_and_verifiable() {
    let app = test_app();

    let response = app
        .router
        .clone()
        .oneshot(build_request("GET", "/items?limit=2", Body::empty()))
        .await
        .expect("request failed");
    assert_eq!(response.status(), StatusCode::OK);

    let body = collect_json(response).await;
    let items = body["items"].as_array().expect("items array");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["id"], "0");
    assert_eq!(items[1]["id"], "1");
    assert!(body["next_cursor"].is_string());
    assert_eq!(body["limit"], 2);

    let cursor = body["next_cursor"].as_str().unwrap().to_string();
    let response = app
        .router
        .clone()
        .oneshot(build_request(
            "GET",
            &format!("/items?cursor={cursor}"),
            Body::empty(),
        ))
        .await
        .expect("request failed");
    assert_eq!(response.status(), StatusCode::OK);

    let body = collect_json(response).await;
    let items = body["items"].as_array().expect("items array");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["id"], "2");
    assert_eq!(items[1]["id"], "3");

    // Tampered cursor is rejected.
    let mut tampered = cursor.clone();
    if tampered.ends_with('A') {
        tampered.pop();
        tampered.push('B');
    } else {
        tampered.pop();
        tampered.push('A');
    }
    let response = app
        .router
        .clone()
        .oneshot(build_request(
            "GET",
            &format!("/items?cursor={tampered}"),
            Body::empty(),
        ))
        .await
        .expect("request failed");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Excess limit is clamped to the configured maximum.
    let response = app
        .router
        .clone()
        .oneshot(build_request("GET", "/items?limit=100", Body::empty()))
        .await
        .expect("request failed");
    let body = collect_json(response).await;
    assert_eq!(body["limit"], 3);
}

#[tokio::test]
async fn etag_conflict_returns_412() {
    let app = test_app();

    let response = app
        .router
        .clone()
        .oneshot(build_request("PUT", "/items/1", Body::empty()))
        .await
        .expect("request failed");
    assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);

    let response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/items/1")
                .header(header::IF_MATCH, "\"99\"")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request failed");
    assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);

    let response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/items/1")
                .header(header::IF_MATCH, "\"1\"")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request failed");
    assert_eq!(response.status(), StatusCode::OK);
    let etag = response
        .headers()
        .get(header::ETAG)
        .and_then(|v| v.to_str().ok())
        .expect("ETag header");
    assert_eq!(etag, "\"2\"");

    // Second update with stale ETag fails.
    let response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/items/1")
                .header(header::IF_MATCH, "\"1\"")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request failed");
    assert_eq!(response.status(), StatusCode::PRECONDITION_FAILED);
}

#[tokio::test]
async fn idempotency_key_returns_same_response_and_rejects_conflicting_body() {
    let app = test_app();

    let key = "idem-001";
    let body_a = Body::from(json!({"name": "a"}).to_string());
    let body_b = Body::from(json!({"name": "b"}).to_string());

    let response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/items")
                .header("idempotency-key", key)
                .header(header::CONTENT_TYPE, "application/json")
                .body(body_a)
                .unwrap(),
        )
        .await
        .expect("request failed");
    assert_eq!(response.status(), StatusCode::OK);
    let first = collect_json(response).await;
    assert_eq!(first["id"], 0);

    let response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/items")
                .header("idempotency-key", key)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"name": "a"}).to_string()))
                .unwrap(),
        )
        .await
        .expect("request failed");
    assert_eq!(response.status(), StatusCode::OK);
    let second = collect_json(response).await;
    assert_eq!(second["id"], 0);

    // Same key with a different request body is a conflict.
    let response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/items")
                .header("idempotency-key", key)
                .header(header::CONTENT_TYPE, "application/json")
                .body(body_b)
                .unwrap(),
        )
        .await
        .expect("request failed");
    assert_eq!(response.status(), StatusCode::CONFLICT);

    // A different key produces a new resource.
    let response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/items")
                .header("idempotency-key", "idem-002")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({"name": "a"}).to_string()))
                .unwrap(),
        )
        .await
        .expect("request failed");
    assert_eq!(response.status(), StatusCode::OK);
    let third = collect_json(response).await;
    assert_eq!(third["id"], 1);
}

#[tokio::test]
async fn sse_filter_and_last_event_id_replay() {
    let app = test_app();

    let mut stream = app
        .router
        .clone()
        .oneshot(build_request("GET", "/events?filter=alerts", Body::empty()))
        .await
        .expect("request failed")
        .into_body()
        .into_data_stream();

    let _heartbeat = app.bus.publish("heartbeat", "ok").await.unwrap();
    let alert1 = app
        .bus
        .publish_with_filters("alert", "fire", vec!["alerts".to_string()])
        .await
        .unwrap();

    let first = read_sse_line(&mut stream).await;
    assert!(first.contains("event: alert"));
    assert!(first.contains(&format!("id: {alert1}")));

    // Reconnect from the first alert and receive subsequent events.
    let mut stream = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/events?filter=alerts")
                .header("last-event-id", alert1.clone())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request failed")
        .into_body()
        .into_data_stream();

    let alert2 = app
        .bus
        .publish_with_filters("alert", "motion", vec!["alerts".to_string()])
        .await
        .unwrap();
    let second = read_sse_line(&mut stream).await;
    assert!(second.contains("event: alert"));
    assert!(second.contains(&format!("id: {alert2}")));

    // A missing old event id produces a gap event telling the client to re-query.
    let mut stream = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/events?filter=alerts")
                .header("last-event-id", "000")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request failed")
        .into_body()
        .into_data_stream();

    let gap = read_sse_line(&mut stream).await;
    assert!(gap.contains("event: gap"));
}

#[tokio::test]
async fn slow_sse_client_receives_gap_event() {
    let app = test_app();

    let mut stream = app
        .router
        .clone()
        .oneshot(build_request("GET", "/events", Body::empty()))
        .await
        .expect("request failed")
        .into_body()
        .into_data_stream();

    // Publish more events than the broadcast buffer can hold while the client
    // is not polling. The receiver will lag and emit a gap event.
    for i in 0..10 {
        app.bus.publish("event", i.to_string()).await.unwrap();
    }

    let first = read_sse_line(&mut stream).await;
    assert!(first.contains("event: gap"));
}

async fn collect_json(response: Response) -> serde_json::Value {
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body collect failed")
        .to_bytes();
    serde_json::from_slice(&body).expect("body is not valid JSON")
}

async fn read_sse_line<S>(stream: &mut S) -> String
where
    S: futures::stream::Stream<Item = Result<Bytes, axum::Error>> + Unpin,
{
    let chunk = timeout(Duration::from_millis(500), stream.next())
        .await
        .expect("SSE timeout")
        .expect("SSE stream ended")
        .expect("SSE chunk error");
    String::from_utf8_lossy(&chunk).to_string()
}
