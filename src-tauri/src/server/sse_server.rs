use axum::{
    extract::State,
    http::{HeaderValue, Method},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Json,
    },
    routing::get,
    Router,
};
use futures_core::Stream;
use serde_json::json;
use std::convert::Infallible;
use std::net::SocketAddr;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing::info;

use crate::watcher::bus::{EventBus, WatcherEvent};
use crate::watcher::state::WatcherState;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
struct SseState {
    watcher_state: WatcherState,
    event_bus: EventBus,
}

pub async fn start_sse_server(
    port: u16,
    allowed_origins: Vec<String>,
    watcher_state: WatcherState,
    event_bus: EventBus,
) -> Result<(), String> {
    let origins: Vec<HeaderValue> = allowed_origins
        .iter()
        .filter_map(|o| o.parse().ok())
        .collect();

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([Method::GET]);

    let state = SseState {
        watcher_state,
        event_bus,
    };

    let app = Router::new()
        .route("/ping", get(ping_handler))
        .route("/state", get(state_handler))
        .route("/events", get(sse_handler))
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    info!("SSE server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("Failed to bind port {}: {}", port, e))?;

    axum::serve(listener, app)
        .await
        .map_err(|e| format!("Server error: {}", e))
}

async fn ping_handler() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "version": VERSION
    }))
}

async fn state_handler(State(state): State<SseState>) -> impl IntoResponse {
    let s = state.watcher_state.inner.lock().await;
    let active = s.snapshot_active();
    Json(json!({ "active": active }))
}

async fn sse_handler(
    State(state): State<SseState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.event_bus.subscribe();

    // Send initial state snapshot
    let snapshot = {
        let s = state.watcher_state.inner.lock().await;
        s.snapshot_active()
    };

    let initial_event = WatcherEvent::StateSnapshot { active: snapshot };
    let initial_data = serde_json::to_string(&initial_event).unwrap_or_default();

    let stream = async_stream::stream! {
        // Emit initial snapshot
        yield Ok(Event::default().data(initial_data));

        // Then stream live events
        let mut stream = BroadcastStream::new(rx);
        while let Some(result) = stream.next().await {
            match result {
                Ok(event) => {
                    if let Ok(data) = serde_json::to_string(&event) {
                        yield Ok(Event::default().data(data));
                    }
                }
                Err(_) => {
                    // Lagged - missed some events, continue
                    continue;
                }
            }
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
}
