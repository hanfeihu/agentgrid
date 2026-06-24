use axum::{
    extract::{Request, State},
    http::{HeaderMap, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde_json::json;

use crate::{
    agents, artifacts, auth, bearer_token_from_headers, bridge_client_ws, bridge_worker_ws,
    close_port_bridge, create_bridge_session, create_port_bridge, diagnostics, get_port_bridge,
    health, home, jobs, list_local_services, list_port_bridges, messages, nodes, policy,
    runtime_standard, settings, static_asset, store, tasks, terminal_client_ws,
    terminal_worker_ws, tools, webhooks, windows_install_script, workbenches, workflows, AppState,
};

/// 公网鉴权护栏:经 nginx(带 x-agentgrid-public 标记)进来的【变更类】请求必须带有效会话。
/// 内部调用(ai-company 直连 hub:20181,无标记)天然绕过;worker(node token)与登录端点豁免。
async fn public_auth_guard(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    let mutating = matches!(
        *request.method(),
        Method::POST | Method::PUT | Method::DELETE | Method::PATCH
    );
    let public = headers.contains_key("x-agentgrid-public");
    let path = request.uri().path();
    // 产物含任务输出(文件/命令结果),GET 下载也要会话,防未授权外泄。
    let sensitive_read = path.starts_with("/api/artifacts");
    let exempt = path.starts_with("/api/worker")   // worker 用 node join_token 自证
        || path.starts_with("/api/auth")           // 登录/注册本身
        || path == "/api/nodes"                     // worker 心跳(POST)
        || path == "/api/events/ingress";           // 事件入口(集成)
    if public && (mutating || sensitive_read) && !exempt {
        let authorized = store(&state)
            .map(|s| {
                s.require_user_session(bearer_token_from_headers(&headers).as_deref())
                    .is_ok()
            })
            .unwrap_or(false);
        if !authorized {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "ok": false,
                    "error": { "code": "unauthorized", "message": "session required" }
                })),
            )
                .into_response();
        }
    }
    next.run(request).await
}

pub(crate) fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(home))
        .route("/install/windows.ps1", get(windows_install_script))
        .route("/api/health", get(health))
        .merge(auth::router())
        .merge(settings::router())
        .merge(policy::router())
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
        .layer(middleware::from_fn_with_state(
            state.clone(),
            public_auth_guard,
        ))
        .with_state(state)
}
