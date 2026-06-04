use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};

use crate::{
    create_port_bridge_session, port_bridge_session_json, store, workbench_channel_node_id,
    workbench_name, ApiError, AppState,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/workbenches", get(list_workbenches))
        .route("/api/workbenches/{id}", get(get_workbench))
        .route("/api/workbenches/{id}/timeline", get(workbench_timeline))
        .route(
            "/api/workbenches/{id}/actions",
            post(create_workbench_action),
        )
}

async fn list_workbenches(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "api_version": "agentgrid.workbench/v1",
        "kind": "WorkbenchList",
        "items": store(&state)?.list_workbenches()?,
        "links": {
            "nodes": "/api/nodes",
            "tools": "/api/node-tools",
            "capability_graph": "/api/runtime-standard/capability-graph"
        }
    })))
}

async fn get_workbench(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?
        .get_workbench(&id)?
        .ok_or_else(|| ApiError::not_found("Workbench not found"))?;
    Ok(Json(json!({
        "ok": true,
        "api_version": "agentgrid.workbench/v1",
        "kind": "WorkbenchDetail",
        "item": item
    })))
}

async fn workbench_timeline(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.workbench_timeline(&id, 100)?;
    Ok(Json(json!({
        "ok": true,
        "api_version": "agentgrid.workbench-timeline/v1",
        "kind": "WorkbenchTimeline",
        "item": item
    })))
}

async fn create_workbench_action(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let action = input
        .get("action")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::bad_request("action is required"))?;
    let output = if matches!(action, "port_bridge.create" | "port_bridge") {
        create_workbench_port_bridge_action(&state, &id, input).await?
    } else {
        store(&state)?.create_workbench_action(&id, input)?
    };
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "ok": true,
            "api_version": "agentgrid.workbench-action/v1",
            "kind": "WorkbenchActionSubmission",
            "item": output
        })),
    ))
}

async fn create_workbench_port_bridge_action(
    state: &AppState,
    workbench_id: &str,
    input: Value,
) -> Result<Value, ApiError> {
    let store = store(state)?;
    let workbench = store
        .get_workbench(workbench_id)?
        .ok_or_else(|| ApiError::not_found("Workbench not found"))?;
    let payload = input.get("payload").cloned().unwrap_or_else(|| json!({}));
    let created_by = input
        .get("created_by")
        .and_then(Value::as_str)
        .unwrap_or("workbench-action-api")
        .to_string();
    let operation_id = crate::new_id("op");
    let source_role = if workbench_channel_node_id(&workbench, "bridge").is_some() {
        "bridge"
    } else {
        "worker"
    };
    let source_node_id = workbench_channel_node_id(&workbench, source_role).ok_or_else(|| {
        ApiError::bad_request("source workbench has no bridge or worker channel for port bridge")
    })?;
    let target_node_id = payload
        .get("target_node_id")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .map(Ok)
        .unwrap_or_else(|| {
            let target_workbench_id = payload
                .get("target_workbench_id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    ApiError::bad_request(
                        "payload.target_node_id or payload.target_workbench_id is required",
                    )
                })?;
            let target_workbench = store
                .get_workbench(target_workbench_id)?
                .ok_or_else(|| ApiError::not_found("Target workbench not found"))?;
            workbench_channel_node_id(&target_workbench, "bridge")
                .or_else(|| workbench_channel_node_id(&target_workbench, "worker"))
                .ok_or_else(|| {
                    ApiError::bad_request(
                        "target workbench has no bridge or worker channel for port bridge",
                    )
                })
        })?;
    let target_port = payload
        .get("target_port")
        .and_then(Value::as_u64)
        .ok_or_else(|| ApiError::bad_request("payload.target_port is required"))?;
    let mut request = json!({
        "source_node_id": source_node_id,
        "target_node_id": target_node_id,
        "source_bind_host": payload
            .get("source_bind_host")
            .and_then(Value::as_str)
            .unwrap_or("127.0.0.1"),
        "source_bind_port": payload
            .get("source_bind_port")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        "target_host": payload
            .get("target_host")
            .and_then(Value::as_str)
            .unwrap_or("127.0.0.1"),
        "target_port": target_port,
        "protocol": payload
            .get("protocol")
            .and_then(Value::as_str)
            .unwrap_or("tcp"),
        "ttl_seconds": payload
            .get("ttl_seconds")
            .and_then(Value::as_u64)
            .unwrap_or(1800),
        "purpose": payload
            .get("purpose")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("{} 端口桥接", workbench_name(&workbench))),
        "created_by": created_by
    });
    if let Some(value) = payload.get("metadata").cloned() {
        request["metadata"] = value;
    }
    drop(store);
    let session = create_port_bridge_session(state, request, created_by).await?;
    let session_json = port_bridge_session_json(session);
    let bridge_id = session_json
        .pointer("/metadata/id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let source_node_id = session_json
        .pointer("/spec/source_node_id")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    Ok(json!({
        "api_version": "agentgrid.workbench-action/v1",
        "kind": "WorkbenchAction",
        "operation_id": operation_id,
        "workbench_id": workbench_id,
        "action": "port_bridge.create",
        "selected_channel": {
            "role": source_role,
            "node_id": source_node_id
        },
        "routing_reason": "端口桥接动作使用 source workbench 的 bridge/worker 通道建立本地端口转发。",
        "state": session_json.pointer("/status/state").cloned().unwrap_or(Value::Null),
        "port_bridge_id": bridge_id,
        "port_bridge": session_json,
        "task_id": Value::Null,
        "message_id": Value::Null,
        "artifacts": [],
        "timeline": {
            "workbench": format!("/api/workbenches/{workbench_id}/timeline"),
            "port_bridge": format!("/api/port-bridges/{bridge_id}")
        },
        "links": {
            "workbench": format!("/api/workbenches/{workbench_id}"),
            "port_bridge": format!("/api/port-bridges/{bridge_id}")
        }
    }))
}
