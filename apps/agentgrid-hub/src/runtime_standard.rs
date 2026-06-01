use axum::{extract::State, routing::get, Json, Router};
use serde_json::{json, Value};

use crate::{
    artifact_store_standard, capability_graph_standard, capability_standard, device_standard,
    event_timeline_standard, evidence_pipeline_standard, evidence_standard,
    execution_contract_standard, mobile_sdk_standard, placement_engine_standard,
    plugin_runtime_standard, probe_engine_standard, result_report_standard, runbook_standard,
    runtime_standard_document, store, task_intent_standard, task_state_machine_standard,
    workbench_standard, workflow_template_standard, ApiError, AppState,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/policy", get(get_policy))
        .route("/api/runtime-standard", get(runtime_standard))
        .route(
            "/api/runtime-standard/tool-contracts",
            get(runtime_standard_tool_contracts),
        )
        .route(
            "/api/runtime-standard/capabilities",
            get(runtime_standard_capabilities),
        )
        .route(
            "/api/runtime-standard/state-machine",
            get(runtime_standard_state_machine),
        )
        .route(
            "/api/runtime-standard/workflow-template",
            get(runtime_standard_workflow_template),
        )
        .route(
            "/api/runtime-standard/result-report",
            get(runtime_standard_result_report),
        )
        .route(
            "/api/runtime-standard/workbench",
            get(runtime_standard_workbench),
        )
        .route(
            "/api/runtime-standard/devices",
            get(runtime_standard_devices),
        )
        .route(
            "/api/runtime-standard/evidence",
            get(runtime_standard_evidence),
        )
        .route(
            "/api/runtime-standard/runbook",
            get(runtime_standard_runbook),
        )
        .route(
            "/api/runtime-standard/mobile-sdk",
            get(runtime_standard_mobile_sdk),
        )
        .route(
            "/api/runtime-standard/plugin-runtime",
            get(runtime_standard_plugin_runtime),
        )
        .route(
            "/api/runtime-standard/capability-graph",
            get(runtime_standard_capability_graph),
        )
        .route(
            "/api/runtime-standard/execution-contract",
            get(runtime_standard_execution_contract),
        )
        .route(
            "/api/runtime-standard/evidence-pipeline",
            get(runtime_standard_evidence_pipeline),
        )
        .route(
            "/api/runtime-standard/probe-engine",
            get(runtime_standard_probe_engine),
        )
        .route(
            "/api/runtime-standard/placement-engine",
            get(runtime_standard_placement_engine),
        )
        .route(
            "/api/runtime-standard/task-intent",
            get(runtime_standard_task_intent),
        )
        .route(
            "/api/runtime-standard/artifact-store",
            get(runtime_standard_artifact_store),
        )
        .route(
            "/api/runtime-standard/event-timeline",
            get(runtime_standard_event_timeline),
        )
        .route("/api/capabilities/manifest", get(capabilities_manifest))
        .route(
            "/api/scheduler-config",
            get(get_scheduler_config).post(update_scheduler_config),
        )
}

async fn capabilities_manifest(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    Ok(Json(store.capabilities_manifest()?))
}

async fn get_policy(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "policy": store(&state)?.security_policy()?
    })))
}

async fn runtime_standard(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    Ok(Json(json!({
        "ok": true,
        "item": runtime_standard_document(&store)?
    })))
}

async fn runtime_standard_tool_contracts(
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    let nodes = store.list_nodes()?;
    let items = store
        .tool_registry_with_dynamic()?
        .into_iter()
        .map(|tool| store.enrich_tool_with_nodes(tool, &nodes))
        .collect::<anyhow::Result<Vec<_>>>()?
        .into_iter()
        .map(|tool| tool.get("tool_contract").cloned().unwrap_or(tool))
        .collect::<Vec<_>>();
    Ok(Json(json!({
        "ok": true,
        "api_version": "agentgrid.runtime/v1",
        "kind": "ToolContractStandard",
        "items": items
    })))
}

async fn runtime_standard_capabilities(
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    Ok(Json(json!({
        "ok": true,
        "item": capability_standard(&store)?
    })))
}

async fn runtime_standard_state_machine() -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "item": task_state_machine_standard()
    })))
}

async fn runtime_standard_workflow_template(
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    Ok(Json(json!({
        "ok": true,
        "item": workflow_template_standard(&store)?
    })))
}

async fn runtime_standard_result_report(
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    Ok(Json(json!({
        "ok": true,
        "item": result_report_standard(&store)?
    })))
}

async fn runtime_standard_workbench(
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    Ok(Json(json!({
        "ok": true,
        "item": workbench_standard(&store)?
    })))
}

async fn runtime_standard_devices(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    Ok(Json(json!({
        "ok": true,
        "item": device_standard(&store)?
    })))
}

async fn runtime_standard_evidence(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    Ok(Json(json!({
        "ok": true,
        "item": evidence_standard(&store)?
    })))
}

async fn runtime_standard_runbook(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    Ok(Json(json!({
        "ok": true,
        "item": runbook_standard(&store)?
    })))
}

async fn runtime_standard_mobile_sdk() -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "item": mobile_sdk_standard()
    })))
}

async fn runtime_standard_plugin_runtime(
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    Ok(Json(json!({
        "ok": true,
        "item": plugin_runtime_standard(&store)?
    })))
}

async fn runtime_standard_capability_graph(
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    Ok(Json(json!({
        "ok": true,
        "item": capability_graph_standard(&store)?
    })))
}

async fn runtime_standard_execution_contract(
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    Ok(Json(json!({
        "ok": true,
        "item": execution_contract_standard(&store)?
    })))
}

async fn runtime_standard_evidence_pipeline(
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    Ok(Json(json!({
        "ok": true,
        "item": evidence_pipeline_standard(&store)?
    })))
}

async fn runtime_standard_probe_engine(
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    Ok(Json(json!({
        "ok": true,
        "item": probe_engine_standard(&store)?
    })))
}

async fn runtime_standard_placement_engine(
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    Ok(Json(json!({
        "ok": true,
        "item": placement_engine_standard(&store)?
    })))
}

async fn runtime_standard_task_intent() -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "item": task_intent_standard()
    })))
}

async fn runtime_standard_artifact_store(
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    Ok(Json(json!({
        "ok": true,
        "item": artifact_store_standard(&store)?
    })))
}

async fn runtime_standard_event_timeline(
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    Ok(Json(json!({
        "ok": true,
        "item": event_timeline_standard(&store)?
    })))
}

async fn get_scheduler_config(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "config": store(&state)?.scheduler_config()?
    })))
}

async fn update_scheduler_config(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let config = store(&state)?.update_scheduler_config(input)?;
    Ok(Json(json!({ "ok": true, "config": config })))
}
