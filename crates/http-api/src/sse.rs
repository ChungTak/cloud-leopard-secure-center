//! Server-Sent Events (SSE) bus with bounded buffering, filtering, and gap detection.

use axum::{
    extract::FromRequestParts,
    http::request::Parts,
    response::{IntoResponse, Response, Sse},
};
use futures::stream::{self, Stream, StreamExt};
use std::{
    collections::VecDeque,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
use tokio::sync::{Mutex, broadcast};
use tokio_stream::wrappers::BroadcastStream;

use crate::error::AppError;

/// SSE event payload.
#[derive(Debug, Clone)]
pub struct Event {
    /// Monotonic event identifier. Clients send this back as `Last-Event-ID`.
    pub id: String,
    /// Event name (maps to SSE `event:` field).
    pub name: String,
    /// Event payload (maps to SSE `data:` field).
    pub data: String,
    /// Filter tags the client can subscribe to.
    pub filters: Vec<String>,
}

impl Event {
    /// Convert to an `axum` SSE event.
    pub fn to_sse(&self) -> axum::response::sse::Event {
        axum::response::sse::Event::default()
            .id(self.id.clone())
            .event(self.name.clone())
            .data(self.data.clone())
    }
}

/// Extractor for `Last-Event-ID`.
pub struct LastEventId(pub Option<String>);

impl<S> FromRequestParts<S> for LastEventId
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get("last-event-id")
            .and_then(|value| value.to_str().ok())
            .map(|s| s.to_string());
        Ok(Self(header))
    }
}

/// Shared event bus with a bounded ring buffer and topic filtering.
#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<Arc<Event>>,
    history: Arc<Mutex<VecDeque<Arc<Event>>>>,
    history_capacity: usize,
    next_id: Arc<AtomicU64>,
}

impl EventBus {
    /// Create an event bus that keeps the last `history_capacity` events in
    /// addition to the live broadcast channel capacity. Capacities are clamped
    /// to positive, bounded values.
    pub fn new(broadcast_capacity: usize, history_capacity: usize) -> Self {
        const MAX_CAPACITY: usize = 100_000;
        let broadcast_capacity = broadcast_capacity.clamp(1, MAX_CAPACITY);
        let history_capacity = history_capacity.clamp(1, MAX_CAPACITY);
        let (sender, _) = broadcast::channel(broadcast_capacity);
        Self {
            sender,
            history: Arc::new(Mutex::new(VecDeque::with_capacity(history_capacity))),
            history_capacity,
            next_id: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Publish an event and return its assigned id.
    pub async fn publish(&self, name: impl Into<String>, data: impl Into<String>) -> String {
        self.publish_with_filters(name, data, Vec::new()).await
    }

    /// Publish an event with filter tags and return its assigned id.
    pub async fn publish_with_filters(
        &self,
        name: impl Into<String>,
        data: impl Into<String>,
        filters: Vec<String>,
    ) -> String {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst).to_string();
        let event = Arc::new(Event {
            id: id.clone(),
            name: name.into(),
            data: data.into(),
            filters,
        });
        {
            let mut history = self.history.lock().await;
            if history.len() >= self.history_capacity {
                history.pop_front();
            }
            history.push_back(Arc::clone(&event));
        }
        let _ = self.sender.send(event);
        id
    }

    /// Subscribe to events, optionally filtered by tag and replaying from `Last-Event-ID`.
    pub async fn subscribe(
        &self,
        filter: Option<String>,
        last_event_id: Option<String>,
    ) -> Pin<
        Box<dyn Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>> + Send>,
    > {
        let filter = filter.map(|f| f.to_lowercase());

        let mut replay = Vec::new();
        let mut missed = false;
        if let Some(last_id) = last_event_id {
            let history = self.history.lock().await;
            if let Some(pos) = history.iter().rposition(|e| e.id == last_id) {
                for event in history.iter().skip(pos + 1) {
                    if matches_filter(event, filter.as_deref()) {
                        replay.push(event.to_sse());
                    }
                }
            } else {
                missed = true;
            }
        }

        if missed {
            replay.push(gap_event());
        }

        let receiver = self.sender.subscribe();
        let lagged = Arc::new(AtomicBool::new(false));
        let live = BroadcastStream::new(receiver).filter_map(move |result| {
            let filter = filter.clone();
            let lagged = Arc::clone(&lagged);
            async move {
                match result {
                    Ok(event) if matches_filter(&event, filter.as_deref()) => {
                        Some(Ok(event.to_sse()))
                    }
                    Ok(_) => None,
                    Err(_) if !lagged.swap(true, Ordering::SeqCst) => Some(Ok(gap_event())),
                    Err(_) => None,
                }
            }
        });

        Box::pin(stream::iter(replay).map(Ok).chain(live))
    }

    /// Number of subscribers currently connected.
    pub fn receiver_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl std::fmt::Debug for EventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventBus")
            .field("capacity", &self.history_capacity)
            .finish_non_exhaustive()
    }
}

fn gap_event() -> axum::response::sse::Event {
    axum::response::sse::Event::default()
        .event("gap")
        .data("events missed; re-query state")
}

fn matches_filter(event: &Event, filter: Option<&str>) -> bool {
    match filter {
        None => true,
        Some(tag) => event.filters.iter().any(|f| f.eq_ignore_ascii_case(tag)),
    }
}

/// Build an SSE endpoint response from a stream.
pub fn sse_response<S>(stream: S) -> Response
where
    S: Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>> + Send + 'static,
{
    Sse::new(stream).into_response()
}
