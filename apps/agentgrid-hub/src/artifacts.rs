use axum::{
    extract::{Path, State},
    http::header,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use base64::Engine as _;
use serde_json::{json, Value};

use crate::{store, ApiError, AppState};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/artifacts", get(list_artifacts))
        .route("/api/artifacts/{id}/download", get(download_artifact))
}

async fn list_artifacts(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "items": store(&state)?.list_artifacts(300)?
    })))
}

async fn download_artifact(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let artifact = store(&state)?
        .get_artifact(&id)?
        .ok_or_else(|| ApiError::not_found("Artifact not found"))?;
    let content = artifact
        .pointer("/spec/content_base64")
        .and_then(Value::as_str)
        .unwrap_or("");
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(content)
        .map_err(|error| {
            ApiError::bad_request(&format!("artifact content decode failed: {error}"))
        })?;
    let filename = artifact
        .pointer("/spec/name")
        .and_then(Value::as_str)
        .unwrap_or("artifact.bin")
        .replace('"', "");
    let content_type = artifact
        .pointer("/spec/content_type")
        .and_then(Value::as_str)
        .unwrap_or("application/octet-stream")
        .to_string();
    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        bytes,
    )
        .into_response())
}
