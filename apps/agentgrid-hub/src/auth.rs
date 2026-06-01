use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use serde_json::Value;

use crate::{bearer_token_from_headers, store, ApiError, AppState};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/bootstrap", get(get_bootstrap_status))
        .route("/api/bootstrap/admin", post(create_super_admin))
        .route("/api/auth/me", get(auth_me))
        .route("/api/auth/login", post(login_user))
        .route(
            "/api/auth/register/request-code",
            post(request_register_code),
        )
        .route("/api/auth/register", post(register_user))
        .route("/api/auth/change-password", post(change_password))
        .route("/api/users", get(list_users))
        .route("/api/users/{id}", post(update_user))
}

async fn get_bootstrap_status(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.bootstrap_status()?))
}

async fn create_super_admin(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.create_super_admin(input)?))
}

async fn auth_me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.auth_state(
        bearer_token_from_headers(&headers).as_deref(),
    )?))
}

async fn login_user(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.login_user(input)?))
}

async fn request_register_code(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.request_register_code(input)?))
}

async fn register_user(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.register_user(input)?))
}

async fn change_password(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.change_password(input)?))
}

async fn list_users(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.list_users()?))
}

async fn update_user(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.update_user(&id, input)?))
}
