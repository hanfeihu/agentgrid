use axum::{
    routing::{delete, get, post},
    Router,
};

use crate::{
    artifacts, auth, cancel_workflow, create_message, create_webhook, create_workflow,
    create_workflow_template, delete_webhook, event_stream, get_diagnostics, get_tool,
    get_workflow, health, home, jobs, list_agents, list_audit_events, list_events, list_messages,
    list_tool_nodes, list_tool_probes, list_tools, list_webhook_deliveries, list_webhooks,
    list_workflow_templates, list_workflows, nodes, probe_all_tools, probe_tool, probe_tool_node,
    runtime_standard, settings, start_workflow, start_workflow_template, static_asset,
    task_execution_record, tasks, terminal_client_ws, terminal_worker_ws, upsert_agent,
    windows_install_script, workflow_execution_record, AppState,
};

pub(crate) fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(home))
        .route("/install/windows.ps1", get(windows_install_script))
        .route("/api/health", get(health))
        .merge(auth::router())
        .merge(settings::router())
        .merge(nodes::router())
        .merge(tasks::router())
        .merge(jobs::router())
        .merge(artifacts::router())
        .merge(runtime_standard::router())
        .route("/api/agents", get(list_agents).post(upsert_agent))
        .route("/api/messages", get(list_messages).post(create_message))
        .route("/api/events", get(list_events))
        .route("/api/events/stream", get(event_stream))
        .route("/api/audit-events", get(list_audit_events))
        .route("/api/webhooks", get(list_webhooks).post(create_webhook))
        .route("/api/webhooks/deliveries", get(list_webhook_deliveries))
        .route("/api/webhooks/{id}", delete(delete_webhook))
        .route("/api/tools", get(list_tools))
        .route("/api/tools/probes", get(list_tool_probes))
        .route("/api/tools/probe", post(probe_all_tools))
        .route("/api/tools/{id}", get(get_tool))
        .route("/api/tools/{id}/nodes", get(list_tool_nodes))
        .route("/api/tools/{id}/probe", post(probe_tool))
        .route(
            "/api/tools/{id}/nodes/{node_id}/probe",
            post(probe_tool_node),
        )
        .route("/api/diagnostics", get(get_diagnostics))
        .route(
            "/api/execution-records/tasks/{id}",
            get(task_execution_record),
        )
        .route(
            "/api/execution-records/workflows/{id}",
            get(workflow_execution_record),
        )
        .route("/api/terminal/ws", get(terminal_client_ws))
        .route(
            "/api/workflow-templates",
            get(list_workflow_templates).post(create_workflow_template),
        )
        .route(
            "/api/workflow-templates/{id}/start",
            post(start_workflow_template),
        )
        .route("/api/workflows", get(list_workflows).post(create_workflow))
        .route("/api/workflows/{id}", get(get_workflow))
        .route("/api/workflows/{id}/start", post(start_workflow))
        .route("/api/workflows/{id}/cancel", post(cancel_workflow))
        .route("/api/worker/terminal/ws", get(terminal_worker_ws))
        .fallback(static_asset)
        .with_state(state)
}
