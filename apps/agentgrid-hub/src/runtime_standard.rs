use axum::{routing::get, Router};

use crate::{
    capabilities_manifest, get_policy, get_scheduler_config, runtime_standard,
    runtime_standard_artifact_store, runtime_standard_capabilities,
    runtime_standard_capability_graph, runtime_standard_devices, runtime_standard_event_timeline,
    runtime_standard_evidence, runtime_standard_evidence_pipeline,
    runtime_standard_execution_contract, runtime_standard_mobile_sdk,
    runtime_standard_placement_engine, runtime_standard_plugin_runtime,
    runtime_standard_probe_engine, runtime_standard_result_report, runtime_standard_runbook,
    runtime_standard_state_machine, runtime_standard_task_intent, runtime_standard_tool_contracts,
    runtime_standard_workbench, runtime_standard_workflow_template, update_scheduler_config,
    AppState,
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
