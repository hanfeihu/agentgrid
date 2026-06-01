use std::convert::Infallible;

use axum::{
    extract::{Path, Query, State},
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio_stream::{wrappers::IntervalStream, StreamExt};

use crate::{now, store, ApiError, AppState, Store};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/jobs", get(list_jobs).post(create_job))
        .route("/api/jobs/plan", post(plan_job))
        .route("/api/jobs/recovery/scan", post(job_recovery_scan))
        .route("/api/jobs/reliability", get(job_reliability))
        .route("/api/jobs/{id}", get(get_job))
        .route("/api/jobs/{id}/checkpoints", post(create_job_checkpoint))
        .route("/api/jobs/{id}/execution", get(job_execution))
        .route("/api/jobs/{id}/events", get(job_events))
        .route("/api/events/ingress", post(create_ingress_event))
}

#[derive(Debug, Deserialize)]
pub(crate) struct JobQuery {
    pub(crate) limit: Option<u16>,
    pub(crate) state: Option<String>,
}

async fn list_jobs(
    State(state): State<AppState>,
    Query(query): Query<JobQuery>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "items": store(&state)?.list_jobs(query)?,
        "next_cursor": null
    })))
}

async fn create_job(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<(axum::http::StatusCode, Json<Value>), ApiError> {
    let item = store(&state)?.create_job(input)?;
    let reused = item
        .pointer("/status/idempotency_reused")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Ok((
        if reused {
            axum::http::StatusCode::OK
        } else {
            axum::http::StatusCode::CREATED
        },
        Json(json!({ "ok": true, "reused": reused, "item": item })),
    ))
}

async fn plan_job(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.plan_job(input)?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn job_reliability(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.job_reliability_status()?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn job_recovery_scan(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.job_recovery_scan("manual")?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn get_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?
        .get_job_detail(&id)?
        .ok_or_else(|| ApiError::not_found("Job not found"))?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn create_job_checkpoint(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.create_job_checkpoint(&id, input)?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn job_execution(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?
        .job_execution_view(&id)?
        .ok_or_else(|| ApiError::not_found("Job not found"))?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn job_events(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let stream = IntervalStream::new(tokio::time::interval(std::time::Duration::from_secs(1))).map(
        move |_| {
            let event = match Store::open(state.db_path.as_ref())
                .and_then(|store| store.job_event_snapshot(&id))
            {
                Ok(snapshot) => Event::default().event("job.snapshot").json_data(snapshot),
                Err(error) => Event::default().event("job.error").json_data(json!({
                    "ok": false,
                    "job_id": id,
                    "error": { "message": error.to_string() },
                    "time": now()
                })),
            }
            .unwrap_or_else(|_| {
                Event::default()
                    .event("job.error")
                    .data("snapshot serialize failed")
            });
            Ok(event)
        },
    );
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn create_ingress_event(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<(axum::http::StatusCode, Json<Value>), ApiError> {
    let item = store(&state)?.create_ingress_event(input)?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(json!({ "ok": true, "item": item })),
    ))
}
