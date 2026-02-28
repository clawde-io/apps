// rest/sse.rs — SSE push event bridge (Sprint QQ RA.4).
//
// GET /api/v1/sessions/{id}/events
//
// Streams session push events as Server-Sent Events. The client subscribes
// to the daemon's broadcast channel and forwards matching events.

use axum::{
    extract::{Path, State},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
};
use futures_util::stream;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

use crate::AppContext;

pub async fn session_events_sse(
    State(ctx): State<Arc<AppContext>>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let rx = ctx.broadcaster.subscribe();

    let s = stream::unfold((rx, session_id.clone()), move |(mut rx, sid)| async move {
        loop {
            match rx.recv().await {
                Ok(event_str) => {
                    // Parse the JSON string emitted by EventBroadcaster
                    let event: serde_json::Value = match serde_json::from_str(&event_str) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    // Forward events that belong to this session or are global
                    let event_session = event
                        .get("params")
                        .and_then(|p: &serde_json::Value| p.get("session_id"))
                        .and_then(|v: &serde_json::Value| v.as_str())
                        .unwrap_or("");

                    if event_session == sid || event_session.is_empty() {
                        let method = event
                            .get("method")
                            .and_then(|v| v.as_str())
                            .unwrap_or("event")
                            .to_string();
                        let data = json!({
                            "method": event.get("method"),
                            "params": event.get("params"),
                        });
                        let sse_event = Event::default().data(data.to_string()).event(method);
                        return Some((Ok::<Event, std::convert::Infallible>(sse_event), (rx, sid)));
                    }
                    // Not our session — continue loop
                }
                Err(_) => return None,
            }
        }
    });

    Sse::new(s).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    )
}
