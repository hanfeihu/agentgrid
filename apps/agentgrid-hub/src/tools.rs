use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};

use crate::{store, ApiError, AppState, API_VERSION};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/tools", get(list_tools))
        .route("/api/tools/probes", get(list_tool_probes))
        .route("/api/tools/probe-center", get(get_probe_center))
        .route("/api/tools/remediation-center", get(get_remediation_center))
        .route(
            "/api/tools/remediations/{id}/runbook",
            get(get_remediation_runbook),
        )
        .route(
            "/api/tools/remediations/{id}/actions",
            post(run_remediation_action),
        )
        .route("/api/tools/probe", post(probe_all_tools))
        .route("/api/tools/{id}", get(get_tool))
        .route("/api/tools/{id}/nodes", get(list_tool_nodes))
        .route("/api/tools/{id}/probe", post(probe_tool))
        .route(
            "/api/tools/{id}/nodes/{node_id}/probe",
            post(probe_tool_node),
        )
}

async fn get_probe_center(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    Ok(Json(json!({
        "ok": true,
        "item": store.tool_probe_center()?
    })))
}

async fn get_remediation_center(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    Ok(Json(json!({
        "ok": true,
        "item": store.tool_remediation_center()?
    })))
}

async fn get_remediation_runbook(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.get_remediation_runbook(&id)?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn run_remediation_action(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let action = input
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("create_task");
    let actor = input
        .get("actor")
        .and_then(Value::as_str)
        .unwrap_or("remediation-center");
    let output = store(&state)?.run_remediation_action(&id, action, actor)?;
    Ok(Json(json!({ "ok": true, "item": output })))
}

async fn list_tools(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    let nodes = store.list_nodes()?;
    let items = store
        .tool_registry_with_dynamic()?
        .into_iter()
        .map(|tool| store.enrich_tool_with_nodes(tool, &nodes))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(Json(json!({
        "ok": true,
        "kind": "ToolRegistry",
        "api_version": API_VERSION,
        "items": items
    })))
}

async fn get_tool(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    let nodes = store.list_nodes()?;
    let tool = store
        .tool_registry_with_dynamic()?
        .into_iter()
        .find(|tool| tool.get("id").and_then(Value::as_str) == Some(id.as_str()))
        .ok_or_else(|| ApiError::not_found("Tool not found"))?;
    Ok(Json(json!({
        "ok": true,
        "item": store.enrich_tool_with_nodes(tool, &nodes)?
    })))
}

async fn list_tool_nodes(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    let nodes = store.list_nodes()?;
    let tool = store
        .tool_registry_with_dynamic()?
        .into_iter()
        .find(|tool| tool.get("id").and_then(Value::as_str) == Some(id.as_str()))
        .ok_or_else(|| ApiError::not_found("Tool not found"))?;
    Ok(Json(json!({
        "ok": true,
        "tool_id": id,
        "items": store.nodes_for_tool_with_probe(&tool, &nodes)?
    })))
}

async fn list_tool_probes(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "items": store(&state)?.list_tool_probes(500)?
    })))
}

async fn probe_all_tools(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let items = store(&state)?.create_tool_probe_tasks(None, None)?;
    Ok(Json(json!({ "ok": true, "items": items })))
}

async fn probe_tool(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let items = store(&state)?.create_tool_probe_tasks(Some(&id), None)?;
    Ok(Json(json!({ "ok": true, "items": items })))
}

async fn probe_tool_node(
    State(state): State<AppState>,
    Path((id, node_id)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    let items = store(&state)?.create_tool_probe_tasks(Some(&id), Some(&node_id))?;
    Ok(Json(json!({ "ok": true, "items": items })))
}
