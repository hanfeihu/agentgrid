use std::{convert::Infallible, time::Duration};

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio_stream::{wrappers::IntervalStream, StreamExt};

use crate::{
    agent_runtime_examples, agent_runtime_result_schema, agent_runtime_submit_schema, now, store,
    ApiError, AppState, Store, AGENTGRID_BUILD_VERSION, API_VERSION, PROJECT_ID,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/agent-runtime/manifest", get(agent_runtime_manifest))
        .route("/api/agent-runtime/tasks", post(agent_runtime_submit_task))
        .route("/api/agent-runtime/tasks/{id}", get(agent_runtime_get_task))
        .route("/api/agent-runtime/tasks/{id}/events", get(task_events))
        .route("/api/task-templates", get(list_task_templates))
        .route("/api/task-templates/{id}", get(get_task_template))
        .route("/api/task-templates/{id}/start", post(start_task_template))
        .route("/api/tasks", get(list_tasks).post(create_task))
        .route("/api/tasks/{id}", get(get_task))
        .route("/api/tasks/{id}/snapshot", get(task_snapshot))
        .route(
            "/api/tasks/{id}/schedule-preview",
            get(task_schedule_preview),
        )
        .route("/api/tasks/{id}/events", get(task_events))
        .route("/api/tasks/{id}/control", post(control_task))
        .route("/api/tasks/{id}/{action}", post(update_task))
        .route("/api/worker/lease", post(lease_tasks))
        .route("/api/worker/reconcile", post(worker_reconcile))
        .route("/api/worker/tasks/{id}/control", get(worker_task_control))
        .route("/api/worker/tasks/{id}/renew", post(renew_worker_task))
        .route("/api/worker/tasks/{id}/logs", post(worker_task_log))
        .route(
            "/api/worker/tasks/{id}/complete",
            post(complete_worker_task),
        )
        .route("/api/worker/tasks/{id}/fail", post(fail_worker_task))
}

async fn agent_runtime_manifest(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    let nodes = store.list_nodes()?;
    let tools = store
        .tool_registry_with_dynamic()?
        .into_iter()
        .map(|tool| store.enrich_tool_with_nodes(tool, &nodes))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(Json(json!({
        "ok": true,
        "api_version": API_VERSION,
        "kind": "AgentRuntimeManifest",
        "runtime": {
            "name": "AgentGrid",
            "version": AGENTGRID_BUILD_VERSION,
            "hub_url": "http://chenqi.tminos.com:20080/agentgrid",
            "project_id": PROJECT_ID,
            "protocols": ["AgentMessage", "AgentTask", "ToolContract", "WorkflowDAG"],
            "event_transport": "sse"
        },
        "capabilities": {
            "submit_task": true,
            "watch_task": true,
            "list_tools": true,
            "result_verification": true,
            "resource_aware_scheduling": true,
            "trust_aware_scheduling": true,
            "workflow_dag": true,
            "artifacts": true,
            "audit": true
        },
        "task_submit_endpoint": "/api/agent-runtime/tasks",
        "task_status_endpoint": "/api/agent-runtime/tasks/{task_id}",
        "task_events_endpoint": "/api/agent-runtime/tasks/{task_id}/events",
        "tools": tools,
        "submit_schema": agent_runtime_submit_schema(),
        "result_schema": agent_runtime_result_schema(),
        "examples": agent_runtime_examples()
    })))
}

async fn agent_runtime_submit_task(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let output = store(&state)?.create_agent_runtime_task(input)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "ok": true,
            "api_version": API_VERSION,
            "kind": "AgentRuntimeTaskSubmission",
            "task_id": output.item.pointer("/metadata/id").and_then(Value::as_str),
            "message_id": output.message_id,
            "item": output.item,
            "links": {
                "status": output.item.pointer("/metadata/id").and_then(Value::as_str).map(|id| format!("/api/agent-runtime/tasks/{id}")),
                "events": output.item.pointer("/metadata/id").and_then(Value::as_str).map(|id| format!("/api/agent-runtime/tasks/{id}/events")),
                "task": output.item.pointer("/metadata/id").and_then(Value::as_str).map(|id| format!("/api/tasks/{id}"))
            }
        })),
    ))
}

async fn agent_runtime_get_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let snapshot = store(&state)?.task_event_snapshot(&id)?;
    Ok(Json(json!({
        "ok": true,
        "api_version": API_VERSION,
        "kind": "AgentRuntimeTaskSnapshot",
        "item": snapshot
    })))
}

async fn list_task_templates(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "kind": "TaskTemplateStore",
        "api_version": API_VERSION,
        "items": store(&state)?.list_task_templates(200)?
    })))
}

async fn get_task_template(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?
        .get_task_template(&id)?
        .ok_or_else(|| ApiError::not_found("Task template not found"))?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn start_task_template(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let output = store(&state)?.start_task_template(&id, input)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "ok": true,
            "item": output.item,
            "message_id": output.message_id
        })),
    ))
}

async fn task_schedule_preview(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.task_schedule_preview(&id)?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

#[derive(Debug, Deserialize)]
pub(crate) struct TaskQuery {
    pub(crate) limit: Option<u16>,
    pub(crate) owner: Option<String>,
    pub(crate) state: Option<String>,
}

async fn list_tasks(
    State(state): State<AppState>,
    Query(query): Query<TaskQuery>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "items": store(&state)?.list_tasks(query)?,
        "next_cursor": null
    })))
}

async fn get_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?
        .get_task(&id)?
        .ok_or_else(|| ApiError::not_found("Task not found"))?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn task_snapshot(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.task_event_snapshot(&id)?))
}

async fn task_events(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let stream =
        IntervalStream::new(tokio::time::interval(Duration::from_secs(1))).map(move |_| {
            let event = match Store::open(state.db_path.as_ref())
                .and_then(|store| store.task_event_snapshot(&id))
            {
                Ok(snapshot) => Event::default().event("task.snapshot").json_data(snapshot),
                Err(error) => Event::default().event("task.error").json_data(json!({
                    "ok": false,
                    "task_id": id,
                    "error": { "message": error.to_string() },
                    "time": now()
                })),
            }
            .unwrap_or_else(|_| {
                Event::default()
                    .event("task.error")
                    .data("snapshot serialize failed")
            });
            Ok(event)
        });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn create_task(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let output = store(&state)?.create_task(input)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "ok": true, "item": output.item, "message_id": output.message_id })),
    ))
}

async fn update_task(
    State(state): State<AppState>,
    Path((id, action)): Path<(String, String)>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let output = store(&state)?.update_task(&id, &action, input)?;
    Ok(Json(
        json!({ "ok": true, "item": output.item, "message_id": output.message_id }),
    ))
}

async fn control_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.control_task(&id, input)?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn lease_tasks(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.lease_tasks(input)?))
}

async fn worker_reconcile(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.worker_reconcile(input)?))
}

async fn worker_task_control(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.worker_task_control(&id)?))
}

async fn renew_worker_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.renew_worker_task(&id, input)?))
}

async fn worker_task_log(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    store(&state)?.append_task_log(&id, input)?;
    Ok(Json(json!({ "ok": true })))
}

async fn complete_worker_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.complete_worker_task(&id, input)?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn fail_worker_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.fail_worker_task(&id, input)?;
    Ok(Json(json!({ "ok": true, "item": item })))
}
