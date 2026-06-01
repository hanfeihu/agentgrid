use std::{convert::Infallible, time::Duration};

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio_stream::{wrappers::IntervalStream, StreamExt};

use crate::{now, store, ApiError, AppState, Store};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/messages", get(list_messages).post(create_message))
        .route("/api/events", get(list_events))
        .route("/api/events/stream", get(event_stream))
        .route("/api/audit-events", get(list_audit_events))
}

#[derive(Debug, Deserialize)]
struct MessageQuery {
    limit: Option<u16>,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct EventQuery {
    pub(crate) limit: Option<u16>,
    pub(crate) event_type: Option<String>,
    #[serde(rename = "type")]
    pub(crate) type_alias: Option<String>,
    pub(crate) subject_id: Option<String>,
}

async fn list_messages(
    State(state): State<AppState>,
    Query(query): Query<MessageQuery>,
) -> Result<Json<Value>, ApiError> {
    let limit = query.limit.unwrap_or(100).min(500);
    Ok(Json(
        json!({ "items": store(&state)?.list_messages(limit)? }),
    ))
}

async fn create_message(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let item = store(&state)?.create_message(input)?;
    Ok((StatusCode::CREATED, Json(item)))
}

async fn list_audit_events(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "items": store(&state)?.list_audit_events(200)?
    })))
}

async fn list_events(
    State(state): State<AppState>,
    Query(query): Query<EventQuery>,
) -> Result<Json<Value>, ApiError> {
    let limit = query.limit.unwrap_or(200).min(1000);
    Ok(Json(json!({
        "ok": true,
        "items": store(&state)?.list_events(query, limit)?,
        "next_cursor": null
    })))
}

async fn event_stream(
    State(state): State<AppState>,
    Query(query): Query<EventQuery>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let limit = query.limit.unwrap_or(100).min(500);
    let stream =
        IntervalStream::new(tokio::time::interval(Duration::from_secs(1))).map(move |_| {
            let event = match Store::open(state.db_path.as_ref())
                .and_then(|store| store.list_events(query.clone(), limit))
            {
                Ok(items) => Event::default().event("events.snapshot").json_data(json!({
                    "ok": true,
                    "time": now(),
                    "items": items
                })),
                Err(error) => Event::default().event("events.error").json_data(json!({
                    "ok": false,
                    "error": { "message": error.to_string() },
                    "time": now()
                })),
            }
            .unwrap_or_else(|_| {
                Event::default()
                    .event("events.error")
                    .data("snapshot serialize failed")
            });
            Ok(event)
        });
    Sse::new(stream).keep_alive(KeepAlive::default())
}
