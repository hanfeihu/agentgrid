use axum::{
    routing::{delete, get, post},
    Router,
};

use crate::{
    approve_node_join, create_node_provisioning_plan, delete_node, download_worker_binary,
    get_node_tool_catalog, list_node_provisioning_plans, list_node_tools, list_node_tools_catalog,
    list_nodes, probe_node_tool, probe_node_tool_node, probe_node_tools, register_node_tools,
    update_node_config, upsert_node, worker_update_manifest, AppState,
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
