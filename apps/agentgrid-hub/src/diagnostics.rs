use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};

use crate::{store, ApiError, AppState};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/diagnostics", get(get_diagnostics))
        .route(
            "/api/execution-records/tasks/{id}",
            get(task_execution_record),
        )
        .route(
            "/api/execution-records/workflows/{id}",
            get(workflow_execution_record),
        )
}

async fn get_diagnostics(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "diagnostics": store(&state)?.diagnostics()?
    })))
}

async fn task_execution_record(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.task_execution_record(&id)?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn workflow_execution_record(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.workflow_execution_record(&id)?;
    Ok(Json(json!({ "ok": true, "item": item })))
}
