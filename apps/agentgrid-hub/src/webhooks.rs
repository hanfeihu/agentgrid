use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get},
    Json, Router,
};
use serde_json::{json, Value};

use crate::{store, ApiError, AppState};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/webhooks", get(list_webhooks).post(create_webhook))
        .route("/api/webhooks/deliveries", get(list_webhook_deliveries))
        .route("/api/webhooks/{id}", delete(delete_webhook))
}

async fn list_webhooks(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "items": store(&state)?.list_webhooks(200)?
    })))
}

async fn create_webhook(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let item = store(&state)?.create_webhook(input)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "ok": true, "item": item })),
    ))
}

async fn delete_webhook(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    store(&state)?.delete_webhook(&id)?;
    Ok(Json(json!({ "ok": true })))
}

async fn list_webhook_deliveries(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "items": store(&state)?.list_webhook_deliveries(200)?
    })))
}
