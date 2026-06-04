use axum::{routing::get, Router};

use crate::{
    agents, artifacts, auth, bridge_client_ws, bridge_worker_ws, close_port_bridge,
    create_bridge_session, create_port_bridge, diagnostics, get_port_bridge, health, home, jobs,
    list_local_services, list_port_bridges, messages, nodes, runtime_standard, settings,
    static_asset, tasks, terminal_client_ws, terminal_worker_ws, tools, webhooks,
    windows_install_script, workbenches, workflows, AppState,
};

pub(crate) fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(home))
        .route("/install/windows.ps1", get(windows_install_script))
        .route("/api/health", get(health))
        .merge(auth::router())
        .merge(settings::router())
        .merge(nodes::router())
        .merge(workbenches::router())
        .merge(tasks::router())
        .merge(jobs::router())
        .merge(artifacts::router())
        .merge(runtime_standard::router())
        .merge(agents::router())
        .merge(messages::router())
        .merge(tools::router())
        .merge(webhooks::router())
        .merge(workflows::router())
        .merge(diagnostics::router())
        .route("/api/local-services", get(list_local_services))
        .route(
            "/api/bridge-sessions",
            axum::routing::post(create_bridge_session),
        )
        .route(
            "/api/bridge-sessions/{session_id}/ws",
            get(bridge_client_ws),
        )
        .route(
            "/api/port-bridges",
            get(list_port_bridges).post(create_port_bridge),
        )
        .route(
            "/api/port-bridges/{id}",
            get(get_port_bridge).delete(close_port_bridge),
        )
        .route("/api/worker/bridge/ws", get(bridge_worker_ws))
        .route("/api/terminal/ws", get(terminal_client_ws))
        .route("/api/worker/terminal/ws", get(terminal_worker_ws))
        .fallback(static_asset)
        .with_state(state)
}
