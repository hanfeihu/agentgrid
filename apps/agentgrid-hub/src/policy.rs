use axum::{extract::State, http::HeaderMap, routing::get, Json, Router};
use serde_json::{json, Value};

use crate::{bearer_token_from_headers, store, ApiError, AppState};

pub(crate) fn router() -> Router<AppState> {
    Router::new().route(
        "/api/admin/policy",
        get(get_admin_policy).put(update_admin_policy),
    )
}

/// 管理面读取当前生效的安全策略（含 version / updated_by / updated_at）。
async fn get_admin_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    store.require_admin_session(bearer_token_from_headers(&headers).as_deref())?;
    Ok(Json(json!({ "ok": true, "policy": store.security_policy()? })))
}

/// 管理面替换安全策略（完整文档）：鉴权 → 校验 → 落库 → 审计 → 返回生效策略。
/// worker 会在下一次心跳（约 10s）自动拉取新策略生效。
async fn update_admin_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    let user = store.require_admin_session(bearer_token_from_headers(&headers).as_deref())?;
    let actor = user
        .pointer("/spec/email")
        .and_then(Value::as_str)
        .unwrap_or("admin")
        .to_string();
    let policy = store.update_security_policy(input, &actor)?;
    Ok(Json(json!({ "ok": true, "policy": policy })))
}
