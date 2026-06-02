use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderName, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use base64::Engine as _;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    bearer_token_from_headers, now, sanitize_worker_target, sha256_hex, store, worker_binary_path,
    worker_target_from_query, worker_update_compatibility, ApiError, AppState, Store,
    AGENTGRID_BUILD_VERSION,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/worker/update-manifest", get(worker_update_manifest))
        .route("/api/worker/download/{target}", get(download_worker_binary))
        .route("/api/nodes", get(list_nodes).post(upsert_node))
        .route("/api/nodes/{id}", delete(delete_node))
        .route("/api/nodes/{id}/config", post(update_node_config))
        .route("/api/nodes/{id}/approve", post(approve_node_join))
        .route(
            "/api/nodes/{id}/tools",
            get(list_node_tools).post(register_node_tools),
        )
        .route("/api/node-tools", get(list_node_tools_catalog))
        .route("/api/node-tools/probe", post(probe_node_tools))
        .route("/api/node-tools/{tool_id}", get(get_node_tool_catalog))
        .route("/api/node-tools/{tool_id}/probe", post(probe_node_tool))
        .route(
            "/api/node-tools/{tool_id}/nodes/{node_id}/probe",
            post(probe_node_tool_node),
        )
        .route(
            "/api/node-provisioning/plans",
            get(list_node_provisioning_plans).post(create_node_provisioning_plan),
        )
}
#[derive(Debug, Deserialize)]
struct UpdateManifestQuery {
    os: Option<String>,
    arch: Option<String>,
    current_sha256: Option<String>,
    glibc_version: Option<String>,
    worker_target: Option<String>,
    node_id: Option<String>,
    channel: Option<String>,
}

async fn worker_update_manifest(
    State(state): State<AppState>,
    Query(query): Query<UpdateManifestQuery>,
) -> Result<Json<Value>, ApiError> {
    let target = query
        .worker_target
        .as_deref()
        .map(sanitize_worker_target)
        .transpose()?
        .unwrap_or_else(|| worker_target_from_query(query.os.as_deref(), query.arch.as_deref()));
    let path = worker_binary_path(&state, &target);
    let bytes = tokio::fs::read(&path).await.map_err(|error| {
        ApiError::not_found(&format!(
            "Worker binary not published for target {target}: {error}"
        ))
    })?;
    let sha256 = sha256_hex(&bytes);
    let signing = worker_update_signing_metadata(&path, &bytes, &sha256)?;
    let compatibility = worker_update_compatibility(&target, query.glibc_version.as_deref());
    let current_matches = query.current_sha256.as_deref() == Some(sha256.as_str());
    let update_available =
        !current_matches && compatibility.get("compatible").and_then(Value::as_bool) == Some(true);
    let _ = Store::open(state.db_path.as_ref()).and_then(|store| {
        store.audit(
            "worker.update.checked",
            query.node_id.as_deref().unwrap_or("worker"),
            query.node_id.as_deref(),
            if update_available {
                "Worker 有可用更新"
            } else {
                "Worker 更新检查完成"
            },
            json!({
                "node_id": query.node_id,
                "channel": query.channel.as_deref().unwrap_or("stable"),
                "target": target,
                "current_sha256": query.current_sha256,
                "published_sha256": sha256,
                "signing": signing,
                "compatible": compatibility.get("compatible").and_then(Value::as_bool).unwrap_or(true),
                "update_available": update_available,
                "compatibility": compatibility
            }),
        )
    });
    Ok(Json(json!({
        "ok": true,
        "service": "agentgrid-worker-update",
        "version": AGENTGRID_BUILD_VERSION,
        "target": target,
        "sha256": sha256,
        "size_bytes": bytes.len(),
        "download_url": format!("/api/worker/download/{target}"),
        "signature": signing.get("signature").cloned().unwrap_or(Value::Null),
        "signature_algorithm": signing.get("algorithm").cloned().unwrap_or_else(|| json!("none")),
        "signing_key_id": signing.get("key_id").cloned().unwrap_or(Value::Null),
        "signing_public_key": signing.get("public_key").cloned().unwrap_or(Value::Null),
        "signature_required": signing.get("required").and_then(Value::as_bool).unwrap_or(false),
        "signing": signing,
        "update_available": update_available,
        "compatible": compatibility.get("compatible").and_then(Value::as_bool).unwrap_or(true),
        "compatibility": compatibility,
        "published_at": now()
    })))
}

fn worker_update_signing_metadata(
    binary_path: &std::path::Path,
    bytes: &[u8],
    sha256: &str,
) -> Result<Value, ApiError> {
    let signature_path = binary_path.with_file_name(format!(
        "{}.ed25519.sig",
        binary_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("agentgrid-worker")
    ));
    let signature = std::fs::read_to_string(&signature_path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let public_key = std::env::var("AGENTGRID_WORKER_UPDATE_PUBLIC_KEY")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let key_id = std::env::var("AGENTGRID_WORKER_UPDATE_KEY_ID")
        .unwrap_or_else(|_| "agentgrid-worker-update-v1".to_string());
    let required = std::env::var("AGENTGRID_WORKER_UPDATE_SIGNATURE_REQUIRED")
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false);
    if required && signature.is_none() {
        return Err(ApiError::bad_request(
            "worker update signature is required but the .ed25519.sig file is missing",
        ));
    }
    if required && public_key.is_none() {
        return Err(ApiError::bad_request(
            "worker update signature is required but AGENTGRID_WORKER_UPDATE_PUBLIC_KEY is missing",
        ));
    }
    let verified = match (signature.as_deref(), public_key.as_deref()) {
        (Some(signature), Some(public_key)) => {
            verify_worker_update_signature(public_key, bytes, signature).map_err(|error| {
                ApiError::bad_request(&format!("worker update signature is invalid: {error}"))
            })?
        }
        _ => false,
    };
    Ok(json!({
        "algorithm": if signature.is_some() { "ed25519" } else { "none" },
        "key_id": if signature.is_some() { Value::String(key_id) } else { Value::Null },
        "public_key": public_key,
        "signature": signature,
        "required": required,
        "verified_by_hub": verified,
        "signed_payload": "raw worker update bytes",
        "sha256": sha256,
        "size_bytes": bytes.len()
    }))
}

fn verify_worker_update_signature(
    public_key: &str,
    bytes: &[u8],
    signature_b64: &str,
) -> anyhow::Result<bool> {
    let public_key_bytes = base64::engine::general_purpose::STANDARD
        .decode(public_key.trim())
        .map_err(|error| anyhow::anyhow!("public key must be base64 ed25519 bytes: {error}"))?;
    let key_bytes: [u8; 32] = public_key_bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("ed25519 public key must be 32 bytes"))?;
    let signature_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature_b64.trim())
        .map_err(|error| anyhow::anyhow!("signature must be base64: {error}"))?;
    let signature_array: [u8; 64] = signature_bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("ed25519 signature must be 64 bytes"))?;
    let verifying_key = VerifyingKey::from_bytes(&key_bytes)?;
    let signature = Signature::from_bytes(&signature_array);
    verifying_key.verify(bytes, &signature)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn with_update_signing_env<R>(
        required: Option<&str>,
        public_key: Option<&str>,
        test: impl FnOnce() -> R,
    ) -> R {
        let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let old_required = std::env::var("AGENTGRID_WORKER_UPDATE_SIGNATURE_REQUIRED").ok();
        let old_public_key = std::env::var("AGENTGRID_WORKER_UPDATE_PUBLIC_KEY").ok();
        let old_key_id = std::env::var("AGENTGRID_WORKER_UPDATE_KEY_ID").ok();

        match required {
            Some(value) => std::env::set_var("AGENTGRID_WORKER_UPDATE_SIGNATURE_REQUIRED", value),
            None => std::env::remove_var("AGENTGRID_WORKER_UPDATE_SIGNATURE_REQUIRED"),
        }
        match public_key {
            Some(value) => std::env::set_var("AGENTGRID_WORKER_UPDATE_PUBLIC_KEY", value),
            None => std::env::remove_var("AGENTGRID_WORKER_UPDATE_PUBLIC_KEY"),
        }
        std::env::remove_var("AGENTGRID_WORKER_UPDATE_KEY_ID");

        let result = test();

        match old_required {
            Some(value) => std::env::set_var("AGENTGRID_WORKER_UPDATE_SIGNATURE_REQUIRED", value),
            None => std::env::remove_var("AGENTGRID_WORKER_UPDATE_SIGNATURE_REQUIRED"),
        }
        match old_public_key {
            Some(value) => std::env::set_var("AGENTGRID_WORKER_UPDATE_PUBLIC_KEY", value),
            None => std::env::remove_var("AGENTGRID_WORKER_UPDATE_PUBLIC_KEY"),
        }
        match old_key_id {
            Some(value) => std::env::set_var("AGENTGRID_WORKER_UPDATE_KEY_ID", value),
            None => std::env::remove_var("AGENTGRID_WORKER_UPDATE_KEY_ID"),
        }

        result
    }

    #[test]
    fn required_worker_update_signature_fails_when_signature_file_missing() {
        let temp = tempfile::tempdir().unwrap();
        let binary_path = temp.path().join("agentgrid-worker");
        let result = with_update_signing_env(
            Some("true"),
            Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="),
            || worker_update_signing_metadata(&binary_path, b"worker", "sha"),
        );

        let error = result.unwrap_err();
        assert!(error.message.contains(".ed25519.sig file is missing"));
    }

    #[test]
    fn required_worker_update_signature_fails_when_public_key_missing() {
        let temp = tempfile::tempdir().unwrap();
        let binary_path = temp.path().join("agentgrid-worker");
        std::fs::write(
            temp.path().join("agentgrid-worker.ed25519.sig"),
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==",
        )
        .unwrap();

        let result = with_update_signing_env(Some("true"), None, || {
            worker_update_signing_metadata(&binary_path, b"worker", "sha")
        });

        let error = result.unwrap_err();
        assert!(error
            .message
            .contains("AGENTGRID_WORKER_UPDATE_PUBLIC_KEY is missing"));
    }
}

async fn download_worker_binary(
    State(state): State<AppState>,
    Path(target): Path<String>,
) -> Result<Response, ApiError> {
    let target = sanitize_worker_target(&target)?;
    let path = worker_binary_path(&state, &target);
    let bytes = tokio::fs::read(&path).await.map_err(|error| {
        ApiError::not_found(&format!(
            "Worker binary not published for target {target}: {error}"
        ))
    })?;
    let filename = if target.contains("windows") {
        format!("agentgrid-worker-{target}.exe")
    } else {
        format!("agentgrid-worker-{target}")
    };
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
            (
                HeaderName::from_static("x-agentgrid-sha256"),
                sha256_hex(&bytes),
            ),
        ],
        bytes,
    )
        .into_response())
}

async fn list_nodes(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({ "ok": true, "items": store(&state)?.list_nodes()? }),
    ))
}

async fn upsert_node(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let item = store(&state)?.upsert_node(input)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "ok": true, "item": item })),
    ))
}

async fn delete_node(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    store.require_admin_session(bearer_token_from_headers(&headers).as_deref())?;
    store.delete_node(&id)?;
    Ok(Json(json!({ "ok": true, "deleted": id })))
}

async fn update_node_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    store.require_admin_session(bearer_token_from_headers(&headers).as_deref())?;
    let item = store.update_node_config(&id, input)?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn approve_node_join(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    let user = store.require_admin_session(bearer_token_from_headers(&headers).as_deref())?;
    let actor = user
        .pointer("/spec/email")
        .and_then(Value::as_str)
        .or_else(|| input.get("actor").and_then(Value::as_str))
        .unwrap_or("super-admin");
    let item = store.approve_node_join(&id, actor)?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn register_node_tools(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    store.require_admin_session(bearer_token_from_headers(&headers).as_deref())?;
    let items = store.register_node_tools(&id, input)?;
    Ok(Json(json!({ "ok": true, "items": items })))
}

async fn list_node_tools(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let items = store(&state)?.list_node_tools(Some(&id))?;
    Ok(Json(json!({ "ok": true, "node_id": id, "items": items })))
}

async fn list_node_tools_catalog(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let items = store(&state)?.list_node_tool_catalog()?;
    Ok(Json(json!({
        "ok": true,
        "api_version": "agentgrid.runtime/v1",
        "kind": "NodeToolCatalog",
        "items": items
    })))
}

async fn get_node_tool_catalog(
    State(state): State<AppState>,
    Path(tool_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?
        .get_node_tool_catalog(&tool_id)?
        .ok_or_else(|| ApiError::not_found("Node tool not found"))?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn probe_node_tools(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    store.require_admin_session(bearer_token_from_headers(&headers).as_deref())?;
    let items = store.create_node_tool_probe_tasks(None, None, "manual")?;
    Ok(Json(json!({ "ok": true, "items": items })))
}

async fn probe_node_tool(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(tool_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    store.require_admin_session(bearer_token_from_headers(&headers).as_deref())?;
    let items = store.create_node_tool_probe_tasks(Some(&tool_id), None, "manual")?;
    Ok(Json(json!({ "ok": true, "items": items })))
}

async fn probe_node_tool_node(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((tool_id, node_id)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    store.require_admin_session(bearer_token_from_headers(&headers).as_deref())?;
    let items = store.create_node_tool_probe_tasks(Some(&tool_id), Some(&node_id), "manual")?;
    Ok(Json(json!({ "ok": true, "items": items })))
}

async fn list_node_provisioning_plans(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    store.require_admin_session(bearer_token_from_headers(&headers).as_deref())?;
    Ok(Json(json!({
        "ok": true,
        "items": store.list_node_provisioning_plans(100)?
    })))
}

async fn create_node_provisioning_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let store = store(&state)?;
    store.require_admin_session(bearer_token_from_headers(&headers).as_deref())?;
    let item = store.create_node_provisioning_plan(input)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "ok": true, "item": item })),
    ))
}
