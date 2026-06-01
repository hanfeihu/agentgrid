use axum::{extract::State, routing::get, Json, Router};
use serde_json::Value;

use crate::{store, ApiError, AppState};

pub(crate) fn router() -> Router<AppState> {
    Router::new().route(
        "/api/settings",
        get(get_system_settings).post(update_system_settings),
    )
}

async fn get_system_settings(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.system_settings()?))
}

async fn update_system_settings(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.update_system_settings(input)?))
}
