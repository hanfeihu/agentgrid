use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use serde_json::{json, Value};

use crate::{store, ApiError, AppState};

pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/api/agents", get(list_agents).post(upsert_agent))
}

async fn list_agents(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({ "items": store(&state)?.list_agents()? })))
}

async fn upsert_agent(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let item = store(&state)?.upsert_agent(input)?;
    Ok((StatusCode::CREATED, Json(item)))
}
