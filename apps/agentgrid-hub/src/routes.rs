use axum::{routing::get, Router};

use crate::{
    agents, artifacts, auth, diagnostics, health, home, jobs, messages, nodes, runtime_standard,
    settings, static_asset, tasks, terminal_client_ws, terminal_worker_ws, tools, webhooks,
    windows_install_script, workflows, AppState,
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
        .merge(agents::router())
        .merge(messages::router())
        .merge(tools::router())
        .merge(webhooks::router())
        .merge(workflows::router())
        .merge(diagnostics::router())
        .route("/api/terminal/ws", get(terminal_client_ws))
        .route("/api/worker/terminal/ws", get(terminal_worker_ws))
        .fallback(static_asset)
        .with_state(state)
}
