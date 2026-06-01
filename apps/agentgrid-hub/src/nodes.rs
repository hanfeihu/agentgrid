use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderName, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    now, sanitize_worker_target, sha256_hex, store, worker_binary_path, worker_target_from_query,
    worker_update_compatibility, ApiError, AppState, Store, AGENTGRID_BUILD_VERSION,
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
        "update_available": update_available,
        "compatible": compatibility.get("compatible").and_then(Value::as_bool).unwrap_or(true),
        "compatibility": compatibility,
        "published_at": now()
    })))
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
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    store(&state)?.delete_node(&id)?;
    Ok(Json(json!({ "ok": true, "deleted": id })))
}

async fn update_node_config(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.update_node_config(&id, input)?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn approve_node_join(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let actor = input
        .get("actor")
        .and_then(Value::as_str)
        .unwrap_or("super-admin");
    let item = store(&state)?.approve_node_join(&id, actor)?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn register_node_tools(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let items = store(&state)?.register_node_tools(&id, input)?;
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

async fn probe_node_tools(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let items = store(&state)?.create_node_tool_probe_tasks(None, None, "manual")?;
    Ok(Json(json!({ "ok": true, "items": items })))
}

async fn probe_node_tool(
    State(state): State<AppState>,
    Path(tool_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let items = store(&state)?.create_node_tool_probe_tasks(Some(&tool_id), None, "manual")?;
    Ok(Json(json!({ "ok": true, "items": items })))
}

async fn probe_node_tool_node(
    State(state): State<AppState>,
    Path((tool_id, node_id)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    let items =
        store(&state)?.create_node_tool_probe_tasks(Some(&tool_id), Some(&node_id), "manual")?;
    Ok(Json(json!({ "ok": true, "items": items })))
}

async fn list_node_provisioning_plans(
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "items": store(&state)?.list_node_provisioning_plans(100)?
    })))
}

async fn create_node_provisioning_plan(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let item = store(&state)?.create_node_provisioning_plan(input)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "ok": true, "item": item })),
    ))
}
