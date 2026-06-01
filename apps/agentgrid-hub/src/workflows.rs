use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{store, ApiError, AppState};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/workflow-templates",
            get(list_workflow_templates).post(create_workflow_template),
        )
        .route(
            "/api/workflow-templates/{id}/start",
            axum::routing::post(start_workflow_template),
        )
        .route("/api/workflows", get(list_workflows).post(create_workflow))
        .route("/api/workflows/{id}", get(get_workflow))
        .route(
            "/api/workflows/{id}/start",
            axum::routing::post(start_workflow),
        )
        .route(
            "/api/workflows/{id}/cancel",
            axum::routing::post(cancel_workflow),
        )
}

#[derive(Debug, Deserialize)]
pub(crate) struct WorkflowQuery {
    pub(crate) limit: Option<u16>,
    pub(crate) state: Option<String>,
}

async fn list_workflow_templates(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "items": store(&state)?.list_workflow_templates(100)?
    })))
}

async fn create_workflow_template(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let item = store(&state)?.create_workflow_template(input)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "ok": true, "item": item })),
    ))
}

async fn start_workflow_template(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.start_workflow_template(&id, input)?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn list_workflows(
    State(state): State<AppState>,
    Query(query): Query<WorkflowQuery>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "items": store(&state)?.list_workflows(query)?,
        "next_cursor": null
    })))
}

async fn create_workflow(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let item = store(&state)?.create_workflow(input)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "ok": true, "item": item })),
    ))
}

async fn get_workflow(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?
        .get_workflow_detail(&id)?
        .ok_or_else(|| ApiError::not_found("Workflow not found"))?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn start_workflow(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.start_workflow(&id, input)?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn cancel_workflow(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.cancel_workflow(&id, input)?;
    Ok(Json(json!({ "ok": true, "item": item })))
}
