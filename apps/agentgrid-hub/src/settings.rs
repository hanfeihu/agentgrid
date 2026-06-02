use axum::{extract::State, http::HeaderMap, routing::get, Json, Router};
use serde_json::Value;

use crate::{bearer_token_from_headers, store, ApiError, AppState};

pub(crate) fn router() -> Router<AppState> {
    Router::new().route(
        "/api/settings",
        get(get_system_settings).post(update_system_settings),
    )
}

async fn get_system_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    store.require_admin_session(bearer_token_from_headers(&headers).as_deref())?;
    Ok(Json(store.system_settings()?))
}

async fn update_system_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    store.require_admin_session(bearer_token_from_headers(&headers).as_deref())?;
    Ok(Json(store.update_system_settings(input)?))
}
