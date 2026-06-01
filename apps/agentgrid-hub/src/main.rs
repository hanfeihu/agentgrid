use std::{
    collections::{HashMap, HashSet},
    convert::Infallible,
    fmt::Write as _,
    fs,
    net::SocketAddr,
    path::{Path as FsPath, PathBuf},
    process::Command,
    sync::Arc,
    time::Duration,
};

use agentgrid_protocol::{
    AgentMessagePayload, BrowserPayload, DockerPayload, FilePayload, GitPayload,
    HttpRequestPayload, Job, JobMetadata, JobPayload, JobRequirements, JobSpec, JobState,
    JobStatus, Node, NodeState, Priority,
};
use agentgrid_scheduler::{choose_node, score_node};
use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::{header, HeaderMap, HeaderName, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        Html, IntoResponse, Response,
    },
    routing::{delete, get, post},
    Json, Router,
};
use base64::Engine as _;
use chrono::Utc;
use clap::Parser;
use futures_util::{SinkExt, StreamExt as FuturesStreamExt};
use hmac::{Hmac, Mac};
use regex::Regex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::sync::{mpsc, Mutex};
use tokio_stream::wrappers::IntervalStream;
use uuid::Uuid;

const API_VERSION: &str = "agentmessage.io/v1";
const AGENTGRID_BUILD_VERSION: &str = env!("CARGO_PKG_VERSION");
const PROJECT_ID: &str = "agentgrid";
const HEARTBEAT_UNKNOWN_AFTER_SECONDS: i64 = 30;
const HEARTBEAT_OFFLINE_AFTER_SECONDS: i64 = 120;
const HIGH_LOAD_SCORE_LIMIT: f64 = 82.0;

#[derive(Debug, Parser)]
#[command(name = "agentgrid-hub")]
#[command(about = "AgentGrid Rust Hub")]
struct Cli {
    #[arg(long, default_value = "0.0.0.0")]
    host: String,
    #[arg(long, default_value_t = 20181)]
    port: u16,
    #[arg(long, default_value = "data/agentgrid-hub.db")]
    db: PathBuf,
    #[arg(long, default_value = "web")]
    web_dir: PathBuf,
}

#[derive(Clone)]
struct AppState {
    db_path: Arc<PathBuf>,
    web_dir: Arc<PathBuf>,
    terminal: Arc<TerminalHub>,
}

#[derive(Default)]
struct TerminalHub {
    workers: Mutex<HashMap<String, mpsc::UnboundedSender<String>>>,
    clients: Mutex<HashMap<String, mpsc::UnboundedSender<String>>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let store = Store::open(&cli.db)?;
    store.migrate()?;
    store.seed()?;

    let state = AppState {
        db_path: Arc::new(cli.db),
        web_dir: Arc::new(cli.web_dir),
        terminal: Arc::new(TerminalHub::default()),
    };
    start_node_tool_probe_loop(state.clone());
    start_job_recovery_loop(state.clone());
    let app = Router::new()
        .route("/", get(home))
        .route("/install/windows.ps1", get(windows_install_script))
        .route("/api/health", get(health))
        .route("/api/bootstrap", get(get_bootstrap_status))
        .route("/api/bootstrap/admin", post(create_super_admin))
        .route("/api/auth/me", get(auth_me))
        .route("/api/auth/login", post(login_user))
        .route(
            "/api/auth/register/request-code",
            post(request_register_code),
        )
        .route("/api/auth/register", post(register_user))
        .route("/api/auth/change-password", post(change_password))
        .route(
            "/api/settings",
            get(get_system_settings).post(update_system_settings),
        )
        .route("/api/worker/update-manifest", get(worker_update_manifest))
        .route("/api/worker/download/{target}", get(download_worker_binary))
        .route("/api/agents", get(list_agents).post(upsert_agent))
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
        .route("/api/messages", get(list_messages).post(create_message))
        .route("/api/events", get(list_events))
        .route("/api/events/stream", get(event_stream))
        .route("/api/audit-events", get(list_audit_events))
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
        .route("/api/agent-runtime/manifest", get(agent_runtime_manifest))
        .route("/api/agent-runtime/tasks", post(agent_runtime_submit_task))
        .route("/api/agent-runtime/tasks/{id}", get(agent_runtime_get_task))
        .route("/api/agent-runtime/tasks/{id}/events", get(task_events))
        .route("/api/task-templates", get(list_task_templates))
        .route("/api/task-templates/{id}", get(get_task_template))
        .route("/api/task-templates/{id}/start", post(start_task_template))
        .route("/api/webhooks", get(list_webhooks).post(create_webhook))
        .route("/api/webhooks/deliveries", get(list_webhook_deliveries))
        .route("/api/webhooks/{id}", delete(delete_webhook))
        .route("/api/capabilities/manifest", get(capabilities_manifest))
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
        .route(
            "/api/scheduler-config",
            get(get_scheduler_config).post(update_scheduler_config),
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
        .route("/api/artifacts", get(list_artifacts))
        .route("/api/artifacts/{id}/download", get(download_artifact))
        .route("/api/terminal/ws", get(terminal_client_ws))
        .route(
            "/api/node-provisioning/plans",
            get(list_node_provisioning_plans).post(create_node_provisioning_plan),
        )
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
        .route("/api/jobs", get(list_jobs).post(create_job))
        .route("/api/jobs/plan", post(plan_job))
        .route("/api/jobs/recovery/scan", post(job_recovery_scan))
        .route("/api/jobs/reliability", get(job_reliability))
        .route("/api/jobs/{id}", get(get_job))
        .route("/api/jobs/{id}/checkpoints", post(create_job_checkpoint))
        .route("/api/jobs/{id}/execution", get(job_execution))
        .route("/api/jobs/{id}/events", get(job_events))
        .route("/api/events/ingress", post(create_ingress_event))
        .route("/api/tasks", get(list_tasks).post(create_task))
        .route("/api/tasks/{id}", get(get_task))
        .route("/api/tasks/{id}/snapshot", get(task_snapshot))
        .route(
            "/api/tasks/{id}/schedule-preview",
            get(task_schedule_preview),
        )
        .route("/api/tasks/{id}/events", get(task_events))
        .route("/api/tasks/{id}/control", post(control_task))
        .route("/api/tasks/{id}/{action}", post(update_task))
        .route("/api/worker/lease", post(lease_tasks))
        .route("/api/worker/reconcile", post(worker_reconcile))
        .route("/api/worker/tasks/{id}/control", get(worker_task_control))
        .route("/api/worker/tasks/{id}/renew", post(renew_worker_task))
        .route("/api/worker/tasks/{id}/logs", post(worker_task_log))
        .route("/api/worker/terminal/ws", get(terminal_worker_ws))
        .route(
            "/api/worker/tasks/{id}/complete",
            post(complete_worker_task),
        )
        .route("/api/worker/tasks/{id}/fail", post(fail_worker_task))
        .fallback(static_asset)
        .with_state(state);

    let addr: SocketAddr = format!("{}:{}", cli.host, cli.port).parse()?;
    println!("AgentGrid Rust Hub listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
}

fn new_id(prefix: &str) -> String {
    format!("{prefix}_{}", Uuid::new_v4().simple())
}

fn store(state: &AppState) -> Result<Store, ApiError> {
    Store::open(state.db_path.as_ref()).map_err(ApiError::from)
}

fn start_node_tool_probe_loop(state: AppState) {
    tokio::spawn(async move {
        let mut timer = tokio::time::interval(Duration::from_secs(60));
        loop {
            timer.tick().await;
            let result = Store::open(state.db_path.as_ref()).and_then(|store| {
                store.expire_stale_node_tool_probes()?;
                let due = store.due_node_tools_for_probe(25)?;
                let mut total = 0usize;
                for tool in due {
                    let tool_id = tool
                        .pointer("/spec/tool_id")
                        .and_then(Value::as_str)
                        .map(ToString::to_string);
                    let node_id = tool
                        .pointer("/metadata/node_id")
                        .and_then(Value::as_str)
                        .map(ToString::to_string);
                    if let (Some(tool_id), Some(node_id)) = (tool_id, node_id) {
                        total += store
                            .create_node_tool_probe_tasks(
                                Some(&tool_id),
                                Some(&node_id),
                                "automatic",
                            )?
                            .len();
                    }
                }
                if total > 0 {
                    store.audit(
                        "node_tool.probe.scheduled",
                        "node-tool-probe-engine",
                        None,
                        "节点工具自动健康检查已调度",
                        json!({ "count": total }),
                    )?;
                }
                Ok::<(), anyhow::Error>(())
            });
            if let Err(error) = result {
                eprintln!("node tool probe loop failed: {error:#}");
            }
        }
    });
}

fn start_job_recovery_loop(state: AppState) {
    tokio::spawn(async move {
        let mut timer = tokio::time::interval(Duration::from_secs(15));
        loop {
            timer.tick().await;
            let result = Store::open(state.db_path.as_ref())
                .and_then(|store| store.job_recovery_scan("loop"));
            if let Err(error) = result {
                eprintln!("job recovery loop failed: {error:#}");
            }
        }
    });
}

async fn home(State(state): State<AppState>) -> Response {
    serve_web_file(&state, "index.html")
        .await
        .unwrap_or_else(|_| Html("<!doctype html><meta charset=utf-8><title>AgentGrid Hub</title><h1>AgentGrid Rust Hub</h1><p>Web console has not been built yet.</p>").into_response())
}

async fn health() -> Json<Value> {
    Json(json!({
        "ok": true,
        "service": "agentgrid-hub",
        "runtime": "rust",
        "version": AGENTGRID_BUILD_VERSION,
        "time": now()
    }))
}

async fn get_bootstrap_status(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.bootstrap_status()?))
}

async fn create_super_admin(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.create_super_admin(input)?))
}

async fn auth_me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.auth_state(
        bearer_token_from_headers(&headers).as_deref(),
    )?))
}

async fn login_user(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.login_user(input)?))
}

async fn request_register_code(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.request_register_code(input)?))
}

async fn register_user(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.register_user(input)?))
}

async fn change_password(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.change_password(input)?))
}

async fn get_system_settings(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.system_settings()?))
}

async fn update_system_settings(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.update_system_settings(input)?))
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

async fn list_agents(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({ "items": store(&state)?.list_agents()? })))
}

async fn upsert_agent(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let item = store(&state)?.upsert_agent(input)?;
    Ok((StatusCode::CREATED, Json(item)))
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

#[derive(Debug, Deserialize)]
struct MessageQuery {
    limit: Option<u16>,
}

async fn list_messages(
    State(state): State<AppState>,
    Query(query): Query<MessageQuery>,
) -> Result<Json<Value>, ApiError> {
    let limit = query.limit.unwrap_or(100).min(500);
    Ok(Json(
        json!({ "items": store(&state)?.list_messages(limit)? }),
    ))
}

async fn create_message(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let item = store(&state)?.create_message(input)?;
    Ok((StatusCode::CREATED, Json(item)))
}

async fn list_audit_events(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "items": store(&state)?.list_audit_events(200)?
    })))
}

#[derive(Debug, Deserialize, Clone)]
struct EventQuery {
    limit: Option<u16>,
    event_type: Option<String>,
    #[serde(rename = "type")]
    type_alias: Option<String>,
    subject_id: Option<String>,
}

async fn list_events(
    State(state): State<AppState>,
    Query(query): Query<EventQuery>,
) -> Result<Json<Value>, ApiError> {
    let limit = query.limit.unwrap_or(200).min(1000);
    Ok(Json(json!({
        "ok": true,
        "items": store(&state)?.list_events(query, limit)?,
        "next_cursor": null
    })))
}

async fn event_stream(
    State(state): State<AppState>,
    Query(query): Query<EventQuery>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let limit = query.limit.unwrap_or(100).min(500);
    let stream =
        IntervalStream::new(tokio::time::interval(Duration::from_secs(1))).map(move |_| {
            let event = match Store::open(state.db_path.as_ref())
                .and_then(|store| store.list_events(query.clone(), limit))
            {
                Ok(items) => Event::default().event("events.snapshot").json_data(json!({
                    "ok": true,
                    "time": now(),
                    "items": items
                })),
                Err(error) => Event::default().event("events.error").json_data(json!({
                    "ok": false,
                    "error": { "message": error.to_string() },
                    "time": now()
                })),
            }
            .unwrap_or_else(|_| {
                Event::default()
                    .event("events.error")
                    .data("snapshot serialize failed")
            });
            Ok(event)
        });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn list_node_provisioning_plans(
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "items": store(&state)?.list_node_provisioning_plans(100)?
    })))
}

async fn list_tools(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    let nodes = store.list_nodes()?;
    let items = store
        .tool_registry_with_dynamic()?
        .into_iter()
        .map(|tool| store.enrich_tool_with_nodes(tool, &nodes))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(Json(json!({
        "ok": true,
        "kind": "ToolRegistry",
        "api_version": API_VERSION,
        "items": items
    })))
}

async fn capabilities_manifest(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    Ok(Json(store.capabilities_manifest()?))
}

async fn agent_runtime_manifest(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    let nodes = store.list_nodes()?;
    let tools = store
        .tool_registry_with_dynamic()?
        .into_iter()
        .map(|tool| store.enrich_tool_with_nodes(tool, &nodes))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(Json(json!({
        "ok": true,
        "api_version": API_VERSION,
        "kind": "AgentRuntimeManifest",
        "runtime": {
            "name": "AgentGrid",
            "version": AGENTGRID_BUILD_VERSION,
            "hub_url": "http://chenqi.tminos.com:20080/agentgrid",
            "project_id": PROJECT_ID,
            "protocols": ["AgentMessage", "AgentTask", "ToolContract", "WorkflowDAG"],
            "event_transport": "sse"
        },
        "capabilities": {
            "submit_task": true,
            "watch_task": true,
            "list_tools": true,
            "result_verification": true,
            "resource_aware_scheduling": true,
            "trust_aware_scheduling": true,
            "workflow_dag": true,
            "artifacts": true,
            "audit": true
        },
        "task_submit_endpoint": "/api/agent-runtime/tasks",
        "task_status_endpoint": "/api/agent-runtime/tasks/{task_id}",
        "task_events_endpoint": "/api/agent-runtime/tasks/{task_id}/events",
        "tools": tools,
        "submit_schema": agent_runtime_submit_schema(),
        "result_schema": agent_runtime_result_schema(),
        "examples": agent_runtime_examples()
    })))
}

async fn agent_runtime_submit_task(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let output = store(&state)?.create_agent_runtime_task(input)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "ok": true,
            "api_version": API_VERSION,
            "kind": "AgentRuntimeTaskSubmission",
            "task_id": output.item.pointer("/metadata/id").and_then(Value::as_str),
            "message_id": output.message_id,
            "item": output.item,
            "links": {
                "status": output.item.pointer("/metadata/id").and_then(Value::as_str).map(|id| format!("/api/agent-runtime/tasks/{id}")),
                "events": output.item.pointer("/metadata/id").and_then(Value::as_str).map(|id| format!("/api/agent-runtime/tasks/{id}/events")),
                "task": output.item.pointer("/metadata/id").and_then(Value::as_str).map(|id| format!("/api/tasks/{id}"))
            }
        })),
    ))
}

async fn agent_runtime_get_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let snapshot = store(&state)?.task_event_snapshot(&id)?;
    Ok(Json(json!({
        "ok": true,
        "api_version": API_VERSION,
        "kind": "AgentRuntimeTaskSnapshot",
        "item": snapshot
    })))
}

async fn list_task_templates(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "kind": "TaskTemplateStore",
        "api_version": API_VERSION,
        "items": store(&state)?.list_task_templates(200)?
    })))
}

async fn get_task_template(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?
        .get_task_template(&id)?
        .ok_or_else(|| ApiError::not_found("Task template not found"))?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn start_task_template(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let output = store(&state)?.start_task_template(&id, input)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "ok": true,
            "item": output.item,
            "message_id": output.message_id
        })),
    ))
}

async fn list_webhooks(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "items": store(&state)?.list_webhooks(200)?
    })))
}

async fn create_webhook(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let item = store(&state)?.create_webhook(input)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "ok": true, "item": item })),
    ))
}

async fn delete_webhook(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    store(&state)?.delete_webhook(&id)?;
    Ok(Json(json!({ "ok": true })))
}

async fn list_webhook_deliveries(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "items": store(&state)?.list_webhook_deliveries(200)?
    })))
}

async fn get_tool(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    let nodes = store.list_nodes()?;
    let tool = store
        .tool_registry_with_dynamic()?
        .into_iter()
        .find(|tool| tool.get("id").and_then(Value::as_str) == Some(id.as_str()))
        .ok_or_else(|| ApiError::not_found("Tool not found"))?;
    Ok(Json(json!({
        "ok": true,
        "item": store.enrich_tool_with_nodes(tool, &nodes)?
    })))
}

async fn list_tool_nodes(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let store = store(&state)?;
    let nodes = store.list_nodes()?;
    let tool = store
        .tool_registry_with_dynamic()?
        .into_iter()
        .find(|tool| tool.get("id").and_then(Value::as_str) == Some(id.as_str()))
        .ok_or_else(|| ApiError::not_found("Tool not found"))?;
    Ok(Json(json!({
        "ok": true,
        "tool_id": id,
        "items": store.nodes_for_tool_with_probe(&tool, &nodes)?
    })))
}

async fn list_tool_probes(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "items": store(&state)?.list_tool_probes(500)?
    })))
}

async fn probe_all_tools(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let items = store(&state)?.create_tool_probe_tasks(None, None)?;
    Ok(Json(json!({ "ok": true, "items": items })))
}

async fn probe_tool(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let items = store(&state)?.create_tool_probe_tasks(Some(&id), None)?;
    Ok(Json(json!({ "ok": true, "items": items })))
}

async fn probe_tool_node(
    State(state): State<AppState>,
    Path((id, node_id)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    let items = store(&state)?.create_tool_probe_tasks(Some(&id), Some(&node_id))?;
    Ok(Json(json!({ "ok": true, "items": items })))
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
async fn list_workflow_templates(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "items": store(&state)?.list_workflow_templates(100)?
    })))
}

async fn create_workflow_template(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let item = store(&state)?.create_workflow_template(input)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "ok": true, "item": item })),
    ))
}

async fn start_workflow_template(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.start_workflow_template(&id, input)?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn task_schedule_preview(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.task_schedule_preview(&id)?;
    Ok(Json(json!({ "ok": true, "item": item })))
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

async fn get_diagnostics(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "diagnostics": store(&state)?.diagnostics()?
    })))
}

async fn task_execution_record(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.task_execution_record(&id)?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn workflow_execution_record(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.workflow_execution_record(&id)?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn list_artifacts(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "items": store(&state)?.list_artifacts(300)?
    })))
}

async fn download_artifact(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let artifact = store(&state)?
        .get_artifact(&id)?
        .ok_or_else(|| ApiError::not_found("Artifact not found"))?;
    let content = artifact
        .pointer("/spec/content_base64")
        .and_then(Value::as_str)
        .unwrap_or("");
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(content)
        .map_err(|error| {
            ApiError::bad_request(&format!("artifact content decode failed: {error}"))
        })?;
    let filename = artifact
        .pointer("/spec/name")
        .and_then(Value::as_str)
        .unwrap_or("artifact.bin")
        .replace('"', "");
    let content_type = artifact
        .pointer("/spec/content_type")
        .and_then(Value::as_str)
        .unwrap_or("application/octet-stream")
        .to_string();
    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        bytes,
    )
        .into_response())
}

#[derive(Debug, Deserialize)]
struct WorkflowQuery {
    limit: Option<u16>,
    state: Option<String>,
}

async fn list_workflows(
    State(state): State<AppState>,
    Query(query): Query<WorkflowQuery>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "items": store(&state)?.list_workflows(query)?,
        "next_cursor": null
    })))
}

async fn create_workflow(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let item = store(&state)?.create_workflow(input)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "ok": true, "item": item })),
    ))
}

async fn get_workflow(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?
        .get_workflow_detail(&id)?
        .ok_or_else(|| ApiError::not_found("Workflow not found"))?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn start_workflow(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.start_workflow(&id, input)?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn cancel_workflow(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.cancel_workflow(&id, input)?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

#[derive(Debug, Deserialize)]
struct TerminalClientQuery {
    node_id: String,
}

#[derive(Debug, Deserialize)]
struct TerminalWorkerQuery {
    node_id: String,
}

async fn terminal_client_ws(
    State(state): State<AppState>,
    Query(query): Query<TerminalClientQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| handle_terminal_client(socket, state, query.node_id))
}

async fn terminal_worker_ws(
    State(state): State<AppState>,
    Query(query): Query<TerminalWorkerQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| handle_terminal_worker(socket, state, query.node_id))
}

async fn handle_terminal_client(socket: WebSocket, state: AppState, node_id: String) {
    let session_id = new_id("term");
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    state
        .terminal
        .clients
        .lock()
        .await
        .insert(session_id.clone(), tx);

    let open_message = json!({
        "type": "terminal.open",
        "session_id": session_id,
        "node_id": node_id
    })
    .to_string();
    if let Some(worker) = state.terminal.workers.lock().await.get(&node_id).cloned() {
        let _ = worker.send(open_message);
        let _ = Store::open(state.db_path.as_ref()).and_then(|store| {
            store.audit(
                "terminal.opened",
                "architect-agent",
                Some(&node_id),
                "远程终端已打开",
                json!({ "node_id": node_id, "session_id": session_id }),
            )
        });
    } else {
        let _ = sink
            .send(WsMessage::Text(
                json!({
                    "type": "terminal.error",
                    "session_id": session_id,
                    "message": "节点没有连接远程终端通道"
                })
                .to_string()
                .into(),
            ))
            .await;
    }

    let send_task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if sink.send(WsMessage::Text(message.into())).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(message)) = stream.next().await {
        match message {
            WsMessage::Text(text) => {
                let value = serde_json::from_str::<Value>(&text).unwrap_or_else(|_| {
                    json!({
                        "type": "terminal.input",
                        "data": text.to_string()
                    })
                });
                let forwarded = json!({
                    "type": value.get("type").and_then(Value::as_str).unwrap_or("terminal.input"),
                    "session_id": session_id,
                    "node_id": node_id,
                    "data": value.get("data").and_then(Value::as_str).unwrap_or(""),
                    "cols": value.get("cols").and_then(Value::as_u64).unwrap_or(120),
                    "rows": value.get("rows").and_then(Value::as_u64).unwrap_or(32)
                })
                .to_string();
                if let Some(worker) = state.terminal.workers.lock().await.get(&node_id).cloned() {
                    let _ = worker.send(forwarded);
                }
            }
            WsMessage::Binary(_) => {}
            WsMessage::Close(_) => break,
            WsMessage::Ping(_) | WsMessage::Pong(_) => {}
        }
    }

    let close_message = json!({
        "type": "terminal.close",
        "session_id": session_id,
        "node_id": node_id
    })
    .to_string();
    if let Some(worker) = state.terminal.workers.lock().await.get(&node_id).cloned() {
        let _ = worker.send(close_message);
    }
    state.terminal.clients.lock().await.remove(&session_id);
    send_task.abort();
}

async fn handle_terminal_worker(socket: WebSocket, state: AppState, node_id: String) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    state
        .terminal
        .workers
        .lock()
        .await
        .insert(node_id.clone(), tx);
    let _ = Store::open(state.db_path.as_ref()).and_then(|store| {
        store.audit(
            "terminal.worker.connected",
            &node_id,
            Some(&node_id),
            "Worker 远程终端通道已连接",
            json!({ "node_id": node_id }),
        )
    });

    let send_task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if sink.send(WsMessage::Text(message.into())).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(message)) = stream.next().await {
        match message {
            WsMessage::Text(text) => {
                let Ok(value) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                let Some(session_id) = value.get("session_id").and_then(Value::as_str) else {
                    continue;
                };
                if let Some(client) = state.terminal.clients.lock().await.get(session_id).cloned() {
                    let _ = client.send(value.to_string());
                }
            }
            WsMessage::Binary(_) => {}
            WsMessage::Close(_) => break,
            WsMessage::Ping(_) | WsMessage::Pong(_) => {}
        }
    }

    state.terminal.workers.lock().await.remove(&node_id);
    send_task.abort();
    let _ = Store::open(state.db_path.as_ref()).and_then(|store| {
        store.audit(
            "terminal.worker.disconnected",
            &node_id,
            Some(&node_id),
            "Worker 远程终端通道已断开",
            json!({ "node_id": node_id }),
        )
    });
}

#[derive(Debug, Deserialize)]
struct TaskQuery {
    limit: Option<u16>,
    owner: Option<String>,
    state: Option<String>,
}

async fn list_tasks(
    State(state): State<AppState>,
    Query(query): Query<TaskQuery>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "items": store(&state)?.list_tasks(query)?,
        "next_cursor": null
    })))
}

async fn get_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?
        .get_task(&id)?
        .ok_or_else(|| ApiError::not_found("Task not found"))?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn task_snapshot(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.task_event_snapshot(&id)?))
}

async fn task_events(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let stream =
        IntervalStream::new(tokio::time::interval(Duration::from_secs(1))).map(move |_| {
            let event = match Store::open(state.db_path.as_ref())
                .and_then(|store| store.task_event_snapshot(&id))
            {
                Ok(snapshot) => Event::default().event("task.snapshot").json_data(snapshot),
                Err(error) => Event::default().event("task.error").json_data(json!({
                    "ok": false,
                    "task_id": id,
                    "error": { "message": error.to_string() },
                    "time": now()
                })),
            }
            .unwrap_or_else(|_| {
                Event::default()
                    .event("task.error")
                    .data("snapshot serialize failed")
            });
            Ok(event)
        });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn create_task(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let output = store(&state)?.create_task(input)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "ok": true, "item": output.item, "message_id": output.message_id })),
    ))
}

async fn update_task(
    State(state): State<AppState>,
    Path((id, action)): Path<(String, String)>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let output = store(&state)?.update_task(&id, &action, input)?;
    Ok(Json(
        json!({ "ok": true, "item": output.item, "message_id": output.message_id }),
    ))
}

async fn control_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.control_task(&id, input)?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn lease_tasks(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.lease_tasks(input)?))
}

async fn worker_reconcile(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.worker_reconcile(input)?))
}

async fn worker_task_control(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.worker_task_control(&id)?))
}

async fn renew_worker_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(store(&state)?.renew_worker_task(&id, input)?))
}

async fn worker_task_log(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    store(&state)?.append_task_log(&id, input)?;
    Ok(Json(json!({ "ok": true })))
}

async fn complete_worker_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.complete_worker_task(&id, input)?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn fail_worker_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.fail_worker_task(&id, input)?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

#[derive(Debug, Deserialize)]
struct JobQuery {
    limit: Option<u16>,
    state: Option<String>,
}

async fn list_jobs(
    State(state): State<AppState>,
    Query(query): Query<JobQuery>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({
        "ok": true,
        "items": store(&state)?.list_jobs(query)?,
        "next_cursor": null
    })))
}

async fn create_job(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let item = store(&state)?.create_job(input)?;
    let reused = item
        .pointer("/status/idempotency_reused")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Ok((
        if reused {
            StatusCode::OK
        } else {
            StatusCode::CREATED
        },
        Json(json!({ "ok": true, "reused": reused, "item": item })),
    ))
}

async fn plan_job(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.plan_job(input)?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn job_reliability(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.job_reliability_status()?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn job_recovery_scan(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.job_recovery_scan("manual")?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn get_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?
        .get_job_detail(&id)?
        .ok_or_else(|| ApiError::not_found("Job not found"))?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn create_job_checkpoint(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?.create_job_checkpoint(&id, input)?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn job_execution(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let item = store(&state)?
        .job_execution_view(&id)?
        .ok_or_else(|| ApiError::not_found("Job not found"))?;
    Ok(Json(json!({ "ok": true, "item": item })))
}

async fn job_events(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let stream =
        IntervalStream::new(tokio::time::interval(Duration::from_secs(1))).map(move |_| {
            let event = match Store::open(state.db_path.as_ref())
                .and_then(|store| store.job_event_snapshot(&id))
            {
                Ok(snapshot) => Event::default().event("job.snapshot").json_data(snapshot),
                Err(error) => Event::default().event("job.error").json_data(json!({
                    "ok": false,
                    "job_id": id,
                    "error": { "message": error.to_string() },
                    "time": now()
                })),
            }
            .unwrap_or_else(|_| {
                Event::default()
                    .event("job.error")
                    .data("snapshot serialize failed")
            });
            Ok(event)
        });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn create_ingress_event(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let item = store(&state)?.create_ingress_event(input)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "ok": true, "item": item })),
    ))
}

async fn static_asset(State(state): State<AppState>, uri: axum::http::Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    match serve_web_file(&state, path).await {
        Ok(response) => response,
        Err(_) => match serve_web_file(&state, "index.html").await {
            Ok(response) => response,
            Err(_) => (
                StatusCode::NOT_FOUND,
                Json(
                    json!({ "ok": false, "error": { "code": "not_found", "message": "Not found" } }),
                ),
            )
                .into_response(),
        },
    }
}

async fn windows_install_script() -> Response {
    (
        [(
            header::CONTENT_TYPE,
            "text/plain; charset=utf-8".to_string(),
        )],
        include_str!("../../../scripts/install-windows-agentgrid.ps1"),
    )
        .into_response()
}

async fn serve_web_file(state: &AppState, requested: &str) -> anyhow::Result<Response> {
    let safe_path = requested.trim_start_matches('/').replace("..", "");
    let path = state.web_dir.join(safe_path);
    let bytes = tokio::fs::read(&path).await?;
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    Ok(([(header::CONTENT_TYPE, mime.to_string())], bytes).into_response())
}

fn worker_binary_path(state: &AppState, target: &str) -> PathBuf {
    state
        .web_dir
        .join("downloads")
        .join(target)
        .join(if target.contains("windows") {
            "agentgrid-worker.exe"
        } else {
            "agentgrid-worker"
        })
}

fn worker_target_from_query(os: Option<&str>, arch: Option<&str>) -> String {
    let os = normalize_os(os.unwrap_or(std::env::consts::OS));
    let arch = normalize_arch(arch.unwrap_or(std::env::consts::ARCH));
    format!("{os}-{arch}")
}

fn sanitize_worker_target(target: &str) -> Result<String, ApiError> {
    if target
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.')
    {
        Ok(target.to_string())
    } else {
        Err(ApiError::bad_request("invalid worker target"))
    }
}

fn normalize_os(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    if lower.contains("darwin") || lower.contains("mac") {
        "darwin".to_string()
    } else if lower.contains("windows") || lower.contains("win") {
        "windows".to_string()
    } else if lower.contains("linux") || lower.contains("ubuntu") {
        "linux".to_string()
    } else {
        lower.replace(' ', "-")
    }
}

fn normalize_arch(value: &str) -> String {
    match value.to_ascii_lowercase().as_str() {
        "x86_64" | "amd64" => "x86_64".to_string(),
        "aarch64" | "arm64" => "aarch64".to_string(),
        other => other.replace(' ', "-"),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn agent_token_hash(agent_id: &str, token: &str) -> String {
    sha256_hex(format!("agentgrid-agent-token-v1:{agent_id}:{token}").as_bytes())
}

fn user_password_hash(email: &str, password: &str) -> String {
    sha256_hex(format!("agentgrid-user-password-v1:{email}:{password}").as_bytes())
}

fn session_token_hash(token: &str) -> String {
    sha256_hex(format!("agentgrid-session-v1:{token}").as_bytes())
}

fn email_code_hash(email: &str, code: &str) -> String {
    sha256_hex(format!("agentgrid-email-code-v1:{email}:{code}").as_bytes())
}

fn node_join_token_hash(node_id: &str, token: &str) -> String {
    sha256_hex(format!("agentgrid-node-join-token-v1:{node_id}:{token}").as_bytes())
}

fn bearer_token_from_headers(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    value
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(ToString::to_string)
}

fn generate_session_token() -> String {
    format!("ags_{}", Uuid::new_v4().simple())
}

fn generate_email_code() -> String {
    let value = Uuid::new_v4().as_u128() % 1_000_000;
    format!("{value:06}")
}

fn default_smtp_setting() -> Value {
    json!({
        "host": "smtp.qq.com",
        "port": 465,
        "username": "1668217900@qq.com",
        "password": "oebnbqxrirmybacd",
        "from": "1668217900@qq.com",
        "enabled": true
    })
}

fn token_hint(token: &str) -> String {
    let chars = token.chars().collect::<Vec<_>>();
    if chars.len() <= 6 {
        return "******".to_string();
    }
    let tail = chars[chars.len().saturating_sub(4)..]
        .iter()
        .collect::<String>();
    format!("****{tail}")
}

fn worker_update_compatibility(target: &str, glibc_version: Option<&str>) -> Value {
    if !target.starts_with("linux-") {
        return json!({
            "compatible": true,
            "reason": "non-linux target does not require glibc compatibility check"
        });
    }
    let minimum = worker_target_min_glibc(target);
    let Some(minimum) = minimum else {
        return json!({
            "compatible": true,
            "reason": "no minimum glibc version declared for target"
        });
    };
    let Some(actual) = glibc_version.filter(|value| !value.trim().is_empty()) else {
        return json!({
            "compatible": false,
            "reason": format!("glibc version is required for target {target}"),
            "required_glibc": minimum
        });
    };
    let compatible = compare_versions(actual, minimum) >= 0;
    json!({
        "compatible": compatible,
        "reason": if compatible {
            format!("glibc {actual} satisfies required {minimum}")
        } else {
            format!("glibc {actual} is lower than required {minimum}")
        },
        "required_glibc": minimum,
        "reported_glibc": actual
    })
}

fn worker_target_min_glibc(target: &str) -> Option<&'static str> {
    match target {
        "linux-x86_64" => Some("2.34"),
        _ if target.starts_with("linux-glibc-2.32-") => Some("2.32"),
        _ if target.starts_with("linux-glibc-2.34-") => Some("2.34"),
        _ => None,
    }
}

fn compare_versions(left: &str, right: &str) -> i32 {
    let parse = |value: &str| {
        value
            .split('.')
            .map(|part| part.parse::<i64>().unwrap_or(0))
            .collect::<Vec<_>>()
    };
    let left = parse(left);
    let right = parse(right);
    let max = left.len().max(right.len());
    for index in 0..max {
        let a = *left.get(index).unwrap_or(&0);
        let b = *right.get(index).unwrap_or(&0);
        if a > b {
            return 1;
        }
        if a < b {
            return -1;
        }
    }
    0
}

struct Store {
    conn: Connection,
}

#[derive(Debug, Clone)]
struct NodeAuthorization {
    status: String,
    previous_status: String,
    machine_fingerprint: Option<String>,
    join_token_hash: Option<String>,
    join_token_hint: String,
    authorized_at: Option<String>,
}

struct SmtpConfig {
    host: String,
    port: u16,
    username: String,
    password: String,
    from: String,
}

struct TaskOutput {
    item: Value,
    message_id: Option<String>,
}

#[derive(Debug, Clone)]
struct WorkflowNode {
    id: String,
    title: String,
    summary: String,
    payload: Value,
    depends_on: Vec<String>,
    on_failure: String,
    optional: bool,
    labels: Vec<String>,
    owner: Option<String>,
    priority: String,
    acceptance_criteria: Vec<String>,
    outputs: Vec<String>,
}

struct ArtifactInput<'a> {
    task_id: &'a str,
    node_id: Option<&'a str>,
    name: &'a str,
    artifact_type: &'a str,
    content_type: &'a str,
    content_base64: Option<&'a str>,
    source_path: Option<&'a str>,
    size_bytes: u64,
    tool_id: Option<&'a str>,
    metadata: Value,
}

#[derive(Debug, Clone)]
struct TrustEvaluation {
    tool_id: Option<String>,
    state: String,
    support_basis: String,
    multiplier: f64,
    risk: String,
    risk_multiplier: f64,
    reason: String,
}

struct RuntimeToolSelection {
    tool: Value,
    dynamic: bool,
}

impl Store {
    fn open(path: impl AsRef<FsPath>) -> anyhow::Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(Self {
            conn: Connection::open(path)?,
        })
    }

    fn migrate(&self) -> anyhow::Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS agents (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                name TEXT NOT NULL,
                role TEXT NOT NULL,
                skills_json TEXT NOT NULL,
                permissions_json TEXT NOT NULL,
                responsibility TEXT NOT NULL DEFAULT '',
                auth_type TEXT NOT NULL DEFAULT 'bearer_token',
                token_hash TEXT,
                token_hint TEXT NOT NULL DEFAULT '',
                credential_status TEXT NOT NULL DEFAULT 'not_configured',
                account_username TEXT NOT NULL DEFAULT '',
                credential_refs_json TEXT NOT NULL DEFAULT '{}',
                node_scope_json TEXT NOT NULL DEFAULT '{\"mode\":\"none\",\"nodes\":[],\"groups\":[],\"os\":[]}',
                tool_scope_json TEXT NOT NULL DEFAULT '{\"mode\":\"declared\",\"tools\":[]}',
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS organizations (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                name TEXT NOT NULL,
                slug TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(project_id, slug)
            );
            CREATE TABLE IF NOT EXISTS hub_users (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                organization_id TEXT NOT NULL,
                email TEXT NOT NULL,
                name TEXT NOT NULL,
                role TEXT NOT NULL,
                password_hash TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'active',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(project_id, email)
            );
            CREATE TABLE IF NOT EXISTS user_sessions (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                token_hash TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS email_verification_codes (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                email TEXT NOT NULL,
                code_hash TEXT NOT NULL,
                purpose TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                consumed_at TEXT,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS hub_settings (
                key TEXT PRIMARY KEY,
                value_json TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS nodes (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                name TEXT NOT NULL,
                os TEXT NOT NULL,
                arch TEXT NOT NULL,
                address TEXT NOT NULL DEFAULT '',
                tags_json TEXT NOT NULL,
                capabilities_json TEXT NOT NULL,
                groups_json TEXT NOT NULL DEFAULT '[]',
                weight REAL NOT NULL DEFAULT 1,
                max_concurrent_jobs INTEGER NOT NULL DEFAULT 1,
                success_count INTEGER NOT NULL DEFAULT 0,
                failure_count INTEGER NOT NULL DEFAULT 0,
                cpu_cores INTEGER NOT NULL DEFAULT 0,
                memory_mb INTEGER NOT NULL DEFAULT 0,
                cpu_usage_percent REAL NOT NULL DEFAULT 0,
                memory_used_mb INTEGER NOT NULL DEFAULT 0,
                disk_total_mb INTEGER NOT NULL DEFAULT 0,
                disk_free_mb INTEGER NOT NULL DEFAULT 0,
                running_jobs INTEGER NOT NULL DEFAULT 0,
                worker_version TEXT,
                worker_target TEXT,
                glibc_version TEXT,
                machine_fingerprint TEXT,
                join_token_hash TEXT,
                join_token_hint TEXT NOT NULL DEFAULT '',
                auth_status TEXT NOT NULL DEFAULT 'legacy',
                authorized_at TEXT,
                auto_update_enabled INTEGER NOT NULL DEFAULT 1,
                update_channel TEXT NOT NULL DEFAULT 'stable',
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                last_heartbeat_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS agent_messages (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                from_agent_id TEXT NOT NULL,
                to_agents_json TEXT NOT NULL,
                message_type TEXT NOT NULL,
                subject TEXT NOT NULL,
                summary TEXT NOT NULL,
                priority TEXT NOT NULL,
                requires_ack INTEGER NOT NULL,
                payload_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS agent_tasks (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                title TEXT NOT NULL,
                summary TEXT NOT NULL DEFAULT '',
                created_by TEXT NOT NULL DEFAULT 'architect-agent',
                owner_agent_id TEXT,
                status TEXT NOT NULL,
                priority TEXT NOT NULL,
                inputs_json TEXT NOT NULL,
                outputs_json TEXT NOT NULL,
                acceptance_criteria_json TEXT NOT NULL,
                progress INTEGER NOT NULL DEFAULT 0,
                blocked_reason TEXT,
                assigned_to_json TEXT NOT NULL DEFAULT '[]',
                labels_json TEXT NOT NULL DEFAULT '[]',
                depends_on_json TEXT NOT NULL DEFAULT '[]',
                due_at TEXT,
                started_at TEXT,
                completed_at TEXT,
                assignment_message_id TEXT,
                last_message_id TEXT,
                correlation_id TEXT,
                leased_by_node_id TEXT,
                lease_expires_at TEXT,
                attempts INTEGER NOT NULL DEFAULT 0,
                result_json TEXT,
                error_json TEXT,
                control_json TEXT,
                verify_json TEXT,
                workflow_id TEXT,
                workflow_node_id TEXT,
                job_id TEXT,
                job_attempt_id TEXT,
                job_shard_id TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS audit_events (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                actor TEXT NOT NULL,
                subject_id TEXT,
                summary TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS task_logs (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                task_id TEXT NOT NULL,
                node_id TEXT NOT NULL,
                stream TEXT NOT NULL,
                line TEXT NOT NULL,
                sequence INTEGER NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS artifacts (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                task_id TEXT NOT NULL,
                node_id TEXT,
                name TEXT NOT NULL,
                artifact_type TEXT NOT NULL,
                content_type TEXT NOT NULL,
                content_base64 TEXT,
                source_path TEXT,
                size_bytes INTEGER NOT NULL DEFAULT 0,
                metadata_json TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS workflows (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                name TEXT NOT NULL,
                summary TEXT NOT NULL DEFAULT '',
                created_by TEXT NOT NULL DEFAULT 'architect-agent',
                status TEXT NOT NULL,
                inputs_json TEXT NOT NULL DEFAULT '{}',
                nodes_json TEXT NOT NULL,
                result_json TEXT,
                error_json TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                started_at TEXT,
                completed_at TEXT
            );
            CREATE TABLE IF NOT EXISTS workflow_runs (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                workflow_id TEXT NOT NULL,
                workflow_node_id TEXT NOT NULL,
                task_id TEXT,
                status TEXT NOT NULL,
                depends_on_json TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                started_at TEXT,
                completed_at TEXT,
                result_json TEXT,
                error_json TEXT
            );
            CREATE TABLE IF NOT EXISTS jobs (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                title TEXT NOT NULL,
                summary TEXT NOT NULL DEFAULT '',
                created_by TEXT NOT NULL DEFAULT 'agent-runtime',
                status TEXT NOT NULL,
                tool_id TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                placement_json TEXT NOT NULL DEFAULT '{}',
                strategy_json TEXT NOT NULL DEFAULT '{}',
                reduce_json TEXT NOT NULL DEFAULT '{}',
                retry_policy_json TEXT NOT NULL DEFAULT '{}',
                checkpoint_policy_json TEXT NOT NULL DEFAULT '{}',
                idempotency_json TEXT NOT NULL DEFAULT '{}',
                idempotency_key TEXT,
                max_attempts INTEGER NOT NULL DEFAULT 3,
                latest_checkpoint_id TEXT,
                current_attempt_id TEXT,
                current_task_id TEXT,
                reducer_task_id TEXT,
                result_json TEXT,
                error_json TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                completed_at TEXT
            );
            CREATE TABLE IF NOT EXISTS job_attempts (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                job_id TEXT NOT NULL,
                shard_id TEXT,
                attempt_number INTEGER NOT NULL,
                task_id TEXT NOT NULL,
                node_id TEXT,
                status TEXT NOT NULL,
                reason TEXT NOT NULL DEFAULT '',
                resume_checkpoint_id TEXT,
                started_at TEXT,
                completed_at TEXT,
                result_json TEXT,
                error_json TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS job_shards (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                job_id TEXT NOT NULL,
                shard_index INTEGER NOT NULL,
                shard_count INTEGER NOT NULL,
                status TEXT NOT NULL,
                current_attempt_id TEXT,
                current_task_id TEXT,
                node_id TEXT,
                payload_json TEXT NOT NULL DEFAULT '{}',
                result_json TEXT,
                error_json TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                completed_at TEXT,
                UNIQUE(project_id, job_id, shard_index)
            );
            CREATE TABLE IF NOT EXISTS job_checkpoints (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                job_id TEXT NOT NULL,
                attempt_id TEXT,
                task_id TEXT,
                node_id TEXT,
                sequence INTEGER NOT NULL DEFAULT 0,
                progress INTEGER NOT NULL DEFAULT 0,
                resume_token_json TEXT NOT NULL DEFAULT '{}',
                artifacts_json TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS ingress_events (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                source TEXT NOT NULL DEFAULT '',
                target_json TEXT NOT NULL DEFAULT '{}',
                event_type TEXT NOT NULL,
                idempotency_key TEXT NOT NULL,
                payload_json TEXT NOT NULL DEFAULT '{}',
                status TEXT NOT NULL,
                ttl_seconds INTEGER NOT NULL DEFAULT 300,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(project_id, idempotency_key)
            );
            CREATE TABLE IF NOT EXISTS node_provisioning_plans (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                node_id TEXT NOT NULL,
                node_name TEXT NOT NULL,
                ssh_host TEXT NOT NULL,
                ssh_user TEXT NOT NULL,
                os TEXT NOT NULL,
                arch TEXT NOT NULL,
                hub_url TEXT NOT NULL,
                status TEXT NOT NULL,
                steps_json TEXT NOT NULL,
                notes TEXT NOT NULL DEFAULT '',
                join_token_hash TEXT,
                join_token_hint TEXT NOT NULL DEFAULT '',
                bound_machine_fingerprint TEXT,
                bound_at TEXT,
                created_by TEXT NOT NULL DEFAULT 'architect-agent',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS workflow_templates (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                name TEXT NOT NULL,
                summary TEXT NOT NULL DEFAULT '',
                created_by TEXT NOT NULL DEFAULT 'architect-agent',
                parameters_json TEXT NOT NULL DEFAULT '[]',
                nodes_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS security_policies (
                project_id TEXT PRIMARY KEY,
                policy_json TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS scheduler_configs (
                project_id TEXT PRIMARY KEY,
                config_json TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS tool_probes (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                tool_id TEXT NOT NULL,
                node_id TEXT NOT NULL,
                task_id TEXT,
                status TEXT NOT NULL,
                support_basis TEXT NOT NULL,
                started_at TEXT,
                completed_at TEXT,
                expires_at TEXT,
                result_json TEXT,
                error_json TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(project_id, tool_id, node_id)
            );
            CREATE TABLE IF NOT EXISTS node_tools (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                node_id TEXT NOT NULL,
                tool_id TEXT NOT NULL,
                name TEXT NOT NULL,
                version TEXT NOT NULL DEFAULT '0.1.0',
                executor TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'available',
                confidence TEXT NOT NULL DEFAULT 'declared',
                input_schema_json TEXT NOT NULL DEFAULT '{}',
                output_schema_json TEXT NOT NULL DEFAULT '{}',
                constraints_json TEXT NOT NULL DEFAULT '{}',
                labels_json TEXT NOT NULL DEFAULT '[]',
                default_verify_json TEXT,
                probe_json TEXT,
                probe_state TEXT NOT NULL DEFAULT 'declared_unverified',
                last_probe_at TEXT,
                next_probe_at TEXT,
                probe_task_id TEXT,
                probe_error_json TEXT,
                metadata_json TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(project_id, node_id, tool_id)
            );
            CREATE TABLE IF NOT EXISTS task_templates (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                name TEXT NOT NULL,
                summary TEXT NOT NULL DEFAULT '',
                category TEXT NOT NULL DEFAULT 'general',
                tool_id TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                parameters_json TEXT NOT NULL DEFAULT '[]',
                verify_json TEXT,
                labels_json TEXT NOT NULL DEFAULT '[]',
                created_by TEXT NOT NULL DEFAULT 'architect-agent',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS webhook_subscriptions (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                name TEXT NOT NULL,
                url TEXT NOT NULL,
                events_json TEXT NOT NULL,
                secret TEXT,
                enabled INTEGER NOT NULL DEFAULT 1,
                created_by TEXT NOT NULL DEFAULT 'architect-agent',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS webhook_deliveries (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                webhook_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                subject_id TEXT,
                status TEXT NOT NULL,
                status_code INTEGER,
                error TEXT,
                payload_json TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_nodes_project_status ON nodes(project_id, status, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_hub_users_project_email ON hub_users(project_id, email);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_one_super_admin ON hub_users(project_id, role) WHERE role = 'super_admin';
            CREATE INDEX IF NOT EXISTS idx_user_sessions_token ON user_sessions(token_hash);
            CREATE INDEX IF NOT EXISTS idx_email_codes_email ON email_verification_codes(project_id, email, purpose, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_agent_messages_project_created ON agent_messages(project_id, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_agent_tasks_project_status ON agent_tasks(project_id, status, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_audit_project_created ON audit_events(project_id, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_task_logs_task_sequence ON task_logs(task_id, sequence);
            CREATE INDEX IF NOT EXISTS idx_artifacts_project_created ON artifacts(project_id, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_artifacts_task_created ON artifacts(task_id, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_workflows_project_status ON workflows(project_id, status, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_workflow_runs_workflow ON workflow_runs(workflow_id, status, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_workflow_runs_task ON workflow_runs(task_id);
            CREATE INDEX IF NOT EXISTS idx_jobs_project_status ON jobs(project_id, status, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_job_attempts_job ON job_attempts(job_id, attempt_number);
            CREATE INDEX IF NOT EXISTS idx_job_attempts_task ON job_attempts(task_id);
            CREATE INDEX IF NOT EXISTS idx_job_shards_job_status ON job_shards(job_id, status, shard_index);
            CREATE INDEX IF NOT EXISTS idx_job_checkpoints_job ON job_checkpoints(job_id, sequence DESC);
            CREATE INDEX IF NOT EXISTS idx_ingress_events_project_status ON ingress_events(project_id, status, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_node_provisioning_project_created ON node_provisioning_plans(project_id, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_workflow_templates_project_created ON workflow_templates(project_id, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_tool_probes_project_tool ON tool_probes(project_id, tool_id, node_id);
            CREATE INDEX IF NOT EXISTS idx_tool_probes_task ON tool_probes(task_id);
            CREATE INDEX IF NOT EXISTS idx_node_tools_project_tool ON node_tools(project_id, tool_id, node_id);
            CREATE INDEX IF NOT EXISTS idx_node_tools_project_node ON node_tools(project_id, node_id);
            CREATE INDEX IF NOT EXISTS idx_task_templates_project_category ON task_templates(project_id, category, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_webhooks_project_enabled ON webhook_subscriptions(project_id, enabled);
            CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_project_created ON webhook_deliveries(project_id, created_at DESC);
            ",
        )?;
        self.ensure_column("agents", "responsibility", "TEXT NOT NULL DEFAULT ''")?;
        self.ensure_column(
            "agents",
            "auth_type",
            "TEXT NOT NULL DEFAULT 'bearer_token'",
        )?;
        self.ensure_column("agents", "token_hash", "TEXT")?;
        self.ensure_column("agents", "token_hint", "TEXT NOT NULL DEFAULT ''")?;
        self.ensure_column(
            "agents",
            "credential_status",
            "TEXT NOT NULL DEFAULT 'not_configured'",
        )?;
        self.ensure_column("agents", "account_username", "TEXT NOT NULL DEFAULT ''")?;
        self.ensure_column(
            "agents",
            "credential_refs_json",
            "TEXT NOT NULL DEFAULT '{}'",
        )?;
        self.ensure_column(
            "agents",
            "node_scope_json",
            "TEXT NOT NULL DEFAULT '{\"mode\":\"none\",\"nodes\":[],\"groups\":[],\"os\":[]}'",
        )?;
        self.ensure_column(
            "agents",
            "tool_scope_json",
            "TEXT NOT NULL DEFAULT '{\"mode\":\"declared\",\"tools\":[]}'",
        )?;
        self.ensure_column("nodes", "address", "TEXT NOT NULL DEFAULT ''")?;
        self.ensure_column("nodes", "groups_json", "TEXT NOT NULL DEFAULT '[]'")?;
        self.ensure_column("nodes", "weight", "REAL NOT NULL DEFAULT 1")?;
        self.ensure_column("nodes", "max_concurrent_jobs", "INTEGER NOT NULL DEFAULT 1")?;
        self.ensure_column("nodes", "success_count", "INTEGER NOT NULL DEFAULT 0")?;
        self.ensure_column("nodes", "failure_count", "INTEGER NOT NULL DEFAULT 0")?;
        self.ensure_column("nodes", "cpu_usage_percent", "REAL NOT NULL DEFAULT 0")?;
        self.ensure_column("nodes", "memory_used_mb", "INTEGER NOT NULL DEFAULT 0")?;
        self.ensure_column("nodes", "disk_total_mb", "INTEGER NOT NULL DEFAULT 0")?;
        self.ensure_column("nodes", "disk_free_mb", "INTEGER NOT NULL DEFAULT 0")?;
        self.ensure_column("nodes", "worker_version", "TEXT")?;
        self.ensure_column("nodes", "worker_target", "TEXT")?;
        self.ensure_column("nodes", "glibc_version", "TEXT")?;
        self.ensure_column("nodes", "machine_fingerprint", "TEXT")?;
        self.ensure_column("nodes", "join_token_hash", "TEXT")?;
        self.ensure_column("nodes", "join_token_hint", "TEXT NOT NULL DEFAULT ''")?;
        self.ensure_column("nodes", "auth_status", "TEXT NOT NULL DEFAULT 'legacy'")?;
        self.ensure_column("nodes", "authorized_at", "TEXT")?;
        self.ensure_column("nodes", "auto_update_enabled", "INTEGER NOT NULL DEFAULT 1")?;
        self.ensure_column("nodes", "update_channel", "TEXT NOT NULL DEFAULT 'stable'")?;
        self.ensure_column("node_provisioning_plans", "join_token_hash", "TEXT")?;
        self.ensure_column(
            "node_provisioning_plans",
            "join_token_hint",
            "TEXT NOT NULL DEFAULT ''",
        )?;
        self.ensure_column(
            "node_provisioning_plans",
            "bound_machine_fingerprint",
            "TEXT",
        )?;
        self.ensure_column("node_provisioning_plans", "bound_at", "TEXT")?;
        self.ensure_column("node_tools", "probe_json", "TEXT")?;
        self.ensure_column(
            "node_tools",
            "probe_state",
            "TEXT NOT NULL DEFAULT 'declared_unverified'",
        )?;
        self.ensure_column("node_tools", "last_probe_at", "TEXT")?;
        self.ensure_column("node_tools", "next_probe_at", "TEXT")?;
        self.ensure_column("node_tools", "probe_task_id", "TEXT")?;
        self.ensure_column("node_tools", "probe_error_json", "TEXT")?;
        self.ensure_column(
            "agent_tasks",
            "assigned_to_json",
            "TEXT NOT NULL DEFAULT '[]'",
        )?;
        self.ensure_column("agent_tasks", "labels_json", "TEXT NOT NULL DEFAULT '[]'")?;
        self.ensure_column(
            "agent_tasks",
            "depends_on_json",
            "TEXT NOT NULL DEFAULT '[]'",
        )?;
        self.ensure_column("agent_tasks", "due_at", "TEXT")?;
        self.ensure_column("agent_tasks", "started_at", "TEXT")?;
        self.ensure_column("agent_tasks", "completed_at", "TEXT")?;
        self.ensure_column("agent_tasks", "assignment_message_id", "TEXT")?;
        self.ensure_column("agent_tasks", "last_message_id", "TEXT")?;
        self.ensure_column("agent_tasks", "correlation_id", "TEXT")?;
        self.ensure_column("agent_tasks", "leased_by_node_id", "TEXT")?;
        self.ensure_column("agent_tasks", "lease_expires_at", "TEXT")?;
        self.ensure_column("agent_tasks", "attempts", "INTEGER NOT NULL DEFAULT 0")?;
        self.ensure_column("agent_tasks", "result_json", "TEXT")?;
        self.ensure_column("agent_tasks", "error_json", "TEXT")?;
        self.ensure_column("agent_tasks", "control_json", "TEXT")?;
        self.ensure_column("agent_tasks", "verify_json", "TEXT")?;
        self.ensure_column("agent_tasks", "workflow_id", "TEXT")?;
        self.ensure_column("agent_tasks", "workflow_node_id", "TEXT")?;
        self.ensure_column("agent_tasks", "job_id", "TEXT")?;
        self.ensure_column("agent_tasks", "job_attempt_id", "TEXT")?;
        self.ensure_column("agent_tasks", "job_shard_id", "TEXT")?;
        self.ensure_column(
            "jobs",
            "strategy_json",
            "TEXT NOT NULL DEFAULT '{\"type\":\"single\"}'",
        )?;
        self.ensure_column("jobs", "reduce_json", "TEXT NOT NULL DEFAULT '{}'")?;
        self.ensure_column("jobs", "reducer_task_id", "TEXT")?;
        self.ensure_column("jobs", "idempotency_key", "TEXT")?;
        self.ensure_column("job_attempts", "shard_id", "TEXT")?;
        self.conn.execute_batch(
            "
            CREATE INDEX IF NOT EXISTS idx_job_attempts_shard ON job_attempts(shard_id, attempt_number);
            CREATE INDEX IF NOT EXISTS idx_agent_tasks_job_shard ON agent_tasks(job_shard_id);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_jobs_idempotency_key ON jobs(project_id, idempotency_key) WHERE idempotency_key IS NOT NULL;
            ",
        )?;
        self.ensure_column("workflows", "summary", "TEXT NOT NULL DEFAULT ''")?;
        self.ensure_column(
            "workflows",
            "created_by",
            "TEXT NOT NULL DEFAULT 'architect-agent'",
        )?;
        self.ensure_column("workflows", "inputs_json", "TEXT NOT NULL DEFAULT '{}'")?;
        self.ensure_column("workflows", "result_json", "TEXT")?;
        self.ensure_column("workflows", "error_json", "TEXT")?;
        self.ensure_column("workflows", "started_at", "TEXT")?;
        self.ensure_column("workflows", "completed_at", "TEXT")?;
        self.ensure_default_organization()?;
        self.ensure_default_settings()?;
        let policy_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM security_policies WHERE project_id = ?1",
            params![PROJECT_ID],
            |row| row.get(0),
        )?;
        if policy_count == 0 {
            self.conn.execute(
                "INSERT INTO security_policies (project_id, policy_json, updated_at) VALUES (?1, ?2, ?3)",
                params![PROJECT_ID, serde_json::to_string(&default_security_policy())?, now()],
            )?;
        }
        let scheduler_config_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM scheduler_configs WHERE project_id = ?1",
            params![PROJECT_ID],
            |row| row.get(0),
        )?;
        if scheduler_config_count == 0 {
            self.conn.execute(
                "INSERT INTO scheduler_configs (project_id, config_json, updated_at) VALUES (?1, ?2, ?3)",
                params![PROJECT_ID, serde_json::to_string(&default_scheduler_config())?, now()],
            )?;
        }
        self.seed_workflow_templates()?;
        self.seed_task_templates()?;
        Ok(())
    }

    fn ensure_column(&self, table: &str, column: &str, definition: &str) -> anyhow::Result<()> {
        let mut stmt = self.conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        let columns = rows.collect::<Result<Vec<_>, _>>()?;
        if !columns.iter().any(|name| name == column) {
            self.conn.execute(
                &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
                [],
            )?;
        }
        Ok(())
    }

    fn ensure_default_organization(&self) -> anyhow::Result<()> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM organizations WHERE project_id = ?1",
            params![PROJECT_ID],
            |row| row.get(0),
        )?;
        if count == 0 {
            let now = now();
            self.conn.execute(
                "
                INSERT INTO organizations (id, project_id, name, slug, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?5)
                ",
                params![
                    "org_agentgrid_default",
                    PROJECT_ID,
                    "AgentGrid 默认组织",
                    "default",
                    now
                ],
            )?;
        }
        Ok(())
    }

    fn ensure_default_settings(&self) -> anyhow::Result<()> {
        let defaults = [
            (
                "hub.public_url",
                json!("http://chenqi.tminos.com:20080/agentgrid"),
            ),
            ("smtp", default_smtp_setting()),
            ("registration.enabled", json!(true)),
        ];
        let now = now();
        for (key, value) in defaults {
            self.conn.execute(
                "
                INSERT INTO hub_settings (key, value_json, updated_at)
                VALUES (?1, ?2, ?3)
                ON CONFLICT(key) DO NOTHING
                ",
                params![key, serde_json::to_string(&value)?, now],
            )?;
        }
        Ok(())
    }

    fn bootstrap_status(&self) -> anyhow::Result<Value> {
        let super_admin_count = self.count_super_admins()?;
        Ok(json!({
            "ok": true,
            "needs_bootstrap": super_admin_count == 0,
            "super_admin_count": super_admin_count,
            "organization": self.default_organization()?,
            "settings": self.system_settings_public()?
        }))
    }

    fn auth_state(&self, token: Option<&str>) -> anyhow::Result<Value> {
        if let Some(user) = token
            .filter(|value| !value.trim().is_empty())
            .map(|value| self.user_by_session_token(value))
            .transpose()?
            .flatten()
        {
            return Ok(json!({
                "ok": true,
                "authenticated": true,
                "needs_bootstrap": self.count_super_admins()? == 0,
                "user": user_public(user),
                "organization": self.default_organization()?,
                "settings": self.system_settings_public()?
            }));
        }
        Ok(json!({
            "ok": true,
            "authenticated": false,
            "needs_bootstrap": self.count_super_admins()? == 0,
            "organization": self.default_organization()?,
            "settings": self.system_settings_public()?
        }))
    }

    fn create_super_admin(&self, data: Value) -> anyhow::Result<Value> {
        if self.count_super_admins()? > 0 {
            anyhow::bail!("super admin already exists");
        }
        let email = required_string(&data, "email")?.to_ascii_lowercase();
        let name = string_or(&data, "name", "超级管理员");
        let password = required_string(&data, "password")?;
        if password.len() < 8 {
            anyhow::bail!("password must be at least 8 characters");
        }
        let org_id = self.default_organization_id()?;
        let user_id = new_id("user");
        let now = now();
        self.conn.execute(
            "
            INSERT INTO hub_users (
                id, project_id, organization_id, email, name, role, password_hash,
                status, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, 'super_admin', ?6, 'active', ?7, ?7)
            ",
            params![
                user_id,
                PROJECT_ID,
                org_id,
                email,
                name,
                user_password_hash(&email, &password),
                now
            ],
        )?;
        self.audit(
            "hub.super_admin.created",
            &email,
            Some(&user_id),
            "Hub 唯一超级管理员已初始化",
            json!({ "user_id": user_id, "email": email }),
        )?;
        Ok(self.login_user(json!({ "email": email, "password": password }))?)
    }

    fn login_user(&self, data: Value) -> anyhow::Result<Value> {
        let email = required_string(&data, "email")?.to_ascii_lowercase();
        let password = required_string(&data, "password")?;
        let Some(user) = self.user_by_email(&email)? else {
            anyhow::bail!("invalid email or password");
        };
        if user.pointer("/status/state").and_then(Value::as_str) != Some("active") {
            anyhow::bail!("user is not active");
        }
        let expected = user
            .pointer("/credentials/password_hash")
            .and_then(Value::as_str)
            .unwrap_or("");
        if expected != user_password_hash(&email, &password) {
            anyhow::bail!("invalid email or password");
        }
        let token = generate_session_token();
        let session_id = new_id("sess");
        let now = now();
        let expires_at = (Utc::now() + chrono::Duration::days(30))
            .to_rfc3339_opts(chrono::SecondsFormat::Micros, true);
        self.conn.execute(
            "
            INSERT INTO user_sessions (id, project_id, user_id, token_hash, expires_at, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                session_id,
                PROJECT_ID,
                user.pointer("/metadata/id")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
                session_token_hash(&token),
                expires_at,
                now,
            ],
        )?;
        Ok(json!({
            "ok": true,
            "token": token,
            "session": {
                "id": session_id,
                "expires_at": expires_at
            },
            "user": user_public(user),
            "organization": self.default_organization()?,
            "settings": self.system_settings_public()?
        }))
    }

    fn request_register_code(&self, data: Value) -> anyhow::Result<Value> {
        if !self.registration_enabled()? {
            anyhow::bail!("registration is disabled");
        }
        let email = required_string(&data, "email")?.to_ascii_lowercase();
        if self.user_by_email(&email)?.is_some() {
            anyhow::bail!("email already registered");
        }
        let code = generate_email_code();
        let now = now();
        let expires_at = (Utc::now() + chrono::Duration::minutes(10))
            .to_rfc3339_opts(chrono::SecondsFormat::Micros, true);
        self.conn.execute(
            "
            INSERT INTO email_verification_codes (
                id, project_id, email, code_hash, purpose, expires_at, created_at
            ) VALUES (?1, ?2, ?3, ?4, 'register', ?5, ?6)
            ",
            params![
                new_id("ecode"),
                PROJECT_ID,
                email,
                email_code_hash(&email, &code),
                expires_at,
                now
            ],
        )?;
        let sent = self.send_email_code(&email, &code).unwrap_or_else(|error| {
            json!({
                "sent": false,
                "error": error.to_string()
            })
        });
        Ok(json!({
            "ok": true,
            "email": email,
            "expires_at": expires_at,
            "delivery": sent
        }))
    }

    fn register_user(&self, data: Value) -> anyhow::Result<Value> {
        if !self.registration_enabled()? {
            anyhow::bail!("registration is disabled");
        }
        let email = required_string(&data, "email")?.to_ascii_lowercase();
        let name = string_or(&data, "name", &email);
        let password = required_string(&data, "password")?;
        let code = required_string(&data, "code")?;
        if password.len() < 8 {
            anyhow::bail!("password must be at least 8 characters");
        }
        self.consume_email_code(&email, &code, "register")?;
        let org_id = self.default_organization_id()?;
        let user_id = new_id("user");
        let now = now();
        self.conn.execute(
            "
            INSERT INTO hub_users (
                id, project_id, organization_id, email, name, role, password_hash,
                status, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, 'member', ?6, 'active', ?7, ?7)
            ",
            params![
                user_id,
                PROJECT_ID,
                org_id,
                email,
                name,
                user_password_hash(&email, &password),
                now
            ],
        )?;
        self.audit(
            "hub.user.registered",
            &email,
            Some(&user_id),
            "Hub 用户已通过邮箱验证码注册",
            json!({ "user_id": user_id, "email": email }),
        )?;
        self.login_user(json!({ "email": email, "password": password }))
    }

    fn change_password(&self, data: Value) -> anyhow::Result<Value> {
        let email = required_string(&data, "email")?.to_ascii_lowercase();
        let old_password = required_string(&data, "old_password")?;
        let new_password = required_string(&data, "new_password")?;
        if new_password.len() < 8 {
            anyhow::bail!("password must be at least 8 characters");
        }
        let Some(user) = self.user_by_email(&email)? else {
            anyhow::bail!("invalid email or password");
        };
        let expected = user
            .pointer("/credentials/password_hash")
            .and_then(Value::as_str)
            .unwrap_or("");
        if expected != user_password_hash(&email, &old_password) {
            anyhow::bail!("invalid email or password");
        }
        let now = now();
        self.conn.execute(
            "
            UPDATE hub_users
            SET password_hash = ?1, updated_at = ?2
            WHERE project_id = ?3 AND email = ?4
            ",
            params![
                user_password_hash(&email, &new_password),
                now,
                PROJECT_ID,
                email
            ],
        )?;
        self.audit(
            "hub.user.password_changed",
            &email,
            user.pointer("/metadata/id").and_then(Value::as_str),
            "Hub 用户密码已修改",
            json!({ "email": email }),
        )?;
        Ok(json!({ "ok": true }))
    }

    fn system_settings(&self) -> anyhow::Result<Value> {
        Ok(json!({
            "ok": true,
            "item": self.system_settings_public()?
        }))
    }

    fn update_system_settings(&self, data: Value) -> anyhow::Result<Value> {
        let now = now();
        if let Some(value) = data.get("hub_public_url") {
            self.upsert_setting("hub.public_url", value.clone(), &now)?;
        }
        if let Some(value) = data.get("registration_enabled") {
            self.upsert_setting("registration.enabled", value.clone(), &now)?;
        }
        if let Some(value) = data.get("smtp") {
            let mut smtp = value.clone();
            if let Some(map) = smtp.as_object_mut() {
                let password_is_blank = map
                    .get("password")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .unwrap_or("")
                    .is_empty();
                if password_is_blank {
                    if let Some(existing_password) = self
                        .setting_value("smtp")?
                        .and_then(|value| {
                            value
                                .get("password")
                                .and_then(Value::as_str)
                                .map(ToString::to_string)
                        })
                        .filter(|value| !value.trim().is_empty())
                    {
                        map.insert("password".to_string(), json!(existing_password));
                    }
                }
            }
            self.upsert_setting("smtp", smtp, &now)?;
        }
        self.audit(
            "hub.settings.changed",
            "super-admin",
            Some(PROJECT_ID),
            "Hub 系统设置已更新",
            json!({ "input": data }),
        )?;
        self.system_settings()
    }

    fn upsert_setting(&self, key: &str, value: Value, now: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "
            INSERT INTO hub_settings (key, value_json, updated_at)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(key) DO UPDATE SET
                value_json = excluded.value_json,
                updated_at = excluded.updated_at
            ",
            params![key, serde_json::to_string(&value)?, now],
        )?;
        Ok(())
    }

    fn seed(&self) -> anyhow::Result<()> {
        let now = now();
        for agent in seed_agents() {
            self.conn.execute(
                "
                INSERT INTO agents (
                    id, project_id, name, role, skills_json, permissions_json, responsibility,
                    auth_type, token_hash, token_hint, credential_status, account_username,
                    credential_refs_json, node_scope_json, tool_scope_json,
                    status, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 'online', ?16, ?16)
                ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    role = excluded.role,
                    skills_json = excluded.skills_json,
                    permissions_json = excluded.permissions_json,
                    responsibility = excluded.responsibility,
                    auth_type = excluded.auth_type,
                    token_hash = COALESCE(agents.token_hash, excluded.token_hash),
                    token_hint = CASE
                        WHEN agents.token_hash IS NULL OR agents.token_hash = '' THEN excluded.token_hint
                        ELSE agents.token_hint
                    END,
                    credential_status = CASE
                        WHEN agents.credential_status = 'active' THEN agents.credential_status
                        ELSE excluded.credential_status
                    END,
                    account_username = excluded.account_username,
                    credential_refs_json = excluded.credential_refs_json,
                    node_scope_json = excluded.node_scope_json,
                    tool_scope_json = excluded.tool_scope_json,
                    updated_at = excluded.updated_at
                ",
                params![
                    agent.id,
                    PROJECT_ID,
                    agent.name,
                    agent.role,
                    serde_json::to_string(agent.skills)?,
                    serde_json::to_string(agent.permissions)?,
                    agent.responsibility,
                    agent.auth_type,
                    agent
                        .bootstrap_token
                        .map(|token| agent_token_hash(agent.id, token)),
                    agent.bootstrap_token.map(token_hint).unwrap_or_default(),
                    agent.credential_status,
                    agent.account_username,
                    serde_json::to_string(&agent.credential_refs)?,
                    serde_json::to_string(&agent.node_scope)?,
                    serde_json::to_string(&agent.tool_scope)?,
                    now,
                ],
            )?;
        }
        let node_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM nodes WHERE project_id = ?1",
            params![PROJECT_ID],
            |row| row.get(0),
        )?;
        if node_count == 0 {
            self.upsert_node(json!({
                "id": "chenqi-linux-01",
                "name": "chenqi Linux 节点",
                "os": "linux",
                "arch": "unknown",
                "address": "chenqi.tminos.com",
                "tags": ["server", "linux"],
                "capabilities": ["http", "command", "agentmessage"],
                "groups": ["default", "linux"],
                "weight": 1,
                "max_concurrent_jobs": 1,
                "cpu_cores": 0,
                "memory_mb": 0,
                "cpu_usage_percent": 0,
                "memory_used_mb": 0,
                "disk_total_mb": 0,
                "disk_free_mb": 0,
                "status": "online"
            }))?;
        }
        let msg_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM agent_messages WHERE project_id = ?1",
            params![PROJECT_ID],
            |row| row.get(0),
        )?;
        if msg_count == 0 {
            self.create_message(json!({
                "from": "architect-agent",
                "to": ["worker-agent", "qa-agent"],
                "type": "broadcast.notice",
                "subject": "AgentGrid Rust Hub 已启动",
                "summary": "核心 Hub 已切换为 Rust 服务。",
                "payload": { "runtime": "rust" }
            }))?;
        }
        Ok(())
    }

    fn seed_workflow_templates(&self) -> anyhow::Result<()> {
        for template in default_workflow_templates() {
            self.create_workflow_template_if_missing(template)?;
        }
        Ok(())
    }

    fn seed_task_templates(&self) -> anyhow::Result<()> {
        for template in default_task_templates() {
            self.create_task_template_if_missing(template)?;
        }
        Ok(())
    }

    fn count_super_admins(&self) -> anyhow::Result<i64> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM hub_users WHERE project_id = ?1 AND role = 'super_admin'",
                params![PROJECT_ID],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    fn default_organization_id(&self) -> anyhow::Result<String> {
        self.conn
            .query_row(
                "SELECT id FROM organizations WHERE project_id = ?1 ORDER BY created_at ASC LIMIT 1",
                params![PROJECT_ID],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    fn default_organization(&self) -> anyhow::Result<Value> {
        self.conn
            .query_row(
                "SELECT id, name, slug, created_at, updated_at FROM organizations WHERE project_id = ?1 ORDER BY created_at ASC LIMIT 1",
                params![PROJECT_ID],
                |row| {
                    Ok(json!({
                        "id": row.get::<_, String>(0)?,
                        "name": row.get::<_, String>(1)?,
                        "slug": row.get::<_, String>(2)?,
                        "created_at": row.get::<_, String>(3)?,
                        "updated_at": row.get::<_, String>(4)?
                    }))
                },
            )
            .map_err(Into::into)
    }

    fn registration_enabled(&self) -> anyhow::Result<bool> {
        Ok(self
            .setting_value("registration.enabled")?
            .and_then(|value| value.as_bool())
            .unwrap_or(true))
    }

    fn setting_value(&self, key: &str) -> anyhow::Result<Option<Value>> {
        self.conn
            .query_row(
                "SELECT value_json FROM hub_settings WHERE key = ?1",
                params![key],
                |row| {
                    let raw: String = row.get(0)?;
                    Ok(serde_json::from_str(&raw).unwrap_or(Value::Null))
                },
            )
            .optional()
            .map_err(Into::into)
    }

    fn system_settings_full(&self) -> anyhow::Result<Value> {
        let hub_public_url = self
            .setting_value("hub.public_url")?
            .unwrap_or_else(|| json!("http://chenqi.tminos.com:20080/agentgrid"));
        let smtp = self
            .setting_value("smtp")?
            .unwrap_or_else(default_smtp_setting);
        let registration_enabled = self
            .setting_value("registration.enabled")?
            .unwrap_or_else(|| json!(true));
        Ok(json!({
            "hub_public_url": hub_public_url,
            "smtp": smtp,
            "registration_enabled": registration_enabled,
        }))
    }

    fn system_settings_public(&self) -> anyhow::Result<Value> {
        let mut settings = self.system_settings_full()?;
        if let Some(map) = settings.get_mut("smtp").and_then(Value::as_object_mut) {
            let password = map
                .get("password")
                .and_then(Value::as_str)
                .map(token_hint)
                .unwrap_or_default();
            map.insert("password_hint".to_string(), json!(password));
            map.remove("password");
        }
        Ok(settings)
    }

    fn user_by_email(&self, email: &str) -> anyhow::Result<Option<Value>> {
        self.conn
            .query_row(
                "SELECT * FROM hub_users WHERE project_id = ?1 AND email = ?2",
                params![PROJECT_ID, email],
                hub_user_row,
            )
            .optional()
            .map_err(Into::into)
    }

    fn user_by_session_token(&self, token: &str) -> anyhow::Result<Option<Value>> {
        let token_hash = session_token_hash(token);
        let now = now();
        self.conn
            .query_row(
                "
                SELECT hub_users.*
                FROM user_sessions
                JOIN hub_users ON hub_users.id = user_sessions.user_id
                WHERE user_sessions.project_id = ?1
                  AND user_sessions.token_hash = ?2
                  AND user_sessions.expires_at > ?3
                  AND hub_users.status = 'active'
                LIMIT 1
                ",
                params![PROJECT_ID, token_hash, now],
                hub_user_row,
            )
            .optional()
            .map_err(Into::into)
    }

    fn consume_email_code(&self, email: &str, code: &str, purpose: &str) -> anyhow::Result<()> {
        let now = now();
        let code_hash = email_code_hash(email, code);
        let id = self
            .conn
            .query_row(
                "
                SELECT id FROM email_verification_codes
                WHERE project_id = ?1
                  AND email = ?2
                  AND purpose = ?3
                  AND code_hash = ?4
                  AND consumed_at IS NULL
                  AND expires_at > ?5
                ORDER BY created_at DESC
                LIMIT 1
                ",
                params![PROJECT_ID, email, purpose, code_hash, now],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| anyhow::anyhow!("invalid or expired verification code"))?;
        self.conn.execute(
            "UPDATE email_verification_codes SET consumed_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }

    fn smtp_config(&self) -> anyhow::Result<SmtpConfig> {
        let value = self
            .setting_value("smtp")?
            .unwrap_or_else(default_smtp_setting);
        Ok(SmtpConfig {
            host: value
                .get("host")
                .and_then(Value::as_str)
                .unwrap_or("smtp.qq.com")
                .to_string(),
            port: value.get("port").and_then(Value::as_u64).unwrap_or(465) as u16,
            username: value
                .get("username")
                .and_then(Value::as_str)
                .unwrap_or("1668217900@qq.com")
                .to_string(),
            password: value
                .get("password")
                .and_then(Value::as_str)
                .unwrap_or("oebnbqxrirmybacd")
                .to_string(),
            from: value
                .get("from")
                .and_then(Value::as_str)
                .unwrap_or("1668217900@qq.com")
                .to_string(),
        })
    }

    fn send_email_code(&self, email: &str, code: &str) -> anyhow::Result<Value> {
        let smtp = self.smtp_config()?;
        let temp_path =
            std::env::temp_dir().join(format!("agentgrid-email-{}.eml", Uuid::new_v4()));
        let subject =
            base64::engine::general_purpose::STANDARD.encode("AgentGrid 注册验证码".as_bytes());
        let body = format!("您的 AgentGrid 注册验证码是：{code}\n\n10 分钟内有效。\n");
        let message = format!(
            "From: AgentGrid <{}>\r\nTo: <{}>\r\nSubject: =?UTF-8?B?{}?=\r\nContent-Type: text/plain; charset=UTF-8\r\n\r\n{}",
            smtp.from, email, subject, body
        );
        fs::write(&temp_path, message)?;
        let url = format!("smtps://{}:{}", smtp.host, smtp.port);
        let output = Command::new("curl")
            .arg("--silent")
            .arg("--show-error")
            .arg("--fail")
            .arg("--url")
            .arg(&url)
            .arg("--ssl-reqd")
            .arg("--mail-from")
            .arg(&smtp.from)
            .arg("--mail-rcpt")
            .arg(email)
            .arg("--user")
            .arg(format!("{}:{}", smtp.username, smtp.password))
            .arg("--upload-file")
            .arg(&temp_path)
            .output();
        let _ = fs::remove_file(&temp_path);
        let output = output?;
        if !output.status.success() {
            anyhow::bail!(
                "smtp curl failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(json!({ "sent": true, "transport": "curl-smtps" }))
    }

    fn list_agents(&self) -> anyhow::Result<Vec<Value>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM agents WHERE project_id = ?1 ORDER BY name ASC")?;
        let rows = stmt.query_map(params![PROJECT_ID], agent_row)?;
        collect_values(rows)
    }

    fn upsert_agent(&self, data: Value) -> anyhow::Result<Value> {
        let now = now();
        let id = string_or(&data, "id", &new_id("agent"));
        let token = optional_string(&data, "token");
        let existing = self.get_agent(&id)?;
        let existing_has_token = existing
            .as_ref()
            .and_then(|agent| agent.pointer("/credentials/token_configured"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let token_hash = token.as_deref().map(|value| agent_token_hash(&id, value));
        let token_hint_value = token.as_deref().map(token_hint).unwrap_or_else(|| {
            existing
                .as_ref()
                .and_then(|agent| agent.pointer("/credentials/token_hint"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        });
        let credential_status = string_or(
            &data,
            "credential_status",
            if token.is_some() || existing_has_token {
                "active"
            } else {
                "not_configured"
            },
        );
        self.conn.execute(
            "
            INSERT INTO agents (
                id, project_id, name, role, skills_json, permissions_json, responsibility,
                auth_type, token_hash, token_hint, credential_status, account_username,
                credential_refs_json, node_scope_json, tool_scope_json,
                status, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?17)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                role = excluded.role,
                skills_json = excluded.skills_json,
                permissions_json = excluded.permissions_json,
                responsibility = excluded.responsibility,
                auth_type = excluded.auth_type,
                token_hash = COALESCE(excluded.token_hash, agents.token_hash),
                token_hint = CASE
                    WHEN excluded.token_hash IS NULL THEN agents.token_hint
                    ELSE excluded.token_hint
                END,
                credential_status = excluded.credential_status,
                account_username = excluded.account_username,
                credential_refs_json = excluded.credential_refs_json,
                node_scope_json = excluded.node_scope_json,
                tool_scope_json = excluded.tool_scope_json,
                status = excluded.status,
                updated_at = excluded.updated_at
            ",
            params![
                id,
                PROJECT_ID,
                required_string(&data, "name")?,
                required_string(&data, "role")?,
                serde_json::to_string(&array_field(&data, "skills"))?,
                serde_json::to_string(&array_field(&data, "permissions"))?,
                string_or(&data, "responsibility", ""),
                string_or(&data, "auth_type", "bearer_token"),
                token_hash,
                token_hint_value,
                credential_status,
                string_or(&data, "account_username", ""),
                serde_json::to_string(data.get("credential_refs").unwrap_or(&json!({})))?,
                serde_json::to_string(&normalize_agent_node_scope(data.get("node_scope")))?,
                serde_json::to_string(&normalize_agent_tool_scope(data.get("tool_scope")))?,
                string_or(&data, "status", "online"),
                now,
            ],
        )?;
        self.get_agent(&id)?
            .ok_or_else(|| anyhow::anyhow!("agent not found"))
    }

    fn get_agent(&self, id: &str) -> anyhow::Result<Option<Value>> {
        self.conn
            .query_row("SELECT * FROM agents WHERE id = ?1", params![id], agent_row)
            .optional()
            .map_err(Into::into)
    }

    fn list_nodes(&self) -> anyhow::Result<Vec<Value>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM nodes WHERE project_id = ?1 ORDER BY status ASC, updated_at DESC",
        )?;
        let rows = stmt.query_map(params![PROJECT_ID], node_row)?;
        let mut nodes = collect_values(rows)?;
        for node in &mut nodes {
            let Some(node_id) = node.pointer("/metadata/id").and_then(Value::as_str) else {
                continue;
            };
            let tools = self.list_node_tools(Some(node_id))?;
            if let Some(spec) = node.get_mut("spec").and_then(Value::as_object_mut) {
                spec.insert("node_tools".to_string(), Value::Array(tools));
            }
        }
        Ok(nodes)
    }

    fn list_tool_probes(&self, limit: u16) -> anyhow::Result<Vec<Value>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT * FROM tool_probes
            WHERE project_id = ?1
            ORDER BY updated_at DESC
            LIMIT ?2
            ",
        )?;
        let rows = stmt.query_map(params![PROJECT_ID, limit.min(1000)], tool_probe_row)?;
        collect_values(rows)
    }

    fn get_tool_probe(&self, tool_id: &str, node_id: &str) -> anyhow::Result<Option<Value>> {
        self.conn
            .query_row(
                "SELECT * FROM tool_probes WHERE project_id = ?1 AND tool_id = ?2 AND node_id = ?3",
                params![PROJECT_ID, tool_id, node_id],
                tool_probe_row,
            )
            .optional()
            .map_err(Into::into)
    }

    fn create_tool_probe_tasks(
        &self,
        tool_id: Option<&str>,
        node_id: Option<&str>,
    ) -> anyhow::Result<Vec<Value>> {
        let nodes = self.list_nodes()?;
        let tools = self
            .tool_registry_with_dynamic()?
            .into_iter()
            .filter(|tool| {
                tool_id
                    .map(|id| tool.get("id").and_then(Value::as_str) == Some(id))
                    .unwrap_or(true)
            })
            .collect::<Vec<_>>();
        if tool_id.is_some() && tools.is_empty() {
            anyhow::bail!("tool not found");
        }

        let mut created = Vec::new();
        for tool in tools {
            let current_nodes = nodes_for_tool(&tool, &nodes)
                .into_iter()
                .filter(|node| {
                    node_id
                        .map(|id| node.get("id").and_then(Value::as_str) == Some(id))
                        .unwrap_or(true)
                })
                .collect::<Vec<_>>();
            if node_id.is_some() && current_nodes.is_empty() {
                continue;
            }
            for node in current_nodes {
                let tool_id = tool
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow::anyhow!("tool id missing"))?;
                let node_id = node
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow::anyhow!("node id missing"))?;
                let Some(payload) = probe_payload_for_tool(tool_id) else {
                    self.upsert_tool_probe_record(
                        tool_id,
                        node_id,
                        None,
                        "unsupported",
                        None,
                        Some(json!({
                            "code": "probe_not_supported",
                            "message": "该工具暂未定义轻量 Probe payload"
                        })),
                    )?;
                    continue;
                };
                let title = format!("Tool Probe {tool_id} on {node_id}");
                let task = self.create_task(json!({
                    "title": title,
                    "summary": "AgentGrid Tool Probe v1 自动能力验证任务。",
                    "created_by": "tool-probe-engine",
                    "owner": "worker-agent",
                    "assigned_to": ["worker-agent"],
                    "priority": "low",
                    "labels": probe_labels_for_tool(tool_id, node_id),
                    "inputs": [serde_json::to_string_pretty(&payload)?],
                    "outputs": ["Probe 执行结果", "验证状态"],
                    "acceptance_criteria": [
                        "Worker 能按工具协议执行轻量 Probe",
                        "Hub 能根据结果更新工具验证状态"
                    ],
                    "correlation_id": format!("tool_probe:{tool_id}:{node_id}")
                }))?;
                let task_id = task
                    .item
                    .pointer("/metadata/id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow::anyhow!("probe task id missing"))?;
                self.upsert_tool_probe_record(
                    tool_id,
                    node_id,
                    Some(task_id),
                    "pending",
                    None,
                    None,
                )?;
                created.push(json!({
                    "tool_id": tool_id,
                    "node_id": node_id,
                    "task_id": task_id,
                    "status": "pending"
                }));
            }
        }
        Ok(created)
    }

    fn upsert_tool_probe_record(
        &self,
        tool_id: &str,
        node_id: &str,
        task_id: Option<&str>,
        status: &str,
        result: Option<Value>,
        error: Option<Value>,
    ) -> anyhow::Result<()> {
        let now = now();
        let completed_at = if matches!(status, "verified" | "failed" | "unsupported") {
            Some(now.clone())
        } else {
            None
        };
        let expires_at = if status == "verified" {
            Some(
                (Utc::now() + chrono::Duration::hours(24))
                    .to_rfc3339_opts(chrono::SecondsFormat::Micros, true),
            )
        } else {
            None
        };
        self.conn.execute(
            "
            INSERT INTO tool_probes (
                id, project_id, tool_id, node_id, task_id, status, support_basis,
                started_at, completed_at, expires_at, result_json, error_json, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'runtime_probe', ?7, ?8, ?9, ?10, ?11, ?12, ?12)
            ON CONFLICT(project_id, tool_id, node_id) DO UPDATE SET
                task_id = COALESCE(excluded.task_id, tool_probes.task_id),
                status = excluded.status,
                support_basis = excluded.support_basis,
                started_at = COALESCE(tool_probes.started_at, excluded.started_at),
                completed_at = excluded.completed_at,
                expires_at = excluded.expires_at,
                result_json = excluded.result_json,
                error_json = excluded.error_json,
                updated_at = excluded.updated_at
            ",
            params![
                new_id("probe"),
                PROJECT_ID,
                tool_id,
                node_id,
                task_id,
                status,
                now,
                completed_at,
                expires_at,
                serde_json::to_string(&result.unwrap_or(Value::Null))?,
                serde_json::to_string(&error.unwrap_or(Value::Null))?,
                now,
            ],
        )?;
        Ok(())
    }

    fn upsert_node(&self, data: Value) -> anyhow::Result<Value> {
        let now = now();
        let id = string_or(&data, "id", &new_id("node"));
        let auth = self.authorize_node_heartbeat(&id, &data, &now)?;
        self.conn.execute(
            "
            INSERT INTO nodes (
                id, project_id, name, os, arch, address, tags_json, capabilities_json,
                groups_json, weight, max_concurrent_jobs,
                cpu_cores, memory_mb, cpu_usage_percent, memory_used_mb, disk_total_mb, disk_free_mb,
                running_jobs, worker_version, worker_target, glibc_version,
                machine_fingerprint, join_token_hash, join_token_hint, auth_status, authorized_at,
                auto_update_enabled, update_channel,
                status, created_at, updated_at, last_heartbeat_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?29, ?30)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                os = excluded.os,
                arch = excluded.arch,
                address = excluded.address,
                tags_json = excluded.tags_json,
                capabilities_json = excluded.capabilities_json,
                groups_json = excluded.groups_json,
                weight = excluded.weight,
                max_concurrent_jobs = excluded.max_concurrent_jobs,
                cpu_cores = excluded.cpu_cores,
                memory_mb = excluded.memory_mb,
                cpu_usage_percent = excluded.cpu_usage_percent,
                memory_used_mb = excluded.memory_used_mb,
                disk_total_mb = excluded.disk_total_mb,
                disk_free_mb = excluded.disk_free_mb,
                running_jobs = excluded.running_jobs,
                worker_version = excluded.worker_version,
                worker_target = excluded.worker_target,
                glibc_version = excluded.glibc_version,
                machine_fingerprint = COALESCE(excluded.machine_fingerprint, nodes.machine_fingerprint),
                join_token_hash = COALESCE(nodes.join_token_hash, excluded.join_token_hash),
                join_token_hint = CASE
                    WHEN nodes.join_token_hint = '' THEN excluded.join_token_hint
                    ELSE nodes.join_token_hint
                END,
                auth_status = excluded.auth_status,
                authorized_at = COALESCE(nodes.authorized_at, excluded.authorized_at),
                auto_update_enabled = excluded.auto_update_enabled,
                update_channel = excluded.update_channel,
                status = excluded.status,
                updated_at = excluded.updated_at,
                last_heartbeat_at = excluded.last_heartbeat_at
            ",
            params![
                id,
                PROJECT_ID,
                required_string(&data, "name")?,
                string_or(&data, "os", "unknown"),
                string_or(&data, "arch", "unknown"),
                string_or(&data, "address", ""),
                serde_json::to_string(&array_field(&data, "tags"))?,
                serde_json::to_string(&array_field(&data, "capabilities"))?,
                serde_json::to_string(&array_field(&data, "groups"))?,
                float_or(&data, "weight", 1.0),
                number_or(&data, "max_concurrent_jobs", 1),
                number_or(&data, "cpu_cores", 0),
                number_or(&data, "memory_mb", 0),
                float_or(&data, "cpu_usage_percent", 0.0),
                number_or(&data, "memory_used_mb", 0),
                number_or(&data, "disk_total_mb", 0),
                number_or(&data, "disk_free_mb", 0),
                number_or(&data, "running_jobs", 0),
                optional_string(&data, "worker_version"),
                optional_string(&data, "worker_target"),
                optional_string(&data, "glibc_version"),
                auth.machine_fingerprint,
                auth.join_token_hash,
                auth.join_token_hint,
                auth.status,
                auth.authorized_at,
                bool_or(&data, "auto_update_enabled", true) as i64,
                string_or(&data, "update_channel", "stable"),
                string_or(&data, "status", "online"),
                now,
                string_or(&data, "last_heartbeat_at", &now),
            ],
        )?;
        let item = self
            .get_node(&id)?
            .ok_or_else(|| anyhow::anyhow!("node not found"))?;
        self.requeue_lost_job_attempts()?;
        self.audit(
            "node.heartbeat",
            &id,
            Some(&id),
            "节点心跳更新",
            json!({ "node": item.clone() }),
        )?;
        Ok(item)
    }

    fn authorize_node_heartbeat(
        &self,
        node_id: &str,
        data: &Value,
        now: &str,
    ) -> anyhow::Result<NodeAuthorization> {
        let machine_fingerprint =
            optional_string(data, "machine_fingerprint").filter(|value| !value.trim().is_empty());
        let join_token =
            optional_string(data, "join_token").filter(|value| !value.trim().is_empty());
        let existing = self.get_node_auth_record(node_id)?;
        if let Some(existing) = existing {
            if existing.previous_status == "pending" {
                return Ok(NodeAuthorization {
                    status: "pending".to_string(),
                    previous_status: existing.previous_status,
                    machine_fingerprint: machine_fingerprint.or(existing.machine_fingerprint),
                    join_token_hash: existing.join_token_hash,
                    join_token_hint: existing.join_token_hint,
                    authorized_at: existing.authorized_at,
                });
            }
            if existing.previous_status == "legacy" && existing.join_token_hash.is_none() {
                return Ok(NodeAuthorization {
                    status: "legacy".to_string(),
                    previous_status: existing.previous_status,
                    machine_fingerprint: machine_fingerprint.or(existing.machine_fingerprint),
                    join_token_hash: None,
                    join_token_hint: String::new(),
                    authorized_at: existing.authorized_at,
                });
            }
            let token = join_token
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("node join token required"))?;
            let token_hash = node_join_token_hash(node_id, token);
            if existing.join_token_hash.as_deref() != Some(token_hash.as_str()) {
                anyhow::bail!("node join token rejected");
            }
            let fingerprint = machine_fingerprint
                .clone()
                .ok_or_else(|| anyhow::anyhow!("machine fingerprint required"))?;
            if let Some(bound) = existing.machine_fingerprint.as_deref() {
                if bound != fingerprint {
                    anyhow::bail!("machine fingerprint mismatch for node");
                }
            }
            return Ok(NodeAuthorization {
                status: "bound".to_string(),
                previous_status: existing.previous_status,
                machine_fingerprint: Some(fingerprint),
                join_token_hash: Some(token_hash),
                join_token_hint: existing.join_token_hint,
                authorized_at: existing.authorized_at.or_else(|| Some(now.to_string())),
            });
        }

        let Some(token) = join_token.as_deref() else {
            return Ok(NodeAuthorization {
                status: "pending".to_string(),
                previous_status: "new".to_string(),
                machine_fingerprint,
                join_token_hash: None,
                join_token_hint: String::new(),
                authorized_at: None,
            });
        };
        let token_hash = node_join_token_hash(node_id, token);
        self.upsert_node_join_request(
            node_id,
            data,
            machine_fingerprint.as_deref(),
            &token_hash,
            token,
            now,
        )?;
        Ok(NodeAuthorization {
            status: "pending".to_string(),
            previous_status: "new".to_string(),
            machine_fingerprint,
            join_token_hash: Some(token_hash),
            join_token_hint: token_hint(token),
            authorized_at: None,
        })
    }

    fn get_node_auth_record(&self, node_id: &str) -> anyhow::Result<Option<NodeAuthorization>> {
        self.conn
            .query_row(
                "
                SELECT machine_fingerprint, join_token_hash, join_token_hint, auth_status, authorized_at
                FROM nodes
                WHERE project_id = ?1 AND id = ?2
                ",
                params![PROJECT_ID, node_id],
                |row| {
                    Ok(NodeAuthorization {
                        machine_fingerprint: row.get::<_, Option<String>>(0)?,
                        join_token_hash: row.get::<_, Option<String>>(1)?,
                        join_token_hint: row.get::<_, String>(2)?,
                        previous_status: row.get::<_, String>(3)?,
                        authorized_at: row.get::<_, Option<String>>(4)?,
                        status: row.get::<_, String>(3)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    fn upsert_node_join_request(
        &self,
        node_id: &str,
        data: &Value,
        machine_fingerprint: Option<&str>,
        token_hash: &str,
        token: &str,
        now: &str,
    ) -> anyhow::Result<()> {
        let node_name = string_or(data, "name", node_id);
        let os = string_or(data, "os", "unknown");
        let arch = string_or(data, "arch", "unknown");
        let hub_url = self
            .setting_value("hub.public_url")?
            .and_then(|value| value.as_str().map(ToString::to_string))
            .unwrap_or_else(|| "http://chenqi.tminos.com:20080/agentgrid".to_string());
        self.conn.execute(
            "
            INSERT INTO node_provisioning_plans (
                id, project_id, node_id, node_name, ssh_host, ssh_user, os, arch,
                hub_url, status, steps_json, notes, join_token_hash, join_token_hint,
                bound_machine_fingerprint, created_by, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, '', '', ?5, ?6, ?7, 'pending', '[]', ?8, ?9, ?10, ?11, 'worker-join', ?12, ?12)
            ON CONFLICT(id) DO UPDATE SET
                node_name = excluded.node_name,
                os = excluded.os,
                arch = excluded.arch,
                status = CASE WHEN node_provisioning_plans.status = 'bound' THEN 'bound' ELSE 'pending' END,
                join_token_hash = excluded.join_token_hash,
                join_token_hint = excluded.join_token_hint,
                bound_machine_fingerprint = COALESCE(node_provisioning_plans.bound_machine_fingerprint, excluded.bound_machine_fingerprint),
                updated_at = excluded.updated_at
            ",
            params![
                format!("join_{node_id}"),
                PROJECT_ID,
                node_id,
                node_name,
                os,
                arch,
                hub_url,
                "Worker 已申请入网，等待超级管理员授权。",
                token_hash,
                token_hint(token),
                machine_fingerprint,
                now,
            ],
        )?;
        Ok(())
    }

    fn approve_node_join(&self, node_id: &str, actor: &str) -> anyhow::Result<Value> {
        let plan = self
            .get_node_provisioning_plan(&format!("join_{node_id}"))?
            .ok_or_else(|| anyhow::anyhow!("node join request not found"))?;
        let token_hash = self
            .conn
            .query_row(
                "SELECT join_token_hash FROM node_provisioning_plans WHERE id = ?1",
                params![format!("join_{node_id}")],
                |row| row.get::<_, Option<String>>(0),
            )?
            .ok_or_else(|| anyhow::anyhow!("node join token missing"))?;
        let fingerprint = plan
            .pointer("/spec/bound_machine_fingerprint")
            .and_then(Value::as_str)
            .unwrap_or("");
        let now = now();
        self.conn.execute(
            "
            UPDATE nodes
            SET auth_status = 'bound',
                join_token_hash = ?1,
                authorized_at = COALESCE(authorized_at, ?2),
                status = 'online',
                updated_at = ?2
            WHERE project_id = ?3 AND id = ?4
            ",
            params![token_hash, now, PROJECT_ID, node_id],
        )?;
        self.conn.execute(
            "
            UPDATE node_provisioning_plans
            SET status = 'bound',
                bound_at = COALESCE(bound_at, ?1),
                updated_at = ?1
            WHERE id = ?2
            ",
            params![now, format!("join_{node_id}")],
        )?;
        self.audit(
            "node.join.approved",
            actor,
            Some(node_id),
            "节点入网申请已授权",
            json!({
                "node_id": node_id,
                "machine_fingerprint": fingerprint
            }),
        )?;
        self.get_node(node_id)?
            .ok_or_else(|| anyhow::anyhow!("node not found after approve"))
    }

    fn get_node(&self, id: &str) -> anyhow::Result<Option<Value>> {
        self.conn
            .query_row("SELECT * FROM nodes WHERE id = ?1", params![id], node_row)
            .optional()
            .map_err(Into::into)
    }

    fn delete_node(&self, id: &str) -> anyhow::Result<()> {
        let changed = self
            .conn
            .execute("DELETE FROM nodes WHERE id = ?1", params![id])?;
        if changed == 0 {
            anyhow::bail!("node not found");
        }
        Ok(())
    }

    fn update_node_config(&self, id: &str, data: Value) -> anyhow::Result<Value> {
        let existing = self
            .get_node(id)?
            .ok_or_else(|| anyhow::anyhow!("node not found"))?;
        let spec = existing.get("spec").cloned().unwrap_or_else(|| json!({}));
        let now = now();
        let tags = data
            .get("tags")
            .cloned()
            .unwrap_or_else(|| spec.get("tags").cloned().unwrap_or_else(|| json!([])));
        let groups = data
            .get("groups")
            .cloned()
            .unwrap_or_else(|| spec.get("groups").cloned().unwrap_or_else(|| json!([])));
        let capabilities = data.get("capabilities").cloned().unwrap_or_else(|| {
            spec.get("capabilities")
                .cloned()
                .unwrap_or_else(|| json!([]))
        });
        let status = data
            .get("status")
            .and_then(Value::as_str)
            .or_else(|| {
                existing
                    .pointer("/status/reported_state")
                    .and_then(Value::as_str)
            })
            .unwrap_or("online");

        self.conn.execute(
            "
            UPDATE nodes
            SET tags_json = ?1,
                groups_json = ?2,
                capabilities_json = ?3,
                weight = ?4,
                max_concurrent_jobs = ?5,
                status = ?6,
                updated_at = ?7
            WHERE id = ?8
            ",
            params![
                serde_json::to_string(&string_array_from_value(&tags))?,
                serde_json::to_string(&string_array_from_value(&groups))?,
                serde_json::to_string(&string_array_from_value(&capabilities))?,
                data.get("weight")
                    .and_then(Value::as_f64)
                    .or_else(|| spec.get("weight").and_then(Value::as_f64))
                    .unwrap_or(1.0),
                data.get("max_concurrent_jobs")
                    .and_then(Value::as_i64)
                    .or_else(|| spec.get("max_concurrent_jobs").and_then(Value::as_i64))
                    .unwrap_or(1),
                status,
                now,
                id,
            ],
        )?;
        let item = self
            .get_node(id)?
            .ok_or_else(|| anyhow::anyhow!("node not found"))?;
        self.audit(
            "node.config.changed",
            "architect-agent",
            Some(id),
            "节点调度配置已更新",
            json!({ "node_id": id, "input": data, "node": item.clone() }),
        )?;
        Ok(item)
    }

    fn register_node_tools(&self, node_id: &str, data: Value) -> anyhow::Result<Vec<Value>> {
        self.get_node(node_id)?
            .ok_or_else(|| anyhow::anyhow!("node not found"))?;
        let tools = data
            .get("tools")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_else(|| vec![data]);
        let mut saved = Vec::new();
        for tool in tools {
            saved.push(self.upsert_node_tool(node_id, tool)?);
        }
        self.audit(
            "node.tools.registered",
            node_id,
            Some(node_id),
            "节点工具已注册",
            json!({ "node_id": node_id, "count": saved.len(), "items": saved.clone() }),
        )?;
        Ok(saved)
    }

    fn upsert_node_tool(&self, node_id: &str, data: Value) -> anyhow::Result<Value> {
        let tool_id = data
            .get("tool_id")
            .or_else(|| data.get("id"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("tool_id is required"))?
            .to_string();
        let id = format!("ntool_{}_{}", node_id, tool_id).replace(
            |character: char| !character.is_ascii_alphanumeric() && character != '_',
            "_",
        );
        let labels = data
            .get("labels")
            .map(string_array_from_value)
            .unwrap_or_else(|| vec!["compute".to_string(), format!("tool:{tool_id}")]);
        let metadata = normalize_node_tool_metadata(&data, &tool_id, node_id);
        let now = now();
        self.conn.execute(
            "
            INSERT INTO node_tools (
                id, project_id, node_id, tool_id, name, version, executor, status, confidence,
                input_schema_json, output_schema_json, constraints_json, labels_json,
                default_verify_json, probe_json, probe_state, next_probe_at, metadata_json, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?19)
            ON CONFLICT(project_id, node_id, tool_id) DO UPDATE SET
                name = excluded.name,
                version = excluded.version,
                executor = excluded.executor,
                status = excluded.status,
                confidence = excluded.confidence,
                input_schema_json = excluded.input_schema_json,
                output_schema_json = excluded.output_schema_json,
                constraints_json = excluded.constraints_json,
                labels_json = excluded.labels_json,
                default_verify_json = excluded.default_verify_json,
                probe_json = excluded.probe_json,
                probe_state = CASE
                    WHEN node_tools.probe_state = 'verified' THEN node_tools.probe_state
                    ELSE excluded.probe_state
                END,
                next_probe_at = COALESCE(excluded.next_probe_at, node_tools.next_probe_at),
                metadata_json = excluded.metadata_json,
                updated_at = excluded.updated_at
            ",
            params![
                id,
                PROJECT_ID,
                node_id,
                tool_id,
                string_or(&data, "name", &tool_id),
                string_or(&data, "version", "0.1.0"),
                string_or(&data, "executor", "plugin"),
                string_or(&data, "status", "available"),
                string_or(&data, "confidence", "declared"),
                serde_json::to_string(data.get("input_schema").unwrap_or(&json!({})))?,
                serde_json::to_string(data.get("output_schema").unwrap_or(&json!({})))?,
                serde_json::to_string(data.get("constraints").unwrap_or(&json!({})))?,
                serde_json::to_string(&labels)?,
                optional_json_value_string(&data, "default_verify")?,
                optional_json_value_string(&data, "probe")?,
                initial_node_tool_probe_state(&data),
                initial_node_tool_next_probe_at(&data),
                serde_json::to_string(&metadata)?,
                now,
            ],
        )?;
        self.get_node_tool(node_id, &tool_id)?
            .ok_or_else(|| anyhow::anyhow!("node tool not found after upsert"))
    }

    fn list_node_tools(&self, node_id: Option<&str>) -> anyhow::Result<Vec<Value>> {
        let mut sql = "SELECT * FROM node_tools WHERE project_id = ?1".to_string();
        let mut values = vec![PROJECT_ID.to_string()];
        if let Some(node_id) = node_id {
            sql.push_str(" AND node_id = ?2");
            values.push(node_id.to_string());
        }
        sql.push_str(" ORDER BY tool_id ASC, node_id ASC");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(values), node_tool_row)?;
        collect_values(rows)
    }

    fn get_node_tool(&self, node_id: &str, tool_id: &str) -> anyhow::Result<Option<Value>> {
        self.conn
            .query_row(
                "SELECT * FROM node_tools WHERE project_id = ?1 AND node_id = ?2 AND tool_id = ?3",
                params![PROJECT_ID, node_id, tool_id],
                node_tool_row,
            )
            .optional()
            .map_err(Into::into)
    }

    fn list_node_tools_by_tool(&self, tool_id: &str) -> anyhow::Result<Vec<Value>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT * FROM node_tools
            WHERE project_id = ?1 AND tool_id = ?2
            ORDER BY node_id ASC
            ",
        )?;
        let rows = stmt.query_map(params![PROJECT_ID, tool_id], node_tool_row)?;
        collect_values(rows)
    }

    fn due_node_tools_for_probe(&self, limit: u16) -> anyhow::Result<Vec<Value>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT * FROM node_tools
            WHERE project_id = ?1
              AND status = 'available'
              AND probe_json IS NOT NULL
              AND json_extract(probe_json, '$.enabled') IS NOT 0
              AND (
                next_probe_at IS NULL
                OR next_probe_at <= ?2
                OR probe_state IN ('declared_unverified', 'expired')
              )
            ORDER BY COALESCE(next_probe_at, created_at) ASC
            LIMIT ?3
            ",
        )?;
        let rows = stmt.query_map(params![PROJECT_ID, now(), limit.min(100)], node_tool_row)?;
        collect_values(rows)
    }

    fn create_node_tool_probe_tasks(
        &self,
        tool_id: Option<&str>,
        node_id: Option<&str>,
        trigger: &str,
    ) -> anyhow::Result<Vec<Value>> {
        let mut tools = if let Some(tool_id) = tool_id {
            self.list_node_tools_by_tool(tool_id)?
        } else {
            self.list_node_tools(None)?
        };
        if let Some(node_id) = node_id {
            tools.retain(|tool| {
                tool.pointer("/metadata/node_id").and_then(Value::as_str) == Some(node_id)
            });
        }
        if tool_id.is_some() && tools.is_empty() {
            anyhow::bail!("node tool not found");
        }

        let mut created = Vec::new();
        for tool in tools {
            let node_id = tool
                .pointer("/metadata/node_id")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("node tool node_id missing"))?;
            let tool_id = tool
                .pointer("/spec/tool_id")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("node tool tool_id missing"))?;
            let node = self.get_node(node_id)?;
            if node
                .as_ref()
                .and_then(|node| node.pointer("/status/state").and_then(Value::as_str))
                != Some("online")
            {
                continue;
            }
            let Some(payload) = node_tool_probe_payload(&tool) else {
                self.upsert_tool_probe_record(
                    tool_id,
                    node_id,
                    None,
                    "unsupported",
                    None,
                    Some(json!({
                        "code": "probe_not_configured",
                        "message": "动态节点工具没有配置 probe.payload"
                    })),
                )?;
                self.update_node_tool_probe_status(
                    node_id,
                    tool_id,
                    None,
                    "unsupported",
                    None,
                    Some(json!({
                        "code": "probe_not_configured",
                        "message": "动态节点工具没有配置 probe.payload"
                    })),
                )?;
                continue;
            };
            let verify = node_tool_probe_verify(&tool);
            let title = format!("Node Tool Probe {tool_id} on {node_id}");
            let task = self.create_task(json!({
                "title": title,
                "summary": "AgentGrid Node Tool Probe v1 节点动态工具健康检查。",
                "created_by": "node-tool-probe-engine",
                "owner": "worker-agent",
                "assigned_to": ["worker-agent"],
                "priority": "low",
                "labels": node_tool_probe_labels(&tool, node_id, tool_id),
                "inputs": [serde_json::to_string_pretty(&payload)?],
                "outputs": ["Probe 执行结果", "验证状态"],
                "acceptance_criteria": [
                    "Worker 能按节点工具协议执行 Probe",
                    "Hub 能根据结果更新节点工具健康状态"
                ],
                "correlation_id": format!("node_tool_probe:{tool_id}:{node_id}:{trigger}"),
                "verify": verify
            }))?;
            let task_id = task
                .item
                .pointer("/metadata/id")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("node tool probe task id missing"))?;
            self.upsert_tool_probe_record(tool_id, node_id, Some(task_id), "pending", None, None)?;
            self.update_node_tool_probe_status(
                node_id,
                tool_id,
                Some(task_id),
                "pending",
                None,
                None,
            )?;
            created.push(json!({
                "tool_id": tool_id,
                "node_id": node_id,
                "task_id": task_id,
                "status": "pending",
                "trigger": trigger
            }));
        }
        Ok(created)
    }

    fn update_node_tool_probe_status(
        &self,
        node_id: &str,
        tool_id: &str,
        task_id: Option<&str>,
        status: &str,
        result: Option<Value>,
        error: Option<Value>,
    ) -> anyhow::Result<()> {
        let now = now();
        let next_probe_at = self.next_node_tool_probe_at(node_id, tool_id, status)?;
        let last_probe_at = if matches!(status, "verified" | "failed" | "unsupported") {
            Some(now.clone())
        } else {
            None
        };
        self.conn.execute(
            "
            UPDATE node_tools
            SET probe_state = ?1,
                probe_task_id = COALESCE(?2, probe_task_id),
                last_probe_at = COALESCE(?3, last_probe_at),
                next_probe_at = ?4,
                probe_error_json = ?5,
                updated_at = ?6
            WHERE project_id = ?7 AND node_id = ?8 AND tool_id = ?9
            ",
            params![
                status,
                task_id,
                last_probe_at,
                next_probe_at,
                serde_json::to_string(&error.unwrap_or(Value::Null))?,
                now,
                PROJECT_ID,
                node_id,
                tool_id,
            ],
        )?;
        if result.is_some() {
            // The canonical probe output is stored in tool_probes; node_tools keeps scheduling metadata.
        }
        Ok(())
    }

    fn expire_stale_node_tool_probes(&self) -> anyhow::Result<usize> {
        let current = now();
        let changed = self.conn.execute(
            "
            UPDATE node_tools
            SET probe_state = 'expired',
                updated_at = ?1
            WHERE project_id = ?2
              AND probe_state = 'verified'
              AND next_probe_at IS NOT NULL
              AND next_probe_at < ?1
            ",
            params![current, PROJECT_ID],
        )?;
        Ok(changed)
    }

    fn next_node_tool_probe_at(
        &self,
        node_id: &str,
        tool_id: &str,
        status: &str,
    ) -> anyhow::Result<Option<String>> {
        let Some(tool) = self.get_node_tool(node_id, tool_id)? else {
            return Ok(None);
        };
        let interval = tool
            .pointer("/spec/probe/interval_seconds")
            .and_then(Value::as_i64)
            .unwrap_or(match status {
                "failed" | "unsupported" => 300,
                "pending" => 120,
                _ => 300,
            })
            .clamp(30, 86_400);
        Ok(Some(
            (Utc::now() + chrono::Duration::seconds(interval))
                .to_rfc3339_opts(chrono::SecondsFormat::Micros, true),
        ))
    }

    fn list_node_tool_catalog(&self) -> anyhow::Result<Vec<Value>> {
        let tools = self.list_node_tools(None)?;
        let nodes = self.list_nodes()?;
        let mut grouped: HashMap<String, Vec<Value>> = HashMap::new();
        for tool in tools {
            if let Some(tool_id) = tool.pointer("/spec/tool_id").and_then(Value::as_str) {
                grouped.entry(tool_id.to_string()).or_default().push(tool);
            }
        }
        let mut catalog = grouped
            .into_iter()
            .map(|(tool_id, items)| node_tool_catalog_item(&tool_id, items, &nodes))
            .collect::<Vec<_>>();
        catalog.sort_by(|left, right| {
            left.get("tool_id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .cmp(right.get("tool_id").and_then(Value::as_str).unwrap_or(""))
        });
        Ok(catalog)
    }

    fn get_node_tool_catalog(&self, tool_id: &str) -> anyhow::Result<Option<Value>> {
        let items = self.list_node_tools_by_tool(tool_id)?;
        if items.is_empty() {
            return Ok(None);
        }
        let nodes = self.list_nodes()?;
        Ok(Some(node_tool_catalog_item(tool_id, items, &nodes)))
    }

    fn node_supports_task_tool(&self, node_id: &str, tool_id: &str) -> anyhow::Result<bool> {
        if self.get_node_tool(node_id, tool_id)?.is_some_and(|tool| {
            tool.pointer("/status/state").and_then(Value::as_str) == Some("available")
                && !matches!(
                    tool.pointer("/status/probe_state").and_then(Value::as_str),
                    Some("failed" | "unavailable" | "unsupported")
                )
        }) {
            return Ok(true);
        }
        let Some(tool) = tool_registry()
            .into_iter()
            .find(|tool| tool.get("id").and_then(Value::as_str) == Some(tool_id))
        else {
            return Ok(false);
        };
        let Some(capability) = tool.get("capability").and_then(Value::as_str) else {
            return Ok(false);
        };
        let Some(node) = self.get_node(node_id)? else {
            return Ok(false);
        };
        Ok(node
            .pointer("/spec/capabilities")
            .and_then(Value::as_array)
            .map(|items| items.iter().any(|item| item.as_str() == Some(capability)))
            .unwrap_or(false))
    }

    fn tool_registry_with_dynamic(&self) -> anyhow::Result<Vec<Value>> {
        let mut tools = tool_registry();
        let mut existing = tools
            .iter()
            .filter_map(|tool| {
                tool.get("id")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            })
            .collect::<HashSet<_>>();
        for item in self.list_node_tool_catalog()? {
            let Some(tool_id) = item.get("tool_id").and_then(Value::as_str) else {
                continue;
            };
            if existing.insert(tool_id.to_string()) {
                tools.push(dynamic_tool_contract_from_catalog(&item));
            }
        }
        Ok(tools)
    }

    fn capabilities_manifest(&self) -> anyhow::Result<Value> {
        let nodes = self.list_nodes()?;
        let tools = self
            .tool_registry_with_dynamic()?
            .into_iter()
            .map(|tool| self.capability_manifest_item(tool, &nodes))
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(json!({
            "ok": true,
            "api_version": "agentgrid.capabilities/v1",
            "kind": "CapabilityManifest",
            "metadata": {
                "project_id": PROJECT_ID,
                "hub_url": "http://chenqi.tminos.com:20080/agentgrid",
                "generated_at": now()
            },
            "workflow": [
                "discover_capabilities",
                "select_tool",
                "construct_job",
                "submit_job",
                "watch_job",
                "read_status_result"
            ],
            "job_features": {
                "partition": ["none", "items", "range"],
                "template_variables": [
                    "${shard.index}",
                    "${shard.count}",
                    "${partition.items[0]}",
                    "${partition.range.start}",
                    "${partition.range.end}"
                ],
                "reduce": ["summary", "stdout_concat", "json_array"],
                "checkpoint_resume": true,
                "node_lost_reschedule": true
            },
            "endpoints": {
                "manifest": "/api/capabilities/manifest",
                "submit_job": "/api/jobs",
                "get_job": "/api/jobs/{id}",
                "tools": "/api/tools",
                "tool_nodes": "/api/tools/{tool_id}/nodes"
            },
            "tools": tools
        }))
    }

    fn capability_manifest_item(&self, tool: Value, nodes: &[Value]) -> anyhow::Result<Value> {
        let enriched = self.enrich_tool_with_nodes(tool, nodes)?;
        let tool_id = enriched.get("id").and_then(Value::as_str).unwrap_or("");
        let nodes = enriched
            .get("nodes")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let available_nodes = nodes.len();
        let verified_nodes = nodes
            .iter()
            .filter(|node| {
                node.get("verification_status").and_then(Value::as_str) == Some("verified")
            })
            .count();
        let recommended_reduce = recommended_reduce_for_tool(tool_id);
        Ok(json!({
            "tool_id": tool_id,
            "name": enriched.get("name").cloned().unwrap_or(Value::Null),
            "summary": enriched.get("summary").cloned().unwrap_or(Value::Null),
            "category": enriched.get("category").cloned().unwrap_or(Value::Null),
            "capability": enriched.get("capability").cloned().unwrap_or(Value::Null),
            "payload_type": enriched.get("payload_type").cloned().unwrap_or(Value::Null),
            "risk": enriched.get("risk").cloned().unwrap_or(Value::Null),
            "requires_policy": enriched.get("requires_policy").cloned().unwrap_or(json!(false)),
            "available_nodes": available_nodes,
            "verified_nodes": verified_nodes,
            "supports_partition": supports_partition_for_tool(tool_id),
            "supports_template": true,
            "recommended_reduce": recommended_reduce,
            "input_schema": enriched.get("input_schema").cloned().unwrap_or_else(|| json!({})),
            "output_schema": enriched.get("output_schema").cloned().unwrap_or_else(|| json!({})),
            "examples": enriched.get("examples").cloned().unwrap_or_else(|| json!([])),
            "nodes": nodes,
            "job_example": capability_job_example(tool_id, recommended_reduce)
        }))
    }

    fn runtime_tool_selection(
        &self,
        tool_id: &str,
    ) -> anyhow::Result<Option<RuntimeToolSelection>> {
        if let Some(tool) = tool_registry()
            .into_iter()
            .find(|tool| tool.get("id").and_then(Value::as_str) == Some(tool_id))
        {
            return Ok(Some(RuntimeToolSelection {
                tool,
                dynamic: false,
            }));
        }
        let Some(catalog) = self.get_node_tool_catalog(tool_id)? else {
            return Ok(None);
        };
        Ok(Some(RuntimeToolSelection {
            tool: dynamic_tool_contract_from_catalog(&catalog),
            dynamic: true,
        }))
    }

    fn enrich_tool_with_nodes(&self, mut tool: Value, nodes: &[Value]) -> anyhow::Result<Value> {
        let supported = self.nodes_for_tool_with_probe(&tool, nodes)?;
        if let Some(map) = tool.as_object_mut() {
            map.insert(
                "supported_nodes".to_string(),
                Value::Array(
                    supported
                        .iter()
                        .filter_map(|node| node.get("id").cloned())
                        .collect(),
                ),
            );
            map.insert("node_count".to_string(), json!(supported.len()));
            map.insert(
                "verified_node_count".to_string(),
                json!(supported
                    .iter()
                    .filter(
                        |node| node.get("verification_status").and_then(Value::as_str)
                            == Some("verified")
                    )
                    .count()),
            );
            map.insert("nodes".to_string(), Value::Array(supported));
            map.insert(
                "support_basis".to_string(),
                json!("node_heartbeat_capabilities"),
            );
            map.insert("verification_status".to_string(), json!("mixed_per_node"));
        }
        Ok(tool)
    }

    fn nodes_for_tool_with_probe(
        &self,
        tool: &Value,
        nodes: &[Value],
    ) -> anyhow::Result<Vec<Value>> {
        let tool_id = tool.get("id").and_then(Value::as_str).unwrap_or("");
        let supported_nodes = if is_dynamic_tool_id(tool_id) {
            self.nodes_for_dynamic_tool(tool_id, nodes)?
        } else {
            nodes_for_tool(tool, nodes)
        };
        supported_nodes
            .into_iter()
            .map(|mut node| {
                let tool_id = tool_id.to_string();
                let node_id = node
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let probe = self.get_tool_probe(&tool_id, &node_id)?;
                let status = probe
                    .as_ref()
                    .and_then(|probe| probe.pointer("/status/state").and_then(Value::as_str))
                    .unwrap_or("declared_unverified");
                let support_basis = probe
                    .as_ref()
                    .and_then(|probe| probe.pointer("/spec/support_basis").and_then(Value::as_str))
                    .unwrap_or("node_heartbeat_capabilities");
                if let Some(map) = node.as_object_mut() {
                    map.insert("verification_status".to_string(), json!(status));
                    map.insert("support_basis".to_string(), json!(support_basis));
                    map.insert(
                        "probe".to_string(),
                        probe.unwrap_or_else(|| {
                            json!({
                                "kind": "ToolProbe",
                                "metadata": {
                                    "tool_id": tool_id,
                                    "node_id": node_id
                                },
                                "status": {
                                    "state": "declared_unverified"
                                }
                            })
                        }),
                    );
                }
                Ok(node)
            })
            .collect()
    }

    fn nodes_for_dynamic_tool(&self, tool_id: &str, nodes: &[Value]) -> anyhow::Result<Vec<Value>> {
        let mut supported = Vec::new();
        for node in nodes {
            if node.pointer("/status/state").and_then(Value::as_str) != Some("online") {
                continue;
            }
            let Some(node_id) = node.pointer("/metadata/id").and_then(Value::as_str) else {
                continue;
            };
            let Some(tool) = self.get_node_tool(node_id, tool_id)? else {
                continue;
            };
            if tool.pointer("/status/state").and_then(Value::as_str) != Some("available") {
                continue;
            }
            supported.push(json!({
                "id": node_id,
                "name": node.pointer("/metadata/name").and_then(Value::as_str),
                "state": node.pointer("/status/state").and_then(Value::as_str),
                "os": node.pointer("/spec/os").and_then(Value::as_str),
                "arch": node.pointer("/spec/arch").and_then(Value::as_str),
                "address": node.pointer("/spec/address").and_then(Value::as_str),
                "cpu_cores": node.pointer("/spec/cpu_cores").and_then(Value::as_i64),
                "memory_mb": node.pointer("/spec/memory_mb").and_then(Value::as_i64),
                "running_jobs": node.pointer("/status/running_jobs").and_then(Value::as_i64),
                "max_concurrent_jobs": node.pointer("/spec/max_concurrent_jobs").and_then(Value::as_i64),
                "worker_version": node.pointer("/spec/worker_version").and_then(Value::as_str),
                "worker_target": node.pointer("/spec/worker_target").and_then(Value::as_str),
                "support_basis": "node_tool_registration",
                "verification_status": tool.pointer("/status/probe_state").and_then(Value::as_str).unwrap_or("declared_unverified")
            }));
        }
        Ok(supported)
    }

    fn list_messages(&self, limit: u16) -> anyhow::Result<Vec<Value>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM agent_messages WHERE project_id = ?1 ORDER BY created_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![PROJECT_ID, limit], message_row)?;
        collect_values(rows)
    }

    fn create_message(&self, data: Value) -> anyhow::Result<Value> {
        let id = string_or(&data, "id", &new_id("msg"));
        let now = now();
        self.conn.execute(
            "
            INSERT INTO agent_messages (
                id, project_id, from_agent_id, to_agents_json, message_type, subject,
                summary, priority, requires_ack, payload_json, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ",
            params![
                id,
                PROJECT_ID,
                required_string(&data, "from")?,
                serde_json::to_string(&array_field(&data, "to"))?,
                required_string(&data, "type")?,
                required_string(&data, "subject")?,
                string_or(&data, "summary", ""),
                string_or(&data, "priority", "normal"),
                bool_or(&data, "requires_ack", false) as i64,
                serde_json::to_string(data.get("payload").unwrap_or(&json!({})))?,
                now,
            ],
        )?;
        let item = self
            .get_message(&id)?
            .ok_or_else(|| anyhow::anyhow!("message not found"))?;
        self.audit(
            "message.created",
            item.pointer("/metadata/from")
                .and_then(Value::as_str)
                .unwrap_or("unknown"),
            Some(&id),
            item.pointer("/spec/subject")
                .and_then(Value::as_str)
                .unwrap_or("消息已创建"),
            json!({ "message": item.clone() }),
        )?;
        Ok(item)
    }

    fn get_message(&self, id: &str) -> anyhow::Result<Option<Value>> {
        self.conn
            .query_row(
                "SELECT * FROM agent_messages WHERE id = ?1",
                params![id],
                message_row,
            )
            .optional()
            .map_err(Into::into)
    }

    fn list_events(&self, query: EventQuery, limit: u16) -> anyhow::Result<Vec<Value>> {
        let mut sql = "SELECT * FROM audit_events WHERE project_id = ?1".to_string();
        let mut values = vec![PROJECT_ID.to_string()];
        let event_type = query.event_type.or(query.type_alias);
        if let Some(event_type) = event_type.filter(|value| !value.trim().is_empty()) {
            sql.push_str(" AND event_type = ?");
            values.push(event_type);
        }
        if let Some(subject_id) = query.subject_id.filter(|value| !value.trim().is_empty()) {
            sql.push_str(" AND subject_id = ?");
            values.push(subject_id);
        }
        sql.push_str(" ORDER BY created_at DESC LIMIT ?");
        values.push(limit.to_string());
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(values), audit_row)?;
        collect_values(rows)
    }

    fn list_node_provisioning_plans(&self, limit: u16) -> anyhow::Result<Vec<Value>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT * FROM node_provisioning_plans
            WHERE project_id = ?1
            ORDER BY created_at DESC
            LIMIT ?2
            ",
        )?;
        let rows = stmt.query_map(params![PROJECT_ID, limit], provisioning_plan_row)?;
        collect_values(rows)
    }

    fn create_node_provisioning_plan(&self, data: Value) -> anyhow::Result<Value> {
        let id = string_or(&data, "id", &new_id("provision"));
        let node_id = required_string(&data, "node_id")?;
        let node_name = string_or(&data, "node_name", &node_id);
        let ssh_host = required_string(&data, "ssh_host")?;
        let ssh_user = string_or(&data, "ssh_user", "root");
        let os = string_or(&data, "os", "linux");
        let arch = string_or(&data, "arch", "x86_64");
        let hub_url = string_or(
            &data,
            "hub_url",
            "http://chenqi.tminos.com:20080/agentgrid/api",
        );
        let notes = string_or(&data, "notes", "凭据不进入 AgentGrid 数据库和文档。");
        let created_by = string_or(&data, "created_by", "architect-agent");
        let steps = node_provisioning_steps(&node_id, &node_name, &ssh_host, &ssh_user, &hub_url);
        let now = now();
        self.conn.execute(
            "
            INSERT INTO node_provisioning_plans (
                id, project_id, node_id, node_name, ssh_host, ssh_user, os, arch,
                hub_url, status, steps_json, notes, created_by, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'planned', ?10, ?11, ?12, ?13, ?13)
            ON CONFLICT(id) DO UPDATE SET
                node_id = excluded.node_id,
                node_name = excluded.node_name,
                ssh_host = excluded.ssh_host,
                ssh_user = excluded.ssh_user,
                os = excluded.os,
                arch = excluded.arch,
                hub_url = excluded.hub_url,
                steps_json = excluded.steps_json,
                notes = excluded.notes,
                updated_at = excluded.updated_at
            ",
            params![
                id,
                PROJECT_ID,
                node_id,
                node_name,
                ssh_host,
                ssh_user,
                os,
                arch,
                hub_url,
                serde_json::to_string(&steps)?,
                notes,
                created_by,
                now,
            ],
        )?;
        let item = self
            .get_node_provisioning_plan(&id)?
            .ok_or_else(|| anyhow::anyhow!("provisioning plan not found"))?;
        self.audit(
            "node.provisioning.planned",
            item.pointer("/metadata/created_by")
                .and_then(Value::as_str)
                .unwrap_or("architect-agent"),
            Some(&id),
            "节点纳管计划已生成",
            json!({ "plan": item.clone() }),
        )?;
        Ok(item)
    }

    fn get_node_provisioning_plan(&self, id: &str) -> anyhow::Result<Option<Value>> {
        self.conn
            .query_row(
                "SELECT * FROM node_provisioning_plans WHERE id = ?1",
                params![id],
                provisioning_plan_row,
            )
            .optional()
            .map_err(Into::into)
    }

    fn list_workflow_templates(&self, limit: u16) -> anyhow::Result<Vec<Value>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT * FROM workflow_templates
            WHERE project_id = ?1
            ORDER BY updated_at DESC
            LIMIT ?2
            ",
        )?;
        let rows = stmt.query_map(params![PROJECT_ID, limit], workflow_template_row)?;
        collect_values(rows)
    }

    fn create_workflow_template(&self, data: Value) -> anyhow::Result<Value> {
        let id = string_or(&data, "id", &new_id("wftpl"));
        self.insert_workflow_template(id, data)
    }

    fn create_workflow_template_if_missing(&self, data: Value) -> anyhow::Result<Value> {
        let id = required_string(&data, "id")?;
        if let Some(existing) = self.get_workflow_template(&id)? {
            return Ok(existing);
        }
        self.insert_workflow_template(id, data)
    }

    fn insert_workflow_template(&self, id: String, data: Value) -> anyhow::Result<Value> {
        let nodes = parse_workflow_nodes(data.get("nodes").unwrap_or(&json!([])))?;
        validate_workflow_nodes(&nodes)?;
        let nodes_json =
            serde_json::to_value(nodes.iter().map(workflow_node_to_json).collect::<Vec<_>>())?;
        let now = now();
        self.conn.execute(
            "
            INSERT INTO workflow_templates (
                id, project_id, name, summary, created_by, parameters_json, nodes_json,
                created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                summary = excluded.summary,
                created_by = excluded.created_by,
                parameters_json = excluded.parameters_json,
                nodes_json = excluded.nodes_json,
                updated_at = excluded.updated_at
            ",
            params![
                id,
                PROJECT_ID,
                required_string(&data, "name")?,
                string_or(&data, "summary", ""),
                string_or(&data, "created_by", "architect-agent"),
                serde_json::to_string(data.get("parameters").unwrap_or(&json!([])))?,
                serde_json::to_string(&nodes_json)?,
                now,
            ],
        )?;
        let item = self
            .get_workflow_template(&id)?
            .ok_or_else(|| anyhow::anyhow!("workflow template not found"))?;
        self.audit(
            "workflow.template.saved",
            item.pointer("/metadata/created_by")
                .and_then(Value::as_str)
                .unwrap_or("architect-agent"),
            Some(&id),
            "工作流模板已保存",
            json!({ "template": item.clone() }),
        )?;
        Ok(item)
    }

    fn get_workflow_template(&self, id: &str) -> anyhow::Result<Option<Value>> {
        self.conn
            .query_row(
                "SELECT * FROM workflow_templates WHERE id = ?1",
                params![id],
                workflow_template_row,
            )
            .optional()
            .map_err(Into::into)
    }

    fn start_workflow_template(&self, id: &str, data: Value) -> anyhow::Result<Value> {
        let template = self
            .get_workflow_template(id)?
            .ok_or_else(|| anyhow::anyhow!("workflow template not found"))?;
        let parameters = data.get("parameters").cloned().unwrap_or_else(|| json!({}));
        let nodes = render_template_value(
            template
                .pointer("/spec/nodes")
                .ok_or_else(|| anyhow::anyhow!("workflow template nodes missing"))?,
            &parameters,
        );
        let name = render_template_text(
            data.get("name")
                .and_then(Value::as_str)
                .or_else(|| template.pointer("/spec/name").and_then(Value::as_str))
                .unwrap_or("模板工作流"),
            &parameters,
        );
        let summary = render_template_text(
            template
                .pointer("/spec/summary")
                .and_then(Value::as_str)
                .unwrap_or(""),
            &parameters,
        );
        let workflow = self.create_workflow(json!({
            "name": name,
            "summary": summary,
            "created_by": string_or(&data, "actor", "architect-agent"),
            "inputs": { "template_id": id, "parameters": parameters },
            "nodes": nodes
        }))?;
        let workflow_id = workflow
            .pointer("/metadata/id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("workflow id missing"))?;
        let item = self.start_workflow(
            workflow_id,
            json!({
                "actor": string_or(&data, "actor", "architect-agent"),
                "template_id": id
            }),
        )?;
        self.audit(
            "workflow.template.started",
            &string_or(&data, "actor", "architect-agent"),
            Some(id),
            "工作流模板已实例化并启动",
            json!({ "template_id": id, "workflow": item.clone() }),
        )?;
        Ok(item)
    }

    fn list_workflows(&self, query: WorkflowQuery) -> anyhow::Result<Vec<Value>> {
        let limit = query.limit.unwrap_or(100).min(500);
        let mut sql = "SELECT * FROM workflows WHERE project_id = ?1".to_string();
        let mut values = vec![PROJECT_ID.to_string()];
        if let Some(state) = query.state {
            sql.push_str(" AND status = ?");
            values.push(state);
        }
        sql.push_str(" ORDER BY updated_at DESC LIMIT ?");
        values.push(limit.to_string());

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(values), workflow_row)?;
        let workflows = collect_values(rows)?;
        workflows
            .iter()
            .map(|workflow| {
                let id = workflow
                    .pointer("/metadata/id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow::anyhow!("workflow id missing"))?;
                self.get_workflow_detail(id)?
                    .ok_or_else(|| anyhow::anyhow!("workflow not found"))
            })
            .collect()
    }

    fn create_workflow(&self, data: Value) -> anyhow::Result<Value> {
        let id = string_or(&data, "id", &new_id("workflow"));
        let created_by = string_or(&data, "created_by", "architect-agent");
        let nodes = parse_workflow_nodes(data.get("nodes").unwrap_or(&json!([])))?;
        validate_workflow_nodes(&nodes)?;
        let nodes_json =
            serde_json::to_value(nodes.iter().map(workflow_node_to_json).collect::<Vec<_>>())?;
        let now = now();
        self.conn.execute(
            "
            INSERT INTO workflows (
                id, project_id, name, summary, created_by, status, inputs_json, nodes_json,
                result_json, error_json, created_at, updated_at, started_at, completed_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, 'draft', ?6, ?7, NULL, NULL, ?8, ?8, NULL, NULL)
            ",
            params![
                id,
                PROJECT_ID,
                required_string(&data, "name")?,
                string_or(&data, "summary", ""),
                created_by,
                serde_json::to_string(data.get("inputs").unwrap_or(&json!({})))?,
                serde_json::to_string(&nodes_json)?,
                now,
            ],
        )?;
        self.audit(
            "workflow.created",
            &string_or(&data, "created_by", "architect-agent"),
            Some(&id),
            &format!("工作流已创建：{}", required_string(&data, "name")?),
            json!({ "workflow_id": id, "nodes": nodes_json }),
        )?;
        self.get_workflow_detail(&id)?
            .ok_or_else(|| anyhow::anyhow!("workflow not found"))
    }

    fn get_workflow(&self, id: &str) -> anyhow::Result<Option<Value>> {
        self.conn
            .query_row(
                "SELECT * FROM workflows WHERE id = ?1",
                params![id],
                workflow_row,
            )
            .optional()
            .map_err(Into::into)
    }

    fn get_workflow_detail(&self, id: &str) -> anyhow::Result<Option<Value>> {
        let Some(mut workflow) = self.get_workflow(id)? else {
            return Ok(None);
        };
        let runs = self.list_workflow_runs_with_tasks(id)?;
        if let Some(spec) = workflow.get_mut("spec").and_then(Value::as_object_mut) {
            spec.insert("runs".to_string(), Value::Array(runs.clone()));
        }
        if let Some(status) = workflow.get_mut("status").and_then(Value::as_object_mut) {
            status.insert("progress".to_string(), json!(workflow_progress(&runs)));
            status.insert("ready_count".to_string(), json!(count_runs(&runs, "ready")));
            status.insert(
                "running_count".to_string(),
                json!(count_runs(&runs, "running")),
            );
            status.insert("done_count".to_string(), json!(count_runs(&runs, "done")));
            status.insert(
                "skipped_count".to_string(),
                json!(count_runs(&runs, "skipped")),
            );
            status.insert(
                "failed_count".to_string(),
                json!(count_runs(&runs, "failed")),
            );
        }
        Ok(Some(workflow))
    }

    fn start_workflow(&self, id: &str, data: Value) -> anyhow::Result<Value> {
        let workflow = self
            .get_workflow(id)?
            .ok_or_else(|| anyhow::anyhow!("workflow not found"))?;
        let state = workflow
            .pointer("/status/state")
            .and_then(Value::as_str)
            .unwrap_or("draft");
        if !matches!(state, "draft" | "failed" | "cancelled") {
            anyhow::bail!("workflow cannot start from state {state}");
        }
        self.conn.execute(
            "DELETE FROM workflow_runs WHERE workflow_id = ?1",
            params![id],
        )?;
        let nodes = parse_workflow_nodes(
            workflow
                .pointer("/spec/nodes")
                .ok_or_else(|| anyhow::anyhow!("workflow nodes missing"))?,
        )?;
        validate_workflow_nodes(&nodes)?;
        let now = now();
        for node in &nodes {
            self.conn.execute(
                "
                INSERT INTO workflow_runs (
                    id, project_id, workflow_id, workflow_node_id, task_id, status,
                    depends_on_json, created_at, updated_at, started_at, completed_at,
                    result_json, error_json
                ) VALUES (?1, ?2, ?3, ?4, NULL, 'pending', ?5, ?6, ?6, NULL, NULL, NULL, NULL)
                ",
                params![
                    new_id("wrun"),
                    PROJECT_ID,
                    id,
                    node.id,
                    serde_json::to_string(&node.depends_on)?,
                    now,
                ],
            )?;
        }
        self.conn.execute(
            "
            UPDATE workflows
            SET status = 'running', started_at = ?1, completed_at = NULL,
                result_json = NULL, error_json = NULL, updated_at = ?1
            WHERE id = ?2
            ",
            params![now, id],
        )?;
        self.audit(
            "workflow.started",
            &string_or(&data, "actor", "architect-agent"),
            Some(id),
            "工作流已启动",
            json!({ "workflow_id": id, "node_count": nodes.len() }),
        )?;
        self.release_ready_workflow_nodes(id)?;
        self.get_workflow_detail(id)?
            .ok_or_else(|| anyhow::anyhow!("workflow not found"))
    }

    fn cancel_workflow(&self, id: &str, data: Value) -> anyhow::Result<Value> {
        self.get_workflow(id)?
            .ok_or_else(|| anyhow::anyhow!("workflow not found"))?;
        let actor = string_or(&data, "actor", "architect-agent");
        let now = now();
        self.conn.execute(
            "
            UPDATE workflows
            SET status = 'cancelled', completed_at = ?1, updated_at = ?1,
                error_json = ?2
            WHERE id = ?3
            ",
            params![
                now,
                serde_json::to_string(&json!({
                    "code": "workflow_cancelled",
                    "message": string_or(&data, "reason", "工作流已取消")
                }))?,
                id,
            ],
        )?;
        self.conn.execute(
            "
            UPDATE workflow_runs
            SET status = 'cancelled', completed_at = ?1, updated_at = ?1
            WHERE workflow_id = ?2 AND status IN ('pending', 'ready')
            ",
            params![now, id],
        )?;
        self.conn.execute(
            "
            UPDATE agent_tasks
            SET status = 'cancelled', control_json = ?1, updated_at = ?2
            WHERE workflow_id = ?3 AND status IN ('assigned', 'todo')
            ",
            params![
                serde_json::to_string(&json!({
                    "action": "cancel",
                    "requested_by": actor,
                    "requested_at": now,
                    "reason": string_or(&data, "reason", "工作流已取消")
                }))?,
                now,
                id,
            ],
        )?;
        self.audit("workflow.cancelled", &actor, Some(id), "工作流已取消", data)?;
        self.get_workflow_detail(id)?
            .ok_or_else(|| anyhow::anyhow!("workflow not found"))
    }

    fn release_ready_workflow_nodes(&self, workflow_id: &str) -> anyhow::Result<()> {
        let workflow = self
            .get_workflow(workflow_id)?
            .ok_or_else(|| anyhow::anyhow!("workflow not found"))?;
        if workflow.pointer("/status/state").and_then(Value::as_str) != Some("running") {
            return Ok(());
        }
        let nodes = parse_workflow_nodes(
            workflow
                .pointer("/spec/nodes")
                .ok_or_else(|| anyhow::anyhow!("workflow nodes missing"))?,
        )?;
        let run_status = self.workflow_run_status_map(workflow_id)?;
        for node in nodes {
            if run_status.get(&node.id).map(String::as_str) != Some("pending") {
                continue;
            }
            if !node.depends_on.iter().all(|dependency| {
                matches!(
                    run_status.get(dependency).map(String::as_str),
                    Some("done" | "skipped")
                )
            }) {
                continue;
            }
            let context = self.workflow_context(workflow_id)?;
            let rendered_node = match render_workflow_node(&node, &context) {
                Ok(rendered_node) => rendered_node,
                Err(error) => {
                    let now = now();
                    let error_json = json!({
                        "code": "workflow_template_render_failed",
                        "message": error.to_string(),
                        "workflow_node_id": node.id
                    });
                    self.conn.execute(
                        "
                        UPDATE workflow_runs
                        SET status = 'failed', updated_at = ?1, completed_at = ?1, error_json = ?2
                        WHERE workflow_id = ?3 AND workflow_node_id = ?4
                        ",
                        params![
                            now,
                            serde_json::to_string(&error_json)?,
                            workflow_id,
                            node.id
                        ],
                    )?;
                    self.audit(
                        "workflow.node.render_failed",
                        "workflow-engine",
                        Some(workflow_id),
                        "工作流节点模板渲染失败",
                        json!({
                            "workflow_id": workflow_id,
                            "workflow_node_id": node.id,
                            "error": error_json
                        }),
                    )?;
                    self.refresh_workflow_state(workflow_id)?;
                    return Ok(());
                }
            };
            let task = self.create_task(workflow_node_task_payload(workflow_id, &rendered_node))?;
            let task_id = task
                .item
                .pointer("/metadata/id")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("workflow task id missing"))?
                .to_string();
            let now = now();
            self.conn.execute(
                "
                UPDATE workflow_runs
                SET status = 'ready', task_id = ?1, updated_at = ?2, started_at = ?2
                WHERE workflow_id = ?3 AND workflow_node_id = ?4
                ",
                params![task_id, now, workflow_id, node.id],
            )?;
            self.audit(
                "workflow.node.released",
                "workflow-engine",
                Some(workflow_id),
                &format!("工作流节点已释放：{}", node.title),
                json!({
                    "workflow_id": workflow_id,
                    "workflow_node_id": rendered_node.id,
                    "task_id": task_id,
                    "depends_on": rendered_node.depends_on,
                    "context_keys": workflow_context_keys(&context)
                }),
            )?;
        }
        self.refresh_workflow_state(workflow_id)?;
        Ok(())
    }

    fn workflow_context(&self, workflow_id: &str) -> anyhow::Result<Value> {
        let runs = self.list_workflow_runs(workflow_id)?;
        let mut steps = serde_json::Map::new();
        for run in runs {
            let node_id = run
                .pointer("/metadata/workflow_node_id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if node_id.is_empty() {
                continue;
            }
            let task_id = run
                .pointer("/metadata/task_id")
                .and_then(Value::as_str)
                .map(ToString::to_string);
            let task = task_id
                .as_deref()
                .map(|id| self.get_task(id))
                .transpose()?
                .flatten();
            let result = task
                .as_ref()
                .and_then(|task| task.pointer("/status/result"))
                .cloned()
                .or_else(|| run.pointer("/status/result").cloned())
                .unwrap_or(Value::Null);
            let error = task
                .as_ref()
                .and_then(|task| task.pointer("/status/error"))
                .cloned()
                .or_else(|| run.pointer("/status/error").cloned())
                .unwrap_or(Value::Null);
            steps.insert(
                node_id.clone(),
                json!({
                    "id": node_id,
                    "task_id": task_id,
                    "state": run.pointer("/status/state").and_then(Value::as_str).unwrap_or("unknown"),
                    "started_at": run.pointer("/status/started_at").cloned().unwrap_or(Value::Null),
                    "completed_at": run.pointer("/status/completed_at").cloned().unwrap_or(Value::Null),
                    "leased_by_node_id": task.as_ref().and_then(|task| task.pointer("/status/leased_by_node_id").and_then(Value::as_str)),
                    "result": result,
                    "error": error
                }),
            );
        }
        Ok(json!({
            "workflow": {
                "id": workflow_id
            },
            "steps": steps
        }))
    }

    fn workflow_run_status_map(
        &self,
        workflow_id: &str,
    ) -> anyhow::Result<HashMap<String, String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT workflow_node_id, status FROM workflow_runs WHERE workflow_id = ?1")?;
        let rows = stmt.query_map(params![workflow_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect::<Result<HashMap<_, _>, _>>()
            .map_err(Into::into)
    }

    fn list_workflow_runs(&self, workflow_id: &str) -> anyhow::Result<Vec<Value>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT * FROM workflow_runs
            WHERE project_id = ?1 AND workflow_id = ?2
            ORDER BY created_at ASC
            ",
        )?;
        let rows = stmt.query_map(params![PROJECT_ID, workflow_id], workflow_run_row)?;
        collect_values(rows)
    }

    fn list_workflow_runs_with_tasks(&self, workflow_id: &str) -> anyhow::Result<Vec<Value>> {
        self.list_workflow_runs(workflow_id)?
            .into_iter()
            .map(|mut run| {
                let task = run
                    .pointer("/metadata/task_id")
                    .and_then(Value::as_str)
                    .map(|id| self.get_task(id))
                    .transpose()?
                    .flatten();
                if let Some(task) = task {
                    if let Some(spec) = run.get_mut("spec").and_then(Value::as_object_mut) {
                        spec.insert(
                            "task".to_string(),
                            json!({
                                "id": task.pointer("/metadata/id").and_then(Value::as_str),
                                "title": task.pointer("/spec/title").and_then(Value::as_str),
                                "summary": task.pointer("/spec/summary").and_then(Value::as_str),
                                "labels": task.pointer("/spec/labels").cloned().unwrap_or_else(|| json!([])),
                                "inputs": task.pointer("/spec/inputs").cloned().unwrap_or_else(|| json!([])),
                                "leased_by_node_id": task.pointer("/status/leased_by_node_id").and_then(Value::as_str),
                                "state": task.pointer("/status/state").and_then(Value::as_str)
                            }),
                        );
                    }
                }
                Ok(run)
            })
            .collect()
    }

    fn refresh_workflow_state(&self, workflow_id: &str) -> anyhow::Result<()> {
        let runs = self.list_workflow_runs(workflow_id)?;
        if runs.is_empty() {
            return Ok(());
        }
        let now = now();
        if runs.iter().any(|run| {
            matches!(
                run.pointer("/status/state").and_then(Value::as_str),
                Some("failed" | "stopped" | "cancelled")
            )
        }) {
            self.conn.execute(
                "
                UPDATE workflows
                SET status = 'failed', completed_at = COALESCE(completed_at, ?1),
                    updated_at = ?1, error_json = COALESCE(error_json, ?2)
                WHERE id = ?3
                ",
                params![
                    now,
                    serde_json::to_string(&json!({
                        "code": "workflow_node_failed",
                        "message": "工作流节点失败，工作流已停止推进"
                    }))?,
                    workflow_id,
                ],
            )?;
            return Ok(());
        }
        if runs.iter().all(|run| {
            matches!(
                run.pointer("/status/state").and_then(Value::as_str),
                Some("done" | "skipped")
            )
        }) {
            let aggregate = self.workflow_result_aggregate(workflow_id, &runs, &now)?;
            self.conn.execute(
                "
                UPDATE workflows
                SET status = 'done', completed_at = COALESCE(completed_at, ?1),
                    updated_at = ?1, result_json = ?2
                WHERE id = ?3
                ",
                params![now, serde_json::to_string(&aggregate)?, workflow_id,],
            )?;
            self.audit(
                "workflow.completed",
                "workflow-engine",
                Some(workflow_id),
                "工作流执行完成",
                json!({ "workflow_id": workflow_id, "done_count": runs.len() }),
            )?;
        }
        Ok(())
    }

    fn workflow_result_aggregate(
        &self,
        workflow_id: &str,
        runs: &[Value],
        completed_at: &str,
    ) -> anyhow::Result<Value> {
        let workflow = self
            .get_workflow(workflow_id)?
            .ok_or_else(|| anyhow::anyhow!("workflow not found"))?;
        let nodes = parse_workflow_nodes(
            workflow
                .pointer("/spec/nodes")
                .ok_or_else(|| anyhow::anyhow!("workflow nodes missing"))?,
        )?;
        let mut run_by_node_id = runs
            .iter()
            .filter_map(|run| {
                run.pointer("/metadata/workflow_node_id")
                    .and_then(Value::as_str)
                    .map(|node_id| (node_id.to_string(), run))
            })
            .collect::<HashMap<_, _>>();
        let mut ordered_runs = Vec::with_capacity(runs.len());
        for node in nodes {
            if let Some(run) = run_by_node_id.remove(&node.id) {
                ordered_runs.push(run);
            }
        }
        ordered_runs.extend(run_by_node_id.into_values());

        let mut steps = Vec::with_capacity(ordered_runs.len());
        for run in ordered_runs {
            let task_id = run
                .pointer("/metadata/task_id")
                .and_then(Value::as_str)
                .map(ToString::to_string);
            let task = task_id
                .as_deref()
                .map(|id| self.get_task(id))
                .transpose()?
                .flatten();
            let result = task
                .as_ref()
                .and_then(|task| task.pointer("/status/result"))
                .cloned()
                .or_else(|| run.pointer("/status/result").cloned())
                .unwrap_or(Value::Null);
            let error = task
                .as_ref()
                .and_then(|task| task.pointer("/status/error"))
                .cloned()
                .or_else(|| run.pointer("/status/error").cloned())
                .unwrap_or(Value::Null);
            steps.push(json!({
                "workflow_node_id": run.pointer("/metadata/workflow_node_id").and_then(Value::as_str),
                "run_id": run.pointer("/metadata/id").and_then(Value::as_str),
                "task_id": task_id,
                "state": run.pointer("/status/state").and_then(Value::as_str).unwrap_or("unknown"),
                "started_at": run.pointer("/status/started_at").cloned().unwrap_or(Value::Null),
                "completed_at": run.pointer("/status/completed_at").cloned().unwrap_or(Value::Null),
                "leased_by_node_id": task.as_ref().and_then(|task| task.pointer("/status/leased_by_node_id").and_then(Value::as_str)),
                "result": result,
                "error": error
            }));
        }
        Ok(json!({
            "type": "workflow_result",
            "workflow_id": workflow_id,
            "done_count": count_runs(runs, "done"),
            "skipped_count": count_runs(runs, "skipped"),
            "completed_at": completed_at,
            "steps": steps
        }))
    }

    fn list_tasks(&self, query: TaskQuery) -> anyhow::Result<Vec<Value>> {
        let limit = query.limit.unwrap_or(100).min(500);
        let mut sql = "SELECT * FROM agent_tasks WHERE project_id = ?1".to_string();
        let mut values = vec![PROJECT_ID.to_string()];
        if let Some(owner) = query.owner {
            sql.push_str(" AND owner_agent_id = ?");
            values.push(owner);
        }
        if let Some(state) = query.state {
            sql.push_str(" AND status = ?");
            values.push(state);
        }
        sql.push_str(" ORDER BY updated_at DESC LIMIT ?");
        values.push(limit.to_string());

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(values), task_row)?;
        collect_values(rows)
    }

    fn get_task(&self, id: &str) -> anyhow::Result<Option<Value>> {
        self.conn
            .query_row(
                "SELECT * FROM agent_tasks WHERE id = ?1",
                params![id],
                task_row,
            )
            .optional()
            .map_err(Into::into)
    }

    fn create_task(&self, data: Value) -> anyhow::Result<TaskOutput> {
        let id = string_or(&data, "id", &new_id("task"));
        let now = now();
        let owner = optional_string(&data, "owner");
        let mut assigned_to = array_field(&data, "assigned_to");
        if let Some(owner) = owner.as_ref() {
            if !assigned_to.iter().any(|item| item == owner) {
                assigned_to.insert(0, owner.clone());
            }
        }
        let status = if assigned_to.is_empty() && owner.is_none() {
            "todo"
        } else {
            "assigned"
        };
        self.conn.execute(
            "
            INSERT INTO agent_tasks (
                id, project_id, title, summary, created_by, owner_agent_id, status, priority,
                inputs_json, outputs_json, acceptance_criteria_json, progress, blocked_reason,
                assigned_to_json, labels_json, depends_on_json, due_at, started_at, completed_at,
                assignment_message_id, last_message_id, correlation_id, workflow_id, workflow_node_id, job_id, job_attempt_id, job_shard_id, verify_json,
                created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, NULL, ?13, ?14, ?15, ?16, NULL, NULL, NULL, NULL, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?24)
            ",
            params![
                id,
                PROJECT_ID,
                required_string(&data, "title")?,
                string_or(&data, "summary", ""),
                required_string(&data, "created_by")?,
                owner,
                status,
                string_or(&data, "priority", "normal"),
                serde_json::to_string(&array_field(&data, "inputs"))?,
                serde_json::to_string(&array_field(&data, "outputs"))?,
                serde_json::to_string(&array_field(&data, "acceptance_criteria"))?,
                number_or(&data, "progress", 0),
                serde_json::to_string(&assigned_to)?,
                serde_json::to_string(&array_field(&data, "labels"))?,
                serde_json::to_string(&array_field(&data, "depends_on"))?,
                optional_string(&data, "due_at"),
                optional_string(&data, "correlation_id"),
                optional_string(&data, "workflow_id"),
                optional_string(&data, "workflow_node_id"),
                optional_string(&data, "job_id"),
                optional_string(&data, "job_attempt_id"),
                optional_string(&data, "job_shard_id"),
                optional_json_value_string(&data, "verify")?,
                now,
            ],
        )?;
        self.audit(
            "task.created",
            &required_string(&data, "created_by")?,
            Some(&id),
            &required_string(&data, "title")?,
            data.clone(),
        )?;
        let mut message_id = None;
        if status == "assigned" {
            let msg = self.create_message(json!({
                "from": required_string(&data, "created_by")?,
                "to": assigned_to,
                "type": "task.assigned",
                "subject": format!("任务：{}", required_string(&data, "title")?),
                "summary": string_or(&data, "summary", ""),
                "priority": string_or(&data, "priority", "normal"),
                "requires_ack": true,
                "payload": { "task_id": id }
            }))?;
            message_id = msg
                .pointer("/metadata/id")
                .and_then(Value::as_str)
                .map(ToString::to_string);
            self.conn.execute(
                "UPDATE agent_tasks SET assignment_message_id = ?1, last_message_id = ?1 WHERE id = ?2",
                params![message_id, id],
            )?;
        }
        Ok(TaskOutput {
            item: self
                .get_task(&id)?
                .ok_or_else(|| anyhow::anyhow!("task not found"))?,
            message_id,
        })
    }

    fn create_agent_runtime_task(&self, data: Value) -> anyhow::Result<TaskOutput> {
        let tool_id = required_string(&data, "tool_id")?;
        let selection = self
            .runtime_tool_selection(&tool_id)?
            .ok_or_else(|| anyhow::anyhow!("unknown tool_id: {tool_id}"))?;
        let tool = selection.tool;
        let payload = data
            .get("payload")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("payload is required"))?;
        let payload = if selection.dynamic {
            dynamic_runtime_payload(&tool_id, &tool, payload)?
        } else {
            payload
        };
        let mut labels = tool
            .get("labels")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| item.as_str().map(ToString::to_string))
            .collect::<Vec<_>>();
        if let Some(node_id) = optional_string(&data, "node_id") {
            ensure_label(&mut labels, &format!("node:{node_id}"));
        }
        if let Some(os) = optional_string(&data, "os") {
            ensure_label(&mut labels, &format!("os:{os}"));
        }
        if let Some(group) = optional_string(&data, "group") {
            ensure_label(&mut labels, &format!("group:{group}"));
        }
        if let Some(prefer) = optional_string(&data, "prefer_node_id") {
            ensure_label(&mut labels, &format!("prefer:{prefer}"));
        }
        if let Some(avoid) = optional_string(&data, "avoid_node_id") {
            ensure_label(&mut labels, &format!("avoid:{avoid}"));
        }
        ensure_label(&mut labels, &format!("tool:{tool_id}"));
        if selection.dynamic {
            ensure_label(&mut labels, "dynamic_tool");
        }
        let owner = string_or(&data, "owner", "worker-agent");
        let created_by = string_or(&data, "created_by", "agent-runtime");
        let title = optional_string(&data, "title").unwrap_or_else(|| {
            format!(
                "{} via AgentRuntime",
                tool.get("name").and_then(Value::as_str).unwrap_or(&tool_id)
            )
        });
        let verify = data
            .get("verify")
            .cloned()
            .or_else(|| tool.get("default_verify").cloned());
        let mut task = json!({
            "title": title,
            "summary": string_or(&data, "summary", "Agent Runtime 提交的标准工具任务。"),
            "created_by": created_by,
            "owner": owner,
            "assigned_to": [owner],
            "priority": string_or(&data, "priority", "normal"),
            "labels": labels,
            "inputs": [serde_json::to_string_pretty(&payload)?],
            "outputs": tool.get("standard_outputs").cloned().unwrap_or_else(|| json!(["结构化结果", "执行耗时", "验收结果"])),
            "acceptance_criteria": [
                "AgentRuntime 使用 ToolContract 创建任务",
                "Hub 根据节点资源和工具可信度调度",
                "Worker 写回结构化结果",
                "Hub 统一执行结果验收"
            ],
            "correlation_id": optional_string(&data, "correlation_id")
        });
        if let Some(verify) = verify {
            if let Some(map) = task.as_object_mut() {
                map.insert("verify".to_string(), verify);
            }
        }
        self.create_task(task)
    }

    fn list_task_templates(&self, limit: u16) -> anyhow::Result<Vec<Value>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT * FROM task_templates
            WHERE project_id = ?1
            ORDER BY category ASC, updated_at DESC
            LIMIT ?2
            ",
        )?;
        let rows = stmt.query_map(params![PROJECT_ID, limit.min(500)], task_template_row)?;
        collect_values(rows)
    }

    fn get_task_template(&self, id: &str) -> anyhow::Result<Option<Value>> {
        self.conn
            .query_row(
                "SELECT * FROM task_templates WHERE project_id = ?1 AND id = ?2",
                params![PROJECT_ID, id],
                task_template_row,
            )
            .optional()
            .map_err(Into::into)
    }

    fn create_task_template_if_missing(&self, data: Value) -> anyhow::Result<Value> {
        let id = required_string(&data, "id")?;
        if let Some(existing) = self.get_task_template(&id)? {
            return Ok(existing);
        }
        self.insert_task_template(id, data)
    }

    fn insert_task_template(&self, id: String, data: Value) -> anyhow::Result<Value> {
        let now = now();
        let tool_id = required_string(&data, "tool_id")?;
        let tool = self
            .runtime_tool_selection(&tool_id)?
            .map(|selection| selection.tool)
            .ok_or_else(|| anyhow::anyhow!("unknown tool_id: {tool_id}"))?;
        let labels = data
            .get("labels")
            .cloned()
            .unwrap_or_else(|| tool.get("labels").cloned().unwrap_or_else(|| json!([])));
        self.conn.execute(
            "
            INSERT INTO task_templates (
                id, project_id, name, summary, category, tool_id, payload_json,
                parameters_json, verify_json, labels_json, created_by, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                summary = excluded.summary,
                category = excluded.category,
                tool_id = excluded.tool_id,
                payload_json = excluded.payload_json,
                parameters_json = excluded.parameters_json,
                verify_json = excluded.verify_json,
                labels_json = excluded.labels_json,
                updated_at = excluded.updated_at
            ",
            params![
                id,
                PROJECT_ID,
                required_string(&data, "name")?,
                string_or(&data, "summary", ""),
                string_or(&data, "category", "general"),
                tool_id,
                serde_json::to_string(data.get("payload").unwrap_or(&json!({})))?,
                serde_json::to_string(data.get("parameters").unwrap_or(&json!([])))?,
                optional_json_value_string(&data, "verify")?,
                serde_json::to_string(&labels)?,
                string_or(&data, "created_by", "architect-agent"),
                now,
            ],
        )?;
        self.get_task_template(&id)?
            .ok_or_else(|| anyhow::anyhow!("task template not found"))
    }

    fn start_task_template(&self, id: &str, data: Value) -> anyhow::Result<TaskOutput> {
        let template = self
            .get_task_template(id)?
            .ok_or_else(|| anyhow::anyhow!("task template not found"))?;
        let parameters = data.get("parameters").cloned().unwrap_or_else(|| json!({}));
        let payload = render_template_value(
            template
                .pointer("/spec/payload")
                .ok_or_else(|| anyhow::anyhow!("task template payload missing"))?,
            &parameters,
        );
        let verify = data
            .get("verify")
            .cloned()
            .or_else(|| template.pointer("/spec/verify").cloned())
            .filter(|value| !value.is_null());
        let mut request = json!({
            "tool_id": template.pointer("/spec/tool_id").and_then(Value::as_str).unwrap_or("command.run"),
            "title": data.get("title").and_then(Value::as_str).unwrap_or_else(|| template.pointer("/spec/name").and_then(Value::as_str).unwrap_or("模板任务")),
            "summary": template.pointer("/spec/summary").and_then(Value::as_str).unwrap_or("模板商店提交的任务"),
            "payload": payload,
            "created_by": string_or(&data, "created_by", "template-store"),
            "owner": string_or(&data, "owner", "worker-agent"),
            "priority": string_or(&data, "priority", "normal"),
            "node_id": optional_string(&data, "node_id"),
            "os": optional_string(&data, "os"),
            "group": optional_string(&data, "group"),
            "correlation_id": optional_string(&data, "correlation_id").unwrap_or_else(|| format!("template:{id}:{}", new_id("run")))
        });
        if let Some(verify) = verify {
            if let Some(map) = request.as_object_mut() {
                map.insert("verify".to_string(), verify);
            }
        }
        let output = self.create_agent_runtime_task(request)?;
        let task_id = output
            .item
            .pointer("/metadata/id")
            .and_then(Value::as_str)
            .unwrap_or("");
        self.audit(
            "task_template.started",
            &string_or(&data, "created_by", "template-store"),
            Some(task_id),
            "任务模板已启动",
            json!({ "template_id": id, "task_id": task_id, "parameters": parameters }),
        )?;
        Ok(output)
    }

    fn list_jobs(&self, query: JobQuery) -> anyhow::Result<Vec<Value>> {
        let limit = query.limit.unwrap_or(100).min(500);
        let mut sql = "SELECT * FROM jobs WHERE project_id = ?1".to_string();
        let mut values = vec![PROJECT_ID.to_string()];
        if let Some(state) = query.state {
            sql.push_str(" AND status = ?");
            values.push(state);
        }
        sql.push_str(" ORDER BY updated_at DESC LIMIT ?");
        values.push(limit.to_string());
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(values), job_row)?;
        collect_values(rows)
    }

    fn get_job(&self, id: &str) -> anyhow::Result<Option<Value>> {
        self.conn
            .query_row(
                "SELECT * FROM jobs WHERE project_id = ?1 AND id = ?2",
                params![PROJECT_ID, id],
                job_row,
            )
            .optional()
            .map_err(Into::into)
    }

    fn get_job_by_idempotency_key(&self, key: &str) -> anyhow::Result<Option<Value>> {
        self.conn
            .query_row(
                "SELECT * FROM jobs WHERE project_id = ?1 AND idempotency_key = ?2",
                params![PROJECT_ID, key],
                job_row,
            )
            .optional()
            .map_err(Into::into)
    }

    fn get_job_detail(&self, id: &str) -> anyhow::Result<Option<Value>> {
        let Some(mut job) = self.get_job(id)? else {
            return Ok(None);
        };
        let attempts = self.list_job_attempts(id)?;
        let shards = self.list_job_shards(id)?;
        let checkpoints = self.list_job_checkpoints(id, 100)?;
        if let Some(map) = job.as_object_mut() {
            map.insert("attempts".to_string(), Value::Array(attempts));
            map.insert("shards".to_string(), Value::Array(shards));
            map.insert("checkpoints".to_string(), Value::Array(checkpoints));
        }
        Ok(Some(job))
    }

    fn job_execution_view(&self, id: &str) -> anyhow::Result<Option<Value>> {
        let Some(job) = self.get_job_detail(id)? else {
            return Ok(None);
        };
        let attempts = job
            .get("attempts")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let checkpoints = job
            .get("checkpoints")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let events = self.list_audit_events_for_subject(id, 500)?;
        let timeline = job_execution_timeline(&job, &attempts, &checkpoints, &events);
        Ok(Some(json!({
            "api_version": "agentgrid.job-execution/v1",
            "kind": "JobExecutionView",
            "generated_at": now(),
            "job_id": id,
            "job": job,
            "summary": job_execution_summary(&attempts, &checkpoints, &events),
            "recovery": job_recovery_view(&job, &attempts, &checkpoints),
            "timeline": timeline,
            "attempts": attempts,
            "checkpoints": checkpoints,
            "events": events
        })))
    }

    fn list_job_attempts(&self, job_id: &str) -> anyhow::Result<Vec<Value>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT * FROM job_attempts
            WHERE project_id = ?1 AND job_id = ?2
            ORDER BY attempt_number ASC
            ",
        )?;
        let rows = stmt.query_map(params![PROJECT_ID, job_id], job_attempt_row)?;
        collect_values(rows)
    }

    fn list_job_shards(&self, job_id: &str) -> anyhow::Result<Vec<Value>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT * FROM job_shards
            WHERE project_id = ?1 AND job_id = ?2
            ORDER BY shard_index ASC
            ",
        )?;
        let rows = stmt.query_map(params![PROJECT_ID, job_id], job_shard_row)?;
        collect_values(rows)
    }

    fn list_job_checkpoints(&self, job_id: &str, limit: u16) -> anyhow::Result<Vec<Value>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT * FROM job_checkpoints
            WHERE project_id = ?1 AND job_id = ?2
            ORDER BY sequence DESC, created_at DESC
            LIMIT ?3
            ",
        )?;
        let rows = stmt.query_map(
            params![PROJECT_ID, job_id, limit.min(500)],
            job_checkpoint_row,
        )?;
        collect_values(rows)
    }

    fn plan_job(&self, data: Value) -> anyhow::Result<Value> {
        let normalized = self.normalize_job_request(data)?;
        let labels = self.job_plan_labels(&normalized)?;
        let payload = normalized
            .get("payload")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let task = json!({
            "id": "job_plan_preview",
            "title": normalized.get("title").and_then(Value::as_str).unwrap_or("Job plan preview"),
            "summary": normalized.get("summary").and_then(Value::as_str).unwrap_or("AgentGrid Job dry-run"),
            "created_by": normalized.get("created_by").and_then(Value::as_str).unwrap_or("job-plan"),
            "owner": "worker-agent",
            "assigned_to": ["worker-agent"],
            "priority": normalized.get("priority").and_then(Value::as_str).unwrap_or("normal"),
            "labels": labels,
            "inputs": [serde_json::to_string_pretty(&payload)?],
            "outputs": [],
            "acceptance_criteria": [],
            "progress": 0
        });
        let job = task_to_job(&preview_task_value(&task))?;
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM nodes WHERE project_id = ?1")?;
        let rows = stmt.query_map(params![PROJECT_ID], node_row)?;
        let raw_node_values = rows.collect::<Result<Vec<_>, _>>()?;
        let has_verified_node = job.spec.requirements.node_id.is_none()
            && raw_node_values.iter().any(|node_value| {
                if node_value.pointer("/status/state").and_then(Value::as_str) != Some("online") {
                    return false;
                }
                let Some(node_id) = node_value.pointer("/metadata/id").and_then(Value::as_str)
                else {
                    return false;
                };
                self.evaluate_trust_for_job(&job, node_id)
                    .map(|trust| trust.state == "verified")
                    .unwrap_or(false)
            });
        let mut candidates = Vec::new();
        let mut eligible_nodes = Vec::new();
        let mut rejected_nodes = Vec::new();
        for node_value in raw_node_values {
            let node = json_node_to_protocol(&node_value)?;
            let evaluation =
                self.evaluate_node_for_job(&node_value, &node, &job, has_verified_node)?;
            let eligible = evaluation
                .get("eligible")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if eligible {
                eligible_nodes.push(evaluation.clone());
            } else {
                rejected_nodes.push(evaluation.clone());
            }
            candidates.push(evaluation);
        }
        eligible_nodes.sort_by(|left, right| {
            left.get("score")
                .and_then(Value::as_f64)
                .unwrap_or(f64::MAX)
                .partial_cmp(
                    &right
                        .get("score")
                        .and_then(Value::as_f64)
                        .unwrap_or(f64::MAX),
                )
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let selected_node_id = eligible_nodes
            .first()
            .and_then(|node| node.get("node_id").and_then(Value::as_str))
            .map(ToString::to_string);
        let selected_score = eligible_nodes
            .first()
            .and_then(|node| node.get("score").and_then(Value::as_f64));
        let decision_reason = selected_node_id
            .as_ref()
            .map(|node_id| format!("selected eligible node {node_id} from dry-run candidates"))
            .unwrap_or_else(|| "no eligible online node matched job requirements".to_string());
        let shard_count = normalized
            .pointer("/strategy/shard_count")
            .and_then(Value::as_i64)
            .unwrap_or(1);
        let max_parallelism = normalized
            .pointer("/strategy/max_parallelism")
            .and_then(Value::as_i64)
            .unwrap_or(1);
        let estimated_attempts =
            if normalized.pointer("/strategy/type").and_then(Value::as_str) == Some("sharded") {
                shard_count
            } else {
                1
            };
        let warnings = job_plan_warnings(&normalized, &eligible_nodes);
        Ok(json!({
            "api_version": "agentgrid.job-plan/v1",
            "kind": "JobPlan",
            "generated_at": now(),
            "valid_payload": true,
            "can_run": selected_node_id.is_some(),
            "tool_id": normalized.get("tool_id").cloned().unwrap_or(Value::Null),
            "selected_node_id": selected_node_id,
            "selected_nodes": selected_node_id.iter().cloned().collect::<Vec<_>>(),
            "eligible_nodes": eligible_nodes,
            "rejected_nodes": rejected_nodes,
            "candidates": candidates,
            "decision": {
                "reason": decision_reason,
                "score": selected_score,
                "candidates": eligible_nodes.iter().map(|candidate| json!({
                    "node_id": candidate.get("node_id").cloned().unwrap_or(Value::Null),
                    "score": candidate.get("score").cloned().unwrap_or(Value::Null),
                    "available_slots": candidate.get("available_slots").cloned().unwrap_or(Value::Null)
                })).collect::<Vec<_>>()
            },
            "execution_shape": {
                "strategy": normalized.get("strategy").cloned().unwrap_or_else(|| json!({ "type": "single" })),
                "partition": normalized.get("partition").cloned().unwrap_or_else(|| json!({ "type": "none" })),
                "estimated_attempts": estimated_attempts,
                "max_parallelism": max_parallelism
            },
            "reliability": job_reliability_contract_from_request(&normalized),
            "retry_reschedule_contract": retry_reschedule_contract_from_request(&normalized),
            "warnings": warnings,
            "normalized_job": normalized
        }))
    }

    fn normalize_job_request(&self, data: Value) -> anyhow::Result<Value> {
        let tool_id = required_string(&data, "tool_id")?;
        self.runtime_tool_selection(&tool_id)?
            .ok_or_else(|| anyhow::anyhow!("unknown tool_id: {tool_id}"))?;
        let strategy = normalize_job_strategy(data.get("strategy"));
        let partition = normalize_job_partition(data.get("partition"))?;
        let strategy = attach_partition_to_strategy(strategy, &partition);
        let retry_policy = data.get("retry_policy").cloned().unwrap_or_else(|| {
            json!({ "max_attempts": 3, "on_node_lost": "reschedule", "on_process_failed": "reschedule_if_idempotent" })
        });
        let max_attempts = retry_policy
            .get("max_attempts")
            .and_then(Value::as_i64)
            .unwrap_or(3)
            .clamp(1, 20);
        Ok(json!({
            "title": optional_string(&data, "title").unwrap_or_else(|| format!("{tool_id} Job")),
            "summary": string_or(&data, "summary", ""),
            "created_by": string_or(&data, "created_by", "agentgrid-cli"),
            "priority": string_or(&data, "priority", "normal"),
            "tool_id": tool_id,
            "payload": data.get("payload").cloned().unwrap_or_else(|| json!({})),
            "placement": data.get("placement").cloned().unwrap_or_else(|| json!({})),
            "strategy": strategy,
            "partition": partition,
            "reduce": normalize_job_reduce(data.get("reduce")),
            "retry_policy": retry_policy,
            "checkpoint_policy": data.get("checkpoint_policy").cloned().unwrap_or_else(|| json!({ "enabled": true, "mode": "worker_reported" })),
            "idempotency": data.get("idempotency").cloned().unwrap_or_else(|| json!({})),
            "max_attempts": max_attempts
        }))
    }

    fn job_plan_labels(&self, normalized: &Value) -> anyhow::Result<Vec<String>> {
        let tool_id = required_string(normalized, "tool_id")?;
        let selection = self
            .runtime_tool_selection(&tool_id)?
            .ok_or_else(|| anyhow::anyhow!("unknown tool_id: {tool_id}"))?;
        let mut labels = selection
            .tool
            .get("labels")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| item.as_str().map(ToString::to_string))
            .collect::<Vec<_>>();
        ensure_label(&mut labels, &format!("tool:{tool_id}"));
        ensure_label(&mut labels, "job");
        if selection.dynamic {
            ensure_label(&mut labels, "dynamic_tool");
        }
        if let Some(node_id) = normalized
            .pointer("/placement/node_id")
            .and_then(Value::as_str)
        {
            ensure_label(&mut labels, &format!("node:{node_id}"));
        }
        if let Some(os) = normalized.pointer("/placement/os").and_then(Value::as_str) {
            ensure_label(&mut labels, &format!("os:{os}"));
        }
        if let Some(group) = normalized
            .pointer("/placement/group")
            .and_then(Value::as_str)
        {
            ensure_label(&mut labels, &format!("group:{group}"));
        }
        if let Some(prefer) = normalized
            .pointer("/placement/prefer_node_id")
            .or_else(|| normalized.pointer("/placement/prefer_node"))
            .and_then(Value::as_str)
        {
            ensure_label(&mut labels, &format!("prefer:{prefer}"));
        }
        if let Some(avoid) = normalized
            .pointer("/placement/avoid_node_id")
            .or_else(|| normalized.pointer("/placement/avoid_node"))
            .and_then(Value::as_str)
        {
            ensure_label(&mut labels, &format!("avoid:{avoid}"));
        }
        Ok(labels)
    }

    fn create_job(&self, data: Value) -> anyhow::Result<Value> {
        let data = self.normalize_job_request(data)?;
        let idempotency_key = data
            .pointer("/idempotency/key")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        if let Some(key) = idempotency_key.as_deref() {
            if let Some(mut existing) = self.get_job_by_idempotency_key(key)? {
                if let Some(map) = existing.get_mut("status").and_then(Value::as_object_mut) {
                    map.insert("idempotency_reused".to_string(), json!(true));
                    map.insert("idempotency_key".to_string(), json!(key));
                }
                self.audit(
                    "job.idempotency.reused",
                    &string_or(&data, "created_by", "agent-runtime"),
                    existing.pointer("/metadata/id").and_then(Value::as_str),
                    "Job 幂等键命中，返回已有 Job",
                    json!({ "idempotency_key": key, "input": data }),
                )?;
                return Ok(existing);
            }
        }
        let id = string_or(&data, "id", &new_id("job"));
        let now = now();
        let strategy = data
            .get("strategy")
            .cloned()
            .unwrap_or_else(|| json!({ "type": "single" }));
        let retry_policy = data
            .get("retry_policy")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let max_attempts = retry_policy
            .get("max_attempts")
            .and_then(Value::as_i64)
            .unwrap_or(3)
            .clamp(1, 20);
        self.conn.execute(
            "
            INSERT INTO jobs (
                id, project_id, title, summary, created_by, status, tool_id, payload_json,
                placement_json, strategy_json, reduce_json, retry_policy_json, checkpoint_policy_json, idempotency_json,
                idempotency_key, max_attempts, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, 'queued', ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?16)
            ",
            params![
                id,
                PROJECT_ID,
                required_string(&data, "title")?,
                string_or(&data, "summary", ""),
                string_or(&data, "created_by", "agent-runtime"),
                required_string(&data, "tool_id")?,
                serde_json::to_string(data.get("payload").unwrap_or(&json!({})))?,
                serde_json::to_string(data.get("placement").unwrap_or(&json!({})))?,
                serde_json::to_string(&strategy)?,
                serde_json::to_string(&normalize_job_reduce(data.get("reduce")))?,
                serde_json::to_string(&retry_policy)?,
                serde_json::to_string(
                    data.get("checkpoint_policy")
                        .unwrap_or(&json!({ "enabled": true }))
                )?,
                serde_json::to_string(data.get("idempotency").unwrap_or(&json!({})))?,
                idempotency_key,
                max_attempts,
                now,
            ],
        )?;
        self.audit(
            "job.created",
            &string_or(&data, "created_by", "agent-runtime"),
            Some(&id),
            "Job 已创建",
            data.clone(),
        )?;
        if strategy.get("type").and_then(Value::as_str) == Some("sharded") {
            self.create_job_shards(&id)?;
        } else {
            self.create_job_attempt(&id, None, "initial")?;
        }
        self.get_job_detail(&id)?
            .ok_or_else(|| anyhow::anyhow!("job not found after create"))
    }

    fn create_job_attempt(
        &self,
        job_id: &str,
        shard_id: Option<&str>,
        reason: &str,
    ) -> anyhow::Result<Value> {
        let job = self
            .get_job(job_id)?
            .ok_or_else(|| anyhow::anyhow!("job not found"))?;
        let shard = shard_id
            .map(|id| self.get_job_shard(id))
            .transpose()?
            .flatten();
        let attempts = self.count_job_attempts(job_id, shard_id)?;
        let max_attempts = job
            .pointer("/spec/retry_policy/max_attempts")
            .and_then(Value::as_i64)
            .or_else(|| job.pointer("/status/max_attempts").and_then(Value::as_i64))
            .unwrap_or(3);
        if attempts >= max_attempts {
            self.fail_job(
                job_id,
                json!({ "code": "max_attempts_exceeded", "message": "Job attempts exhausted" }),
            )?;
            anyhow::bail!("job max attempts exceeded");
        }
        let attempt_number = attempts + 1;
        let attempt_id = new_id("attempt");
        let latest_checkpoint_id = job
            .pointer("/status/latest_checkpoint_id")
            .and_then(Value::as_str);
        let tool_id = job
            .pointer("/spec/tool_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("job tool_id missing"))?;
        let payload = if let Some(shard) = shard.as_ref() {
            shard
                .pointer("/spec/payload")
                .cloned()
                .unwrap_or_else(|| json!({}))
        } else {
            job.pointer("/spec/payload")
                .cloned()
                .unwrap_or_else(|| json!({}))
        };
        let placement = job
            .pointer("/spec/placement")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let task = self.job_attempt_task_payload(
            &job,
            tool_id,
            payload,
            &attempt_id,
            latest_checkpoint_id,
            shard.as_ref(),
            placement,
        )?;
        let output = self.create_task(task)?;
        let task_id = output
            .item
            .pointer("/metadata/id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("job attempt task id missing"))?
            .to_string();
        let now = now();
        self.conn.execute(
            "
            INSERT INTO job_attempts (
                id, project_id, job_id, shard_id, attempt_number, task_id, status, reason,
                resume_checkpoint_id, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'queued', ?7, ?8, ?9, ?9)
            ",
            params![
                attempt_id,
                PROJECT_ID,
                job_id,
                shard_id,
                attempt_number,
                task_id,
                reason,
                latest_checkpoint_id,
                now
            ],
        )?;
        self.conn.execute(
            "
            UPDATE jobs
            SET status = 'running',
                current_attempt_id = ?1,
                current_task_id = ?2,
                updated_at = ?3
            WHERE id = ?4
            ",
            params![attempt_id, task_id, now, job_id],
        )?;
        if let Some(shard_id) = shard_id {
            self.conn.execute(
                "
                UPDATE job_shards
                SET status = 'running',
                    current_attempt_id = ?1,
                    current_task_id = ?2,
                    updated_at = ?3
                WHERE id = ?4
                ",
                params![attempt_id, task_id, now, shard_id],
            )?;
        }
        self.audit(
            "job.attempt.created",
            "job-runtime",
            Some(job_id),
            "Job Attempt 已创建",
            json!({ "job_id": job_id, "shard_id": shard_id, "attempt_id": attempt_id, "task_id": task_id, "attempt_number": attempt_number, "reason": reason, "resume_checkpoint_id": latest_checkpoint_id }),
        )?;
        self.get_job_detail(job_id)?
            .ok_or_else(|| anyhow::anyhow!("job not found after attempt"))
    }

    fn create_job_shards(&self, job_id: &str) -> anyhow::Result<()> {
        let job = self
            .get_job(job_id)?
            .ok_or_else(|| anyhow::anyhow!("job not found"))?;
        let strategy = job
            .pointer("/spec/strategy")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let shard_count = strategy
            .get("shard_count")
            .and_then(Value::as_i64)
            .unwrap_or(1)
            .clamp(1, 1024);
        let payload = job
            .pointer("/spec/payload")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let now = now();
        for index in 0..shard_count {
            let shard_id = format!("{}_shard_{index:04}", job_id);
            let partition = partition_for_shard(&strategy, index, shard_count)?;
            let shard_payload =
                inject_job_shard_payload(payload.clone(), index, shard_count, partition)?;
            self.conn.execute(
                "
                INSERT INTO job_shards (
                    id, project_id, job_id, shard_index, shard_count, status, payload_json,
                    created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, 'queued', ?6, ?7, ?7)
                ON CONFLICT(project_id, job_id, shard_index) DO NOTHING
                ",
                params![
                    shard_id,
                    PROJECT_ID,
                    job_id,
                    index,
                    shard_count,
                    serde_json::to_string(&shard_payload)?,
                    now,
                ],
            )?;
        }
        self.release_job_shards(job_id, "shard_initial")?;
        self.audit(
            "job.shards.created",
            "job-runtime",
            Some(job_id),
            "Job 已拆分为 Shards",
            json!({ "job_id": job_id, "shard_count": shard_count }),
        )?;
        Ok(())
    }

    fn release_job_shards(&self, job_id: &str, reason: &str) -> anyhow::Result<usize> {
        let job = self
            .get_job(job_id)?
            .ok_or_else(|| anyhow::anyhow!("job not found"))?;
        if !matches!(
            job.pointer("/status/state").and_then(Value::as_str),
            Some("queued" | "running")
        ) {
            return Ok(0);
        }
        let max_parallelism = job
            .pointer("/spec/strategy/max_parallelism")
            .and_then(Value::as_i64)
            .unwrap_or(1)
            .clamp(1, 1024);
        let running = self.count_job_shards_by_status(job_id, "running")?;
        let available = max_parallelism.saturating_sub(running);
        if available == 0 {
            return Ok(0);
        }
        let queued = self.queued_job_shard_ids(job_id, available)?;
        let mut released = 0usize;
        for shard_id in queued {
            let claimed = self.conn.execute(
                "
                UPDATE job_shards
                SET status = 'releasing',
                    updated_at = ?1
                WHERE id = ?2 AND status = 'queued'
                ",
                params![now(), shard_id],
            )?;
            if claimed == 0 {
                continue;
            }
            self.create_job_attempt(job_id, Some(&shard_id), reason)?;
            released += 1;
        }
        if released > 0 {
            self.audit(
                "job.shards.released",
                "job-runtime",
                Some(job_id),
                "Job Shards 已按并行上限释放",
                json!({ "job_id": job_id, "released": released, "max_parallelism": max_parallelism }),
            )?;
        }
        Ok(released)
    }

    fn get_job_shard(&self, shard_id: &str) -> anyhow::Result<Option<Value>> {
        self.conn
            .query_row(
                "SELECT * FROM job_shards WHERE project_id = ?1 AND id = ?2",
                params![PROJECT_ID, shard_id],
                job_shard_row,
            )
            .optional()
            .map_err(Into::into)
    }

    fn queued_job_shard_ids(&self, job_id: &str, limit: i64) -> anyhow::Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT id FROM job_shards
            WHERE project_id = ?1 AND job_id = ?2 AND status = 'queued'
            ORDER BY shard_index ASC
            LIMIT ?3
            ",
        )?;
        let rows = stmt.query_map(params![PROJECT_ID, job_id, limit], |row| row.get(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn count_job_shards_by_status(&self, job_id: &str, status: &str) -> anyhow::Result<i64> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM job_shards WHERE project_id = ?1 AND job_id = ?2 AND status = ?3",
                params![PROJECT_ID, job_id, status],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    fn count_job_shards(&self, job_id: &str) -> anyhow::Result<i64> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM job_shards WHERE project_id = ?1 AND job_id = ?2",
                params![PROJECT_ID, job_id],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    fn complete_sharded_job_with_reduce(
        &self,
        job_id: &str,
        completed_at: &str,
    ) -> anyhow::Result<()> {
        let job = self
            .get_job(job_id)?
            .ok_or_else(|| anyhow::anyhow!("job not found"))?;
        let reduce = job
            .pointer("/spec/reduce")
            .cloned()
            .unwrap_or_else(|| json!({ "type": "summary" }));
        let reducer_task = self.create_reducer_task(job_id, &job, &reduce)?;
        let reducer_task_id = reducer_task
            .item
            .pointer("/metadata/id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("reducer task id missing"))?
            .to_string();
        let reduced = self.reduce_job_shards(job_id, &reduce, completed_at)?;
        self.conn.execute(
            "
            UPDATE agent_tasks
            SET status = 'done',
                progress = 100,
                completed_at = ?1,
                updated_at = ?1,
                result_json = ?2,
                error_json = NULL
            WHERE id = ?3
            ",
            params![
                completed_at,
                serde_json::to_string(&reduced)?,
                reducer_task_id
            ],
        )?;
        self.conn.execute(
            "
            UPDATE jobs
            SET status = 'done',
                completed_at = ?1,
                reducer_task_id = ?2,
                result_json = ?3,
                updated_at = ?1
            WHERE id = ?4
            ",
            params![
                completed_at,
                reducer_task_id,
                serde_json::to_string(&reduced)?,
                job_id
            ],
        )?;
        self.audit(
            "job.reduced",
            "job-runtime",
            Some(job_id),
            "Sharded Job 已完成 Reduce 汇总",
            json!({ "job_id": job_id, "reducer_task_id": reducer_task_id, "reduce": reduce, "result": reduced }),
        )?;
        self.enqueue_webhook_deliveries(
            "job.completed",
            Some(job_id),
            &json!({ "job_id": job_id, "reducer_task_id": reducer_task_id, "result": reduced }),
        )?;
        Ok(())
    }

    fn create_reducer_task(
        &self,
        job_id: &str,
        job: &Value,
        reduce: &Value,
    ) -> anyhow::Result<TaskOutput> {
        let shards = self.list_job_shards(job_id)?;
        let input = json!({
            "type": "job_reduce",
            "job_id": job_id,
            "reduce": reduce,
            "shards": shards
        });
        self.create_task(json!({
            "title": format!("{} / reduce", job.pointer("/spec/title").and_then(Value::as_str).unwrap_or("AgentGrid Job")),
            "summary": "Hub 内置 reducer 汇总所有 shard 结果，生成 Job 最终结果。",
            "created_by": "job-runtime",
            "owner": "job-runtime",
            "assigned_to": ["job-runtime"],
            "priority": job.pointer("/spec/priority").and_then(Value::as_str).unwrap_or("normal"),
            "labels": ["job", "reduce", "tool:job.reduce"],
            "inputs": [serde_json::to_string_pretty(&input)?],
            "outputs": ["summary", "success_count", "failed_count", "shards", "artifacts"],
            "acceptance_criteria": [
                "读取所有 shard 结果",
                "按 reduce 策略生成最终 Job result",
                "写回 reducer task 和 Job result"
            ],
            "job_id": job_id,
            "correlation_id": format!("job:{job_id}:reduce")
        }))
    }

    fn reduce_job_shards(
        &self,
        job_id: &str,
        reduce: &Value,
        completed_at: &str,
    ) -> anyhow::Result<Value> {
        let shards = self.list_job_shards(job_id)?;
        let total = shards.len();
        let success_count = shards
            .iter()
            .filter(|shard| {
                shard.pointer("/status/state").and_then(Value::as_str) == Some("done")
                    && shard.pointer("/status/error").is_none_or(Value::is_null)
            })
            .count();
        let failed_count = total.saturating_sub(success_count);
        let reducer_type = reduce
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("summary");
        let shard_results = shards
            .iter()
            .map(|shard| {
                json!({
                    "shard_id": shard.pointer("/metadata/id").cloned().unwrap_or(Value::Null),
                    "index": shard.pointer("/spec/shard_index").cloned().unwrap_or(Value::Null),
                    "node_id": shard.pointer("/status/node_id").cloned().unwrap_or(Value::Null),
                    "state": shard.pointer("/status/state").cloned().unwrap_or(Value::Null),
                    "result": shard.pointer("/status/result").cloned().unwrap_or(Value::Null),
                    "error": shard.pointer("/status/error").cloned().unwrap_or(Value::Null)
                })
            })
            .collect::<Vec<_>>();
        let artifacts = collect_reduce_artifacts(&shards);
        let reducer_result = match reducer_type {
            "stdout_concat" => json!({
                "type": "stdout_concat",
                "stdout": shards.iter()
                    .filter_map(|shard| shard.pointer("/status/result/stdout").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("")
            }),
            "json_array" => json!({
                "type": "json_array",
                "items": shard_results
                    .iter()
                    .filter_map(|item| item.get("result").cloned())
                    .collect::<Vec<_>>()
            }),
            _ => json!({
                "type": "summary",
                "message": format!("{success_count}/{total} shards succeeded")
            }),
        };
        Ok(json!({
            "type": "job_reduce_result",
            "job_id": job_id,
            "completed_at": completed_at,
            "reduce": reduce,
            "summary": {
                "state": if failed_count == 0 { "passed" } else { "failed" },
                "shard_count": total,
                "success_count": success_count,
                "failed_count": failed_count
            },
            "reducer_result": reducer_result,
            "shards": shard_results,
            "artifacts": artifacts
        }))
    }

    fn job_attempt_task_payload(
        &self,
        job: &Value,
        tool_id: &str,
        payload: Value,
        attempt_id: &str,
        checkpoint_id: Option<&str>,
        shard: Option<&Value>,
        placement: Value,
    ) -> anyhow::Result<Value> {
        let selection = self
            .runtime_tool_selection(tool_id)?
            .ok_or_else(|| anyhow::anyhow!("unknown tool_id: {tool_id}"))?;
        let tool = selection.tool;
        let mut payload = if selection.dynamic {
            dynamic_runtime_payload(tool_id, &tool, payload)?
        } else {
            payload
        };
        if let Some(checkpoint_id) = checkpoint_id {
            if let Some(checkpoint) = self.get_job_checkpoint(checkpoint_id)? {
                inject_payload_field(
                    &mut payload,
                    "resume_from",
                    json!({
                        "checkpoint_id": checkpoint_id,
                        "sequence": checkpoint.pointer("/status/sequence").cloned().unwrap_or(json!(0)),
                        "progress": checkpoint.pointer("/status/progress").cloned().unwrap_or(json!(0)),
                        "resume_token": checkpoint.pointer("/status/resume_token").cloned().unwrap_or_else(|| json!({})),
                        "artifacts": checkpoint.pointer("/status/artifacts").cloned().unwrap_or_else(|| json!([]))
                    }),
                )?;
            }
        }
        let mut labels = tool
            .get("labels")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| item.as_str().map(ToString::to_string))
            .collect::<Vec<_>>();
        ensure_label(&mut labels, &format!("tool:{tool_id}"));
        ensure_label(&mut labels, "job");
        if let Some(shard) = shard {
            if let Some(shard_id) = shard.pointer("/metadata/id").and_then(Value::as_str) {
                ensure_label(&mut labels, &format!("shard:{shard_id}"));
            }
        }
        if let Some(node_id) = placement.get("node_id").and_then(Value::as_str) {
            ensure_label(&mut labels, &format!("node:{node_id}"));
        }
        if let Some(os) = placement.get("os").and_then(Value::as_str) {
            ensure_label(&mut labels, &format!("os:{os}"));
        }
        if let Some(group) = placement.get("group").and_then(Value::as_str) {
            ensure_label(&mut labels, &format!("group:{group}"));
        }
        if let Some(avoid) = placement.get("avoid_node").and_then(Value::as_str) {
            ensure_label(&mut labels, &format!("avoid:{avoid}"));
        }
        let job_id = job
            .pointer("/metadata/id")
            .and_then(Value::as_str)
            .unwrap_or("");
        let shard_id = shard.and_then(|item| item.pointer("/metadata/id").and_then(Value::as_str));
        Ok(json!({
            "title": format!("{} / attempt {attempt_id}", job.pointer("/spec/title").and_then(Value::as_str).unwrap_or("AgentGrid Job")),
            "summary": job.pointer("/spec/summary").and_then(Value::as_str).unwrap_or("AgentGrid Job Runtime v1 attempt"),
            "created_by": job.pointer("/metadata/created_by").and_then(Value::as_str).unwrap_or("job-runtime"),
            "owner": "worker-agent",
            "assigned_to": ["worker-agent"],
            "priority": job.pointer("/spec/priority").and_then(Value::as_str).unwrap_or("normal"),
            "labels": labels,
            "inputs": [serde_json::to_string_pretty(&payload)?],
            "outputs": tool.get("standard_outputs").cloned().unwrap_or_else(|| json!(["结构化结果", "执行耗时", "验收结果"])),
            "acceptance_criteria": [
                "Worker 执行 Job Attempt",
                "Hub 记录 Attempt 状态",
                "失败或节点丢失时 Hub 可按 checkpoint 重调度"
            ],
            "job_id": job_id,
            "job_attempt_id": attempt_id,
            "job_shard_id": shard_id,
            "correlation_id": if let Some(shard_id) = shard_id { format!("job:{job_id}:shard:{shard_id}:attempt:{attempt_id}") } else { format!("job:{job_id}:attempt:{attempt_id}") },
            "verify": job.pointer("/spec/verify").cloned().unwrap_or_else(|| tool.get("default_verify").cloned().unwrap_or(Value::Null))
        }))
    }

    fn count_job_attempts(&self, job_id: &str, shard_id: Option<&str>) -> anyhow::Result<i64> {
        if let Some(shard_id) = shard_id {
            self.conn
                .query_row(
                    "SELECT COUNT(*) FROM job_attempts WHERE project_id = ?1 AND job_id = ?2 AND shard_id = ?3",
                    params![PROJECT_ID, job_id, shard_id],
                    |row| row.get(0),
                )
                .map_err(Into::into)
        } else {
            self.conn
                .query_row(
                    "SELECT COUNT(*) FROM job_attempts WHERE project_id = ?1 AND job_id = ?2 AND shard_id IS NULL",
                    params![PROJECT_ID, job_id],
                    |row| row.get(0),
                )
                .map_err(Into::into)
        }
    }

    fn create_job_checkpoint(&self, job_id: &str, data: Value) -> anyhow::Result<Value> {
        self.get_job(job_id)?
            .ok_or_else(|| anyhow::anyhow!("job not found"))?;
        let id = string_or(&data, "id", &new_id("checkpoint"));
        let now = now();
        let attempt_id = optional_string(&data, "attempt_id").or_else(|| {
            data.get("job_attempt_id")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        });
        let task_id = optional_string(&data, "task_id");
        self.conn.execute(
            "
            INSERT INTO job_checkpoints (
                id, project_id, job_id, attempt_id, task_id, node_id, sequence, progress,
                resume_token_json, artifacts_json, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ",
            params![
                id,
                PROJECT_ID,
                job_id,
                attempt_id,
                task_id,
                optional_string(&data, "node_id"),
                number_or(&data, "sequence", 0),
                number_or(&data, "progress", 0),
                serde_json::to_string(data.get("resume_token").unwrap_or(&json!({})))?,
                serde_json::to_string(data.get("artifacts").unwrap_or(&json!([])))?,
                now,
            ],
        )?;
        self.conn.execute(
            "UPDATE jobs SET latest_checkpoint_id = ?1, updated_at = ?2 WHERE id = ?3",
            params![id, now, job_id],
        )?;
        self.audit(
            "job.checkpoint.created",
            optional_string(&data, "node_id")
                .as_deref()
                .unwrap_or("job-runtime"),
            Some(job_id),
            "Job checkpoint 已记录",
            json!({ "job_id": job_id, "checkpoint_id": id, "input": data }),
        )?;
        self.get_job_checkpoint(&id)?
            .ok_or_else(|| anyhow::anyhow!("checkpoint not found after create"))
    }

    fn get_job_checkpoint(&self, id: &str) -> anyhow::Result<Option<Value>> {
        self.conn
            .query_row(
                "SELECT * FROM job_checkpoints WHERE project_id = ?1 AND id = ?2",
                params![PROJECT_ID, id],
                job_checkpoint_row,
            )
            .optional()
            .map_err(Into::into)
    }

    fn job_event_snapshot(&self, job_id: &str) -> anyhow::Result<Value> {
        let job = self
            .get_job_detail(job_id)?
            .ok_or_else(|| anyhow::anyhow!("job not found"))?;
        let events = self.list_events(
            EventQuery {
                limit: Some(100),
                event_type: None,
                type_alias: None,
                subject_id: Some(job_id.to_string()),
            },
            100,
        )?;
        Ok(json!({ "ok": true, "job_id": job_id, "time": now(), "job": job, "events": events }))
    }

    fn create_ingress_event(&self, data: Value) -> anyhow::Result<Value> {
        let id = string_or(&data, "event_id", &new_id("evt"));
        let idempotency_key =
            optional_string(&data, "idempotency_key").unwrap_or_else(|| id.clone());
        let now = now();
        self.conn.execute(
            "
            INSERT INTO ingress_events (
                id, project_id, source, target_json, event_type, idempotency_key,
                payload_json, status, ttl_seconds, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'accepted', ?8, ?9, ?9)
            ON CONFLICT(project_id, idempotency_key) DO UPDATE SET
                updated_at = excluded.updated_at
            ",
            params![
                id,
                PROJECT_ID,
                string_or(&data, "source", ""),
                serde_json::to_string(data.get("target").unwrap_or(&json!({})))?,
                required_string(&data, "type")?,
                idempotency_key,
                serde_json::to_string(data.get("payload").unwrap_or(&json!({})))?,
                number_or(&data, "ttl_seconds", 300),
                now,
            ],
        )?;
        self.audit(
            "event.ingress.accepted",
            data.get("source")
                .and_then(Value::as_str)
                .unwrap_or("external"),
            data.pointer("/target/job_id").and_then(Value::as_str),
            "外部事件已进入 AgentGrid",
            data.clone(),
        )?;
        self.get_ingress_event(&id)?
            .or_else(|| {
                self.get_ingress_event_by_key(&idempotency_key)
                    .ok()
                    .flatten()
            })
            .ok_or_else(|| anyhow::anyhow!("ingress event not found"))
    }

    fn get_ingress_event(&self, id: &str) -> anyhow::Result<Option<Value>> {
        self.conn
            .query_row(
                "SELECT * FROM ingress_events WHERE project_id = ?1 AND id = ?2",
                params![PROJECT_ID, id],
                ingress_event_row,
            )
            .optional()
            .map_err(Into::into)
    }

    fn get_ingress_event_by_key(&self, key: &str) -> anyhow::Result<Option<Value>> {
        self.conn
            .query_row(
                "SELECT * FROM ingress_events WHERE project_id = ?1 AND idempotency_key = ?2",
                params![PROJECT_ID, key],
                ingress_event_row,
            )
            .optional()
            .map_err(Into::into)
    }

    fn update_task(&self, id: &str, action: &str, data: Value) -> anyhow::Result<TaskOutput> {
        let current = self
            .get_task(id)?
            .ok_or_else(|| anyhow::anyhow!("task not found"))?;
        let current_state = current
            .pointer("/status/state")
            .and_then(Value::as_str)
            .unwrap_or("todo");
        let actor = required_string(&data, "actor")?;
        let now = now();
        let (next_state, progress, message_type) = match action {
            "accept" => (
                "in_progress".to_string(),
                number_or(&data, "progress", 1).max(1),
                "task.started",
            ),
            "progress" => (
                string_or(&data, "state", current_state),
                number_or(&data, "progress", 0),
                "task.progress",
            ),
            "block" => (
                "blocked".to_string(),
                number_or(&data, "progress", 0),
                "task.blocked",
            ),
            "complete" => ("review".to_string(), 100, "task.completed"),
            _ => anyhow::bail!("unknown task action"),
        };
        let message = self.create_message(json!({
            "from": actor,
            "to": array_field(&data, "notify"),
            "type": message_type,
            "subject": string_or(&data, "subject", &format!("{id} {next_state}")),
            "summary": string_or(&data, "summary", ""),
            "priority": string_or(&data, "priority", "normal"),
            "requires_ack": action == "complete",
            "payload": { "task_id": id, "state": next_state, "progress": progress }
        }))?;
        let message_id = message
            .pointer("/metadata/id")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        let completed_at = if action == "complete" {
            Some(now.clone())
        } else {
            None
        };
        self.conn.execute(
            "
            UPDATE agent_tasks
            SET status = ?1, progress = ?2, blocked_reason = ?3, started_at = COALESCE(started_at, ?4),
                completed_at = COALESCE(?5, completed_at), updated_at = ?4, last_message_id = ?6
            WHERE id = ?7
            ",
            params![
                next_state,
                progress,
                optional_string(&data, "reason"),
                now,
                completed_at,
                message_id,
                id,
            ],
        )?;
        Ok(TaskOutput {
            item: self
                .get_task(id)?
                .ok_or_else(|| anyhow::anyhow!("task not found"))?,
            message_id,
        })
    }

    fn control_task(&self, id: &str, data: Value) -> anyhow::Result<Value> {
        let current = self
            .get_task(id)?
            .ok_or_else(|| anyhow::anyhow!("task not found"))?;
        let action = required_string(&data, "action")?;
        let actor = string_or(&data, "actor", "architect-agent");
        let now = now();
        match action.as_str() {
            "cancel" => {
                self.conn.execute(
                    "
                    UPDATE agent_tasks
                    SET status = 'cancelled',
                        progress = 0,
                        lease_expires_at = NULL,
                        control_json = ?1,
                        updated_at = ?2
                    WHERE id = ?3 AND status IN ('assigned', 'todo')
                    ",
                    params![
                        serde_json::to_string(&json!({
                            "action": "cancel",
                            "requested_by": actor,
                            "requested_at": now,
                            "reason": string_or(&data, "reason", "任务已取消")
                        }))?,
                        now,
                        id
                    ],
                )?;
                self.audit("task.cancelled", &actor, Some(id), "任务已取消", data)?;
            }
            "stop" => {
                self.conn.execute(
                    "
                    UPDATE agent_tasks
                    SET status = 'stopping',
                        control_json = ?1,
                        blocked_reason = ?2,
                        updated_at = ?3
                    WHERE id = ?4
                    ",
                    params![
                        serde_json::to_string(&json!({
                            "action": "stop",
                            "requested_by": actor,
                            "requested_at": now,
                            "reason": string_or(&data, "reason", "请求停止正在执行的任务")
                        }))?,
                        string_or(&data, "reason", "请求停止正在执行的任务"),
                        now,
                        id
                    ],
                )?;
                self.audit(
                    "task.stop.requested",
                    &actor,
                    Some(id),
                    "任务停止请求已发送",
                    data,
                )?;
            }
            "requeue" => {
                self.conn.execute(
                    "
                    UPDATE agent_tasks
                    SET status = 'assigned',
                        progress = 0,
                        leased_by_node_id = NULL,
                        lease_expires_at = NULL,
                        control_json = NULL,
                        updated_at = ?1
                    WHERE id = ?2
                    ",
                    params![now, id],
                )?;
                self.audit("task.requeued", &actor, Some(id), "任务已重新入队", data)?;
            }
            "update_routing" => {
                let mut labels = current
                    .pointer("/spec/labels")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|item| item.as_str().map(ToString::to_string))
                    .filter(|label| {
                        !label.starts_with("node:")
                            && !label.starts_with("os:")
                            && !label.starts_with("prefer:")
                            && !label.starts_with("avoid:")
                    })
                    .collect::<Vec<_>>();
                if let Some(node_id) = optional_string(&data, "node_id") {
                    labels.push(format!("node:{node_id}"));
                }
                if let Some(os) = optional_string(&data, "os") {
                    labels.push(format!("os:{os}"));
                }
                if let Some(prefer) = optional_string(&data, "prefer_node_id") {
                    labels.push(format!("prefer:{prefer}"));
                }
                if let Some(avoid) = optional_string(&data, "avoid_node_id") {
                    labels.push(format!("avoid:{avoid}"));
                }
                self.conn.execute(
                    "UPDATE agent_tasks SET labels_json = ?1, updated_at = ?2 WHERE id = ?3",
                    params![serde_json::to_string(&labels)?, now, id],
                )?;
                self.audit(
                    "task.routing.changed",
                    &actor,
                    Some(id),
                    "任务路由已调整",
                    json!({ "labels": labels }),
                )?;
            }
            "update_priority" => {
                self.conn.execute(
                    "UPDATE agent_tasks SET priority = ?1, updated_at = ?2 WHERE id = ?3",
                    params![string_or(&data, "priority", "normal"), now, id],
                )?;
                self.audit(
                    "task.priority.changed",
                    &actor,
                    Some(id),
                    "任务优先级已调整",
                    data,
                )?;
            }
            other => anyhow::bail!("unknown control action: {other}"),
        }
        self.get_task(id)?
            .ok_or_else(|| anyhow::anyhow!("task not found"))
    }

    fn worker_task_control(&self, id: &str) -> anyhow::Result<Value> {
        let task = self
            .get_task(id)?
            .ok_or_else(|| anyhow::anyhow!("task not found"))?;
        let control = task
            .pointer("/status/control")
            .cloned()
            .unwrap_or_else(|| json!({}));
        Ok(json!({
            "ok": true,
            "task_id": id,
            "control": control,
            "state": task.pointer("/status/state").and_then(Value::as_str).unwrap_or("unknown")
        }))
    }

    fn renew_worker_task(&self, id: &str, data: Value) -> anyhow::Result<Value> {
        let node_id = required_string(&data, "node_id")?;
        let lease_seconds = number_or(&data, "lease_seconds", 120).clamp(10, 600);
        let task = self
            .get_task(id)?
            .ok_or_else(|| anyhow::anyhow!("task not found"))?;
        let state = task
            .pointer("/status/state")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let leased_by = task
            .pointer("/status/leased_by_node_id")
            .and_then(Value::as_str)
            .unwrap_or("");
        if state != "in_progress" {
            anyhow::bail!("task is not in_progress; current state is {state}");
        }
        if leased_by != node_id {
            anyhow::bail!("task lease owner mismatch");
        }
        let now_value = now();
        let lease_expires_at = (Utc::now() + chrono::Duration::seconds(lease_seconds))
            .to_rfc3339_opts(chrono::SecondsFormat::Micros, true);
        self.conn.execute(
            "
            UPDATE agent_tasks
            SET lease_expires_at = ?1,
                updated_at = ?2
            WHERE id = ?3
              AND status = 'in_progress'
              AND leased_by_node_id = ?4
            ",
            params![lease_expires_at, now_value, id, node_id],
        )?;
        self.audit(
            "task.lease.renewed",
            &node_id,
            Some(id),
            "任务租约已续期",
            json!({
                "node_id": node_id,
                "task_id": id,
                "lease_seconds": lease_seconds,
                "lease_expires_at": lease_expires_at
            }),
        )?;
        Ok(json!({
            "ok": true,
            "api_version": "agentgrid.worker-lease/v1",
            "kind": "WorkerLeaseRenewal",
            "task_id": id,
            "node_id": node_id,
            "lease_seconds": lease_seconds,
            "lease_expires_at": lease_expires_at
        }))
    }

    fn lease_tasks(&self, data: Value) -> anyhow::Result<Value> {
        let node_id = required_string(&data, "node_id")?;
        self.verify_node_request_auth(&node_id, &data)?;
        let max_tasks = number_or(&data, "max_tasks", 1).clamp(1, 64);
        let lease_seconds = number_or(&data, "lease_seconds", 60).clamp(10, 600);
        let capabilities = array_field(&data, "capabilities");
        let now = now();
        let lease_expires_at = (Utc::now() + chrono::Duration::seconds(lease_seconds))
            .to_rfc3339_opts(chrono::SecondsFormat::Micros, true);
        let requesting_node = self
            .get_node(&node_id)?
            .ok_or_else(|| anyhow::anyhow!("node not found"))?;
        let node_state = requesting_node
            .pointer("/status/state")
            .and_then(Value::as_str)
            .unwrap_or("offline");
        let auth_status = requesting_node
            .pointer("/spec/auth_status")
            .and_then(Value::as_str)
            .unwrap_or("legacy");
        if auth_status == "pending" {
            return Ok(json!({
                "ok": true,
                "node_id": node_id,
                "lease_seconds": lease_seconds,
                "tasks": [],
                "decision": {
                    "leased": false,
                    "reason": "节点已申请入网，但还未经过 Hub 超级管理员授权"
                }
            }));
        }
        if node_state != "online" {
            return Ok(json!({
                "ok": true,
                "node_id": node_id,
                "lease_seconds": lease_seconds,
                "tasks": [],
                "decision": {
                    "leased": false,
                    "reason": format!("节点当前状态是 {node_state}，不能接任务")
                }
            }));
        }

        let mut stmt = self.conn.prepare(
            "
            SELECT * FROM agent_tasks
            WHERE project_id = ?1
              AND (
                status IN ('assigned', 'todo')
                OR (status = 'in_progress' AND lease_expires_at IS NOT NULL AND lease_expires_at < ?2)
              )
              AND labels_json LIKE '%\"compute\"%'
              AND (lease_expires_at IS NULL OR lease_expires_at < ?2)
            ORDER BY
              CASE lower(priority)
                WHEN 'p0' THEN 0
                WHEN 'urgent' THEN 0
                WHEN 'high' THEN 1
                WHEN 'p1' THEN 1
                WHEN 'normal' THEN 2
                WHEN 'p2' THEN 2
                WHEN 'low' THEN 3
                ELSE 2
              END ASC,
              created_at ASC
            LIMIT 100
            ",
        )?;
        let rows = stmt.query_map(params![PROJECT_ID, now], task_row)?;
        let mut leased = Vec::new();
        for task in rows {
            let task = task?;
            if leased.len() >= max_tasks as usize {
                break;
            }
            if !task_matches_capabilities(&task, &capabilities) {
                continue;
            }
            let Some(task_tool_id) = tool_id_for_task(&task) else {
                continue;
            };
            if !self.node_supports_task_tool(&node_id, &task_tool_id)? {
                continue;
            }
            let decision = self.choose_best_node_for_task(&task)?;
            if decision.node_id.as_deref() != Some(&node_id) {
                continue;
            }
            let task_id = task
                .pointer("/metadata/id")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow::anyhow!("task id missing"))?
                .to_string();
            self.conn.execute(
                "
                UPDATE agent_tasks
                SET status = 'in_progress',
                    progress = CASE WHEN progress < 1 THEN 1 ELSE progress END,
                    leased_by_node_id = ?1,
                    lease_expires_at = ?2,
                    attempts = attempts + 1,
                    started_at = COALESCE(started_at, ?3),
                    updated_at = ?3
                WHERE id = ?4
                ",
                params![node_id, lease_expires_at, now, task_id],
            )?;
            self.mark_workflow_task_running(&task_id, &now)?;
            self.mark_job_attempt_running(&task_id, &node_id, &now)?;
            self.audit(
                "task.leased",
                &node_id,
                Some(&task_id),
                "任务租约已分配",
                json!({
                    "node_id": node_id,
                    "task_id": task_id,
                    "lease_expires_at": lease_expires_at,
                    "scheduler": {
                        "reason": decision.reason,
                        "score": decision.score,
                        "candidates": decision.candidates.iter().map(|candidate| json!({
                            "node_id": candidate.node_id,
                            "score": candidate.score,
                            "available_slots": candidate.available_slots
                        })).collect::<Vec<_>>()
                    }
                }),
            )?;
            leased.push(
                self.get_task(&task_id)?
                    .ok_or_else(|| anyhow::anyhow!("task not found after lease"))?,
            );
        }

        Ok(json!({
            "ok": true,
            "node_id": node_id,
            "lease_seconds": lease_seconds,
            "tasks": leased
        }))
    }

    fn verify_node_request_auth(&self, node_id: &str, data: &Value) -> anyhow::Result<()> {
        let Some(auth) = self.get_node_auth_record(node_id)? else {
            anyhow::bail!("node not found");
        };
        if auth.status == "legacy" && auth.join_token_hash.is_none() {
            return Ok(());
        }
        let token = optional_string(data, "join_token")
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("node join token required"))?;
        let token_hash = node_join_token_hash(node_id, &token);
        if auth.join_token_hash.as_deref() != Some(token_hash.as_str()) {
            anyhow::bail!("node join token rejected");
        }
        let fingerprint = optional_string(data, "machine_fingerprint")
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("machine fingerprint required"))?;
        if let Some(bound) = auth.machine_fingerprint.as_deref() {
            if bound != fingerprint {
                anyhow::bail!("machine fingerprint mismatch for node");
            }
        }
        Ok(())
    }

    fn worker_reconcile(&self, data: Value) -> anyhow::Result<Value> {
        let node_id = required_string(&data, "node_id")?;
        let records = data
            .get("records")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut items = Vec::new();
        let mut needs_attention = Vec::new();
        for record in records.into_iter().take(200) {
            let Some(task_id) = record.get("task_id").and_then(Value::as_str) else {
                continue;
            };
            let task = self.get_task(task_id)?;
            let hub_state = task
                .as_ref()
                .and_then(|task| task.pointer("/status/state").and_then(Value::as_str))
                .unwrap_or("missing");
            let journal_event = record.get("event").and_then(Value::as_str).unwrap_or("");
            let leased_by = task.as_ref().and_then(|task| {
                task.pointer("/status/leased_by_node_id")
                    .and_then(Value::as_str)
            });
            let action = reconcile_action(journal_event, hub_state, leased_by, &node_id);
            let recovery = reconcile_recovery(action, task.as_ref(), &record, &node_id);
            let item = json!({
                "task_id": task_id,
                "journal_event": journal_event,
                "hub_state": hub_state,
                "leased_by_node_id": leased_by,
                "action": action,
                "severity": recovery
                    .get("severity")
                    .and_then(Value::as_str)
                    .unwrap_or("info"),
                "recovery": recovery,
                "hub_snapshot": reconcile_hub_snapshot(task.as_ref()),
                "journal": record
            });
            if action != "none" {
                needs_attention.push(item.clone());
            }
            items.push(item);
        }
        self.audit(
            "worker.reconciled",
            &node_id,
            Some(&node_id),
            "Worker execution journal 已对账",
            json!({
                "node_id": node_id,
                "record_count": items.len(),
                "needs_attention_count": needs_attention.len(),
                "needs_attention": needs_attention
            }),
        )?;
        Ok(json!({
            "ok": true,
            "api_version": "agentgrid.worker-reconcile/v2",
            "kind": "WorkerReconcileResult",
            "node_id": node_id,
            "checked": items.len(),
            "summary": reconcile_summary(&items),
            "needs_attention": needs_attention,
            "items": items
        }))
    }

    fn choose_best_node_for_task(
        &self,
        task: &Value,
    ) -> anyhow::Result<agentgrid_scheduler::ScheduleDecision> {
        let job = task_to_job(task)?;
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM nodes WHERE project_id = ?1")?;
        let rows = stmt.query_map(params![PROJECT_ID], |row| node_from_row(row))?;
        let mut nodes = Vec::new();
        for node in rows {
            let node = node?;
            if node.status != NodeState::Online {
                continue;
            }
            if let Some(tool_id) = tool_id_for_job(&job) {
                if !self.node_supports_task_tool(&node.id, &tool_id)? {
                    continue;
                }
            }
            if score_node(&node) >= HIGH_LOAD_SCORE_LIMIT {
                continue;
            }
            let trust = self.evaluate_trust_for_job(&job, &node.id)?;
            if trust.state == "failed" && job.spec.requirements.node_id.as_deref() != Some(&node.id)
            {
                continue;
            }
            nodes.push(node);
        }
        let has_verified_node = job.spec.requirements.node_id.is_none()
            && nodes.iter().any(|node| {
                self.evaluate_trust_for_job(&job, &node.id)
                    .map(|trust| trust.state == "verified")
                    .unwrap_or(false)
            });
        if has_verified_node {
            nodes.retain(|node| {
                self.evaluate_trust_for_job(&job, &node.id)
                    .map(|trust| trust.state == "verified")
                    .unwrap_or(false)
            });
        }
        let mut decision = choose_node(&job, &nodes);
        if decision.node_id.is_some() {
            let eligible_node_ids = decision
                .candidates
                .iter()
                .map(|candidate| candidate.node_id.as_str())
                .collect::<HashSet<_>>();
            let mut best_node_id = decision.node_id.clone();
            let mut best_score = f64::MAX;
            let mut candidates = Vec::new();
            for candidate in &nodes {
                if !eligible_node_ids.contains(candidate.id.as_str()) {
                    continue;
                }
                let trust = self.evaluate_trust_for_job(&job, &candidate.id)?;
                let graph_multiplier = graph_multiplier_for_job(&job, &trust);
                let score = score_node(candidate)
                    * trust.multiplier
                    * trust.risk_multiplier
                    * graph_multiplier;
                let available_slots = candidate
                    .max_concurrent_jobs
                    .saturating_sub(candidate.running_jobs);
                candidates.push(agentgrid_scheduler::NodeScore {
                    node_id: candidate.id.clone(),
                    score,
                    available_slots,
                });
                if score < best_score {
                    best_score = score;
                    best_node_id = Some(candidate.id.clone());
                }
            }
            candidates.sort_by(|left, right| {
                left.score
                    .partial_cmp(&right.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            decision.node_id = best_node_id;
            decision.score = if best_score.is_finite() {
                Some(best_score)
            } else {
                None
            };
            decision.reason = decision
                .node_id
                .as_ref()
                .map(|node_id| {
                    let trust = self
                        .evaluate_trust_for_job(&job, node_id)
                        .unwrap_or_else(|_| default_trust_evaluation(None));
                    let verified_gate = if has_verified_node {
                        "; verified-only gate applied"
                    } else {
                        ""
                    };
                    format!(
                        "selected graph-aware eligible node {node_id}; {}; probe {:.2}; risk {} x{:.2}; graph x{:.2}{verified_gate}",
                        trust.reason,
                        trust.multiplier,
                        trust.risk,
                        trust.risk_multiplier,
                        graph_multiplier_for_job(&job, &trust)
                    )
                })
                .unwrap_or_else(|| "no eligible trusted node matched job requirements".to_string());
            decision.candidates = candidates;
        }
        Ok(decision)
    }

    fn requeue_lost_job_attempts(&self) -> anyhow::Result<Vec<Value>> {
        let now_value = now();
        let mut stmt = self.conn.prepare(
            "
            SELECT t.id, t.job_id, t.job_attempt_id, t.leased_by_node_id
            FROM agent_tasks t
            LEFT JOIN nodes n ON n.id = t.leased_by_node_id
            WHERE t.project_id = ?1
              AND t.job_id IS NOT NULL
              AND t.status = 'in_progress'
              AND t.leased_by_node_id IS NOT NULL
              AND (
                t.lease_expires_at < ?2
                OR n.id IS NULL
                OR n.status <> 'online'
                OR n.last_heartbeat_at < ?3
              )
            LIMIT 50
            ",
        )?;
        let offline_cutoff = (Utc::now()
            - chrono::Duration::seconds(HEARTBEAT_OFFLINE_AFTER_SECONDS))
        .to_rfc3339_opts(chrono::SecondsFormat::Micros, true);
        let rows = stmt.query_map(params![PROJECT_ID, now_value, offline_cutoff], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?;
        let items = rows.collect::<Result<Vec<_>, _>>()?;
        let mut recovered = Vec::new();
        for (task_id, job_id, attempt_id, node_id) in items {
            let error = json!({
                "code": "job_attempt_lost",
                "message": "Job attempt lease expired or node went offline",
                "task_id": task_id,
                "node_id": node_id
            });
            self.conn.execute(
                "
                UPDATE agent_tasks
                SET status = 'failed',
                    lease_expires_at = NULL,
                    error_json = ?1,
                    blocked_reason = 'Job attempt lost; rescheduled by Hub',
                    updated_at = ?2
                WHERE id = ?3
                ",
                params![serde_json::to_string(&error)?, now(), task_id],
            )?;
            self.conn.execute(
                "
                UPDATE job_attempts
                SET status = 'lost',
                    completed_at = ?1,
                    error_json = ?2,
                    updated_at = ?1
                WHERE id = ?3
                ",
                params![now(), serde_json::to_string(&error)?, attempt_id],
            )?;
            let Some(job) = self.get_job(&job_id)? else {
                continue;
            };
            let shard_id = self.job_attempt_shard_id(&attempt_id)?;
            let attempts = self.count_job_attempts(&job_id, shard_id.as_deref())?;
            let decision = retry_reschedule_decision(&job, "node_lost", attempts, Some(&error));
            let mut outcome = "failed";
            let mut next_attempt_id = Value::Null;
            if decision.get("should_reschedule").and_then(Value::as_bool) == Some(true) {
                let next_attempt =
                    self.create_job_attempt(&job_id, shard_id.as_deref(), "node_lost")?;
                next_attempt_id = next_attempt
                    .pointer("/metadata/id")
                    .cloned()
                    .unwrap_or(Value::Null);
                outcome = "rescheduled";
            } else {
                if let Some(shard_id) = shard_id.as_deref() {
                    self.fail_job_shard(&job_id, shard_id, error.clone())?;
                } else {
                    self.fail_job(&job_id, error.clone())?;
                }
            }
            let item = json!({
                "job_id": job_id,
                "attempt_id": attempt_id,
                "task_id": task_id,
                "node_id": node_id,
                "shard_id": shard_id,
                "outcome": outcome,
                "next_attempt_id": next_attempt_id,
                "error": error,
                "retry_decision": decision
            });
            self.audit(
                "job.attempt.lost",
                "job-runtime",
                item.get("job_id").and_then(Value::as_str),
                "Job Attempt 丢失并触发重调度",
                item.clone(),
            )?;
            recovered.push(item);
        }
        Ok(recovered)
    }

    fn task_schedule_preview(&self, id: &str) -> anyhow::Result<Value> {
        let task = self
            .get_task(id)?
            .ok_or_else(|| anyhow::anyhow!("task not found"))?;
        let job = task_to_job(&task)?;
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM nodes WHERE project_id = ?1")?;
        let rows = stmt.query_map(params![PROJECT_ID], node_row)?;
        let raw_node_values = rows.collect::<Result<Vec<_>, _>>()?;
        let has_verified_node = job.spec.requirements.node_id.is_none()
            && raw_node_values.iter().any(|node_value| {
                if node_value.pointer("/status/state").and_then(Value::as_str) != Some("online") {
                    return false;
                }
                let Some(node_id) = node_value.pointer("/metadata/id").and_then(Value::as_str)
                else {
                    return false;
                };
                self.evaluate_trust_for_job(&job, node_id)
                    .map(|trust| trust.state == "verified")
                    .unwrap_or(false)
            });
        let mut candidates = Vec::new();
        for node_value in raw_node_values {
            let node = json_node_to_protocol(&node_value)?;
            let evaluation =
                self.evaluate_node_for_job(&node_value, &node, &job, has_verified_node)?;
            candidates.push(evaluation);
        }
        let decision = self.choose_best_node_for_task(&task)?;
        let selected = decision.node_id.clone();
        Ok(json!({
            "task_id": id,
            "generated_at": now(),
            "selected_node_id": selected,
            "decision": {
                "node_id": decision.node_id,
                "reason": decision.reason,
                "score": decision.score,
                "candidates": decision.candidates.iter().map(|candidate| json!({
                    "node_id": candidate.node_id,
                    "score": candidate.score,
                    "available_slots": candidate.available_slots
                })).collect::<Vec<_>>()
            },
            "requirements": job.spec.requirements,
            "payload_type": task_type(&task),
            "candidates": candidates
        }))
    }

    fn evaluate_trust_for_job(&self, job: &Job, node_id: &str) -> anyhow::Result<TrustEvaluation> {
        let tool_id = tool_id_for_job(job);
        let Some(tool_id) = tool_id else {
            return Ok(default_trust_evaluation(None));
        };
        let risk = self.tool_risk(&tool_id)?.unwrap_or_else(|| {
            if is_dynamic_tool_id(&tool_id) {
                "high".to_string()
            } else {
                "medium".to_string()
            }
        });
        let probe = self.get_tool_probe(&tool_id, node_id)?;
        let Some(probe) = probe else {
            return Ok(default_trust_evaluation_with_risk(Some(tool_id), risk));
        };
        let state = probe
            .pointer("/status/state")
            .and_then(Value::as_str)
            .unwrap_or("declared_unverified")
            .to_string();
        let support_basis = probe
            .pointer("/spec/support_basis")
            .and_then(Value::as_str)
            .unwrap_or("node_heartbeat_capabilities")
            .to_string();
        let multiplier = trust_multiplier(&state);
        let reason = match state.as_str() {
            "verified" => format!("{tool_id} runtime probe verified"),
            "failed" => format!("{tool_id} runtime probe failed"),
            "pending" => format!("{tool_id} runtime probe pending"),
            "expired" => format!("{tool_id} runtime probe expired"),
            "unsupported" => format!("{tool_id} probe unsupported"),
            _ => format!("{tool_id} declared but not verified"),
        };
        let risk_multiplier = risk_multiplier(&risk, &state);
        Ok(TrustEvaluation {
            tool_id: Some(tool_id),
            state,
            support_basis,
            multiplier,
            risk,
            risk_multiplier,
            reason,
        })
    }

    fn tool_risk(&self, tool_id: &str) -> anyhow::Result<Option<String>> {
        if let Some(tool) = tool_registry()
            .into_iter()
            .find(|tool| tool.get("id").and_then(Value::as_str) == Some(tool_id))
        {
            return Ok(tool
                .get("risk")
                .and_then(Value::as_str)
                .map(ToString::to_string));
        }
        if let Some(catalog) = self.get_node_tool_catalog(tool_id)? {
            if let Some(risk) = catalog.get("risk").and_then(Value::as_str) {
                return Ok(Some(risk.to_string()));
            }
            if let Some(items) = catalog.get("nodes").and_then(Value::as_array) {
                for item in items {
                    if let Some(risk) = item.pointer("/metadata/risk").and_then(Value::as_str) {
                        return Ok(Some(risk.to_string()));
                    }
                }
            }
        }
        Ok(None)
    }

    fn evaluate_node_for_job(
        &self,
        node_value: &Value,
        node: &Node,
        job: &Job,
        verified_gate: bool,
    ) -> anyhow::Result<Value> {
        let mut evaluation = evaluate_node_for_job(node_value, node, job);
        if let Some(tool_id) = tool_id_for_job(job) {
            if is_dynamic_tool_id(&tool_id) {
                let supports_tool = self.node_supports_task_tool(&node.id, &tool_id)?;
                let mut dynamic_reasons = evaluation
                    .get("reasons")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|item| item.as_str().map(ToString::to_string))
                    .collect::<Vec<_>>();
                dynamic_reasons.retain(|reason| {
                    reason != "满足任务要求，可参与调度"
                        && !reason.contains(&format!("节点未注册动态工具 {tool_id}"))
                });
                if !supports_tool {
                    dynamic_reasons.push(format!("节点未注册动态工具 {tool_id}"));
                }
                let dynamic_eligible = dynamic_reasons.is_empty();
                let dynamic_reasons = if dynamic_eligible {
                    vec!["满足任务要求，可参与调度".to_string()]
                } else {
                    dynamic_reasons
                };
                if let Some(map) = evaluation.as_object_mut() {
                    map.insert("eligible".to_string(), json!(dynamic_eligible));
                    map.insert("reasons".to_string(), json!(dynamic_reasons));
                }
            }
        }
        let trust = self.evaluate_trust_for_job(job, &node.id)?;
        let mut reasons = evaluation
            .get("reasons")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| item.as_str().map(ToString::to_string))
            .collect::<Vec<_>>();
        if trust.state == "failed" {
            reasons.push(format!("工具验证失败：{}", trust.reason));
            if evaluation
                .get("eligible")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                && job.spec.requirements.node_id.as_deref() != Some(&node.id)
            {
                if let Some(map) = evaluation.as_object_mut() {
                    map.insert("eligible".to_string(), json!(false));
                }
            }
        } else if verified_gate && trust.state != "verified" {
            reasons.push("已有 verified 节点，本节点未通过运行时验证，可信调度跳过".to_string());
            if evaluation
                .get("eligible")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                if let Some(map) = evaluation.as_object_mut() {
                    map.insert("eligible".to_string(), json!(false));
                }
            }
        } else {
            reasons.push(format!("可信调度：{}", trust.reason));
        }
        let base_score = evaluation
            .get("score")
            .and_then(Value::as_f64)
            .unwrap_or_else(|| score_node(node));
        if let Some(map) = evaluation.as_object_mut() {
            map.insert(
                "trust".to_string(),
                json!({
                    "tool_id": trust.tool_id,
                    "state": trust.state,
                    "support_basis": trust.support_basis,
                    "multiplier": trust.multiplier,
                    "risk": trust.risk,
                    "risk_multiplier": trust.risk_multiplier,
                    "graph_multiplier": graph_multiplier_for_job(job, &trust),
                    "reason": trust.reason
                }),
            );
            map.insert(
                "score".to_string(),
                json!(
                    base_score
                        * trust.multiplier
                        * trust.risk_multiplier
                        * graph_multiplier_for_job(job, &trust)
                ),
            );
            map.insert("base_resource_score".to_string(), json!(base_score));
            map.insert("reasons".to_string(), json!(reasons));
        }
        Ok(evaluation)
    }

    fn complete_worker_task(&self, id: &str, data: Value) -> anyhow::Result<Value> {
        let now = now();
        let current_task = self
            .get_task(id)?
            .ok_or_else(|| anyhow::anyhow!("task not found"))?;
        let result = data.get("result").cloned().unwrap_or_else(|| json!({}));
        let result = apply_result_verification(&current_task, result, &now);
        let node_id = data
            .get("node_id")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        self.conn.execute(
            "
            UPDATE agent_tasks
            SET status = 'done',
                progress = 100,
                completed_at = ?1,
                updated_at = ?1,
                lease_expires_at = NULL,
                result_json = ?2,
                error_json = NULL
            WHERE id = ?3
            ",
            params![now, serde_json::to_string(&result)?, id],
        )?;
        self.extract_artifacts_from_result(id, node_id.as_deref(), &result)?;
        if self
            .get_task(id)?
            .and_then(|task| {
                task.pointer("/status/control/action")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            })
            .as_deref()
            == Some("stop")
        {
            self.conn.execute(
                "UPDATE agent_tasks SET blocked_reason = NULL, control_json = NULL WHERE id = ?1",
                params![id],
            )?;
        }
        self.audit(
            "task.completed",
            data.get("node_id")
                .and_then(Value::as_str)
                .unwrap_or("worker"),
            Some(id),
            "任务执行完成",
            data.clone(),
        )?;
        if let Some(verification) = result.get("verification") {
            let passed = verification
                .get("passed")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            self.audit(
                if passed {
                    "task.result.verified"
                } else {
                    "task.result.verification_failed"
                },
                "result-verifier",
                Some(id),
                if passed {
                    "任务结果验证通过"
                } else {
                    "任务结果验证失败"
                },
                verification.clone(),
            )?;
        }
        if let Some(node_id) = node_id.as_deref() {
            self.bump_node_result(node_id, true)?;
        }
        self.update_tool_probe_from_task(id, true, Some(&result), None, &now)?;
        self.mark_workflow_task_done(id, &result, &now)?;
        self.mark_job_task_done(id, &result, &now)?;
        self.enqueue_webhook_deliveries(
            "task.completed",
            Some(id),
            &json!({ "task_id": id, "node_id": node_id, "result": result }),
        )?;
        self.get_task(id)?
            .ok_or_else(|| anyhow::anyhow!("task not found"))
    }

    fn fail_worker_task(&self, id: &str, data: Value) -> anyhow::Result<Value> {
        let now = now();
        let error = data.get("error").cloned().unwrap_or_else(|| json!({}));
        let is_stopped = error.get("code").and_then(Value::as_str) == Some("task_stopped");
        let status = if is_stopped { "stopped" } else { "failed" };
        self.conn.execute(
            "
            UPDATE agent_tasks
            SET status = ?1,
                updated_at = ?2,
                lease_expires_at = NULL,
                control_json = NULL,
                error_json = ?3,
                blocked_reason = ?4
            WHERE id = ?5
            ",
            params![
                status,
                now,
                serde_json::to_string(&error)?,
                string_or(
                    &data,
                    "message",
                    if is_stopped {
                        "任务已停止"
                    } else {
                        "worker task failed"
                    }
                ),
                id
            ],
        )?;
        self.audit(
            if is_stopped {
                "task.stopped"
            } else {
                "task.failed"
            },
            data.get("node_id")
                .and_then(Value::as_str)
                .unwrap_or("worker"),
            Some(id),
            string_or(
                &data,
                "message",
                if is_stopped {
                    "任务已停止"
                } else {
                    "任务执行失败"
                },
            )
            .as_str(),
            data.clone(),
        )?;
        if let Some(node_id) = data.get("node_id").and_then(Value::as_str) {
            self.bump_node_result(node_id, false)?;
        }
        self.update_tool_probe_from_task(id, false, None, Some(&error), &now)?;
        self.mark_workflow_task_failed(id, &error, &now)?;
        if !is_stopped {
            self.reschedule_job_task_failure(id, &error, &now)?;
        }
        self.enqueue_webhook_deliveries(
            "task.failed",
            Some(id),
            &json!({ "task_id": id, "error": error, "status": status }),
        )?;
        self.get_task(id)?
            .ok_or_else(|| anyhow::anyhow!("task not found"))
    }

    fn mark_job_task_done(
        &self,
        task_id: &str,
        result: &Value,
        completed_at: &str,
    ) -> anyhow::Result<()> {
        let Some(task) = self.get_task(task_id)? else {
            return Ok(());
        };
        let Some(job_id) = task.pointer("/metadata/job_id").and_then(Value::as_str) else {
            return Ok(());
        };
        let Some(attempt_id) = task
            .pointer("/metadata/job_attempt_id")
            .and_then(Value::as_str)
        else {
            return Ok(());
        };
        self.conn.execute(
            "
            UPDATE job_attempts
            SET status = 'done',
                completed_at = ?1,
                result_json = ?2,
                updated_at = ?1
            WHERE id = ?3
            ",
            params![completed_at, serde_json::to_string(result)?, attempt_id],
        )?;
        let shard_id = task
            .pointer("/metadata/job_shard_id")
            .and_then(Value::as_str);
        if let Some(shard_id) = shard_id {
            let node_id = task
                .pointer("/status/leased_by_node_id")
                .and_then(Value::as_str);
            self.conn.execute(
                "
                UPDATE job_shards
                SET status = 'done',
                    node_id = ?1,
                    completed_at = ?2,
                    result_json = ?3,
                    updated_at = ?2
                WHERE id = ?4
                ",
                params![
                    node_id,
                    completed_at,
                    serde_json::to_string(result)?,
                    shard_id
                ],
            )?;
            let total = self.count_job_shards(job_id)?;
            let done = self.count_job_shards_by_status(job_id, "done")?;
            self.audit(
                "job.shard.completed",
                "job-runtime",
                Some(job_id),
                "Job Shard 执行完成",
                json!({ "job_id": job_id, "shard_id": shard_id, "attempt_id": attempt_id, "task_id": task_id, "done": done, "total": total }),
            )?;
            if total > 0 && done == total {
                self.complete_sharded_job_with_reduce(job_id, completed_at)?;
                self.audit(
                    "job.completed",
                    "job-runtime",
                    Some(job_id),
                    "Sharded Job 全部分片完成",
                    json!({ "job_id": job_id, "shard_count": total }),
                )?;
            } else {
                self.release_job_shards(job_id, "shard_released")?;
            }
        } else {
            self.conn.execute(
                "
                UPDATE jobs
                SET status = 'done',
                    completed_at = ?1,
                    result_json = ?2,
                    updated_at = ?1
                WHERE id = ?3
                ",
                params![completed_at, serde_json::to_string(result)?, job_id],
            )?;
            self.audit(
                "job.completed",
                "job-runtime",
                Some(job_id),
                "Job 执行完成",
                json!({ "job_id": job_id, "attempt_id": attempt_id, "task_id": task_id, "result": result }),
            )?;
        }
        Ok(())
    }

    fn mark_job_attempt_running(
        &self,
        task_id: &str,
        node_id: &str,
        started_at: &str,
    ) -> anyhow::Result<()> {
        let Some(task) = self.get_task(task_id)? else {
            return Ok(());
        };
        let Some(attempt_id) = task
            .pointer("/metadata/job_attempt_id")
            .and_then(Value::as_str)
        else {
            return Ok(());
        };
        self.conn.execute(
            "
            UPDATE job_attempts
            SET status = 'running',
                node_id = ?1,
                started_at = COALESCE(started_at, ?2),
                updated_at = ?2
            WHERE id = ?3
            ",
            params![node_id, started_at, attempt_id],
        )?;
        if let Some(shard_id) = task
            .pointer("/metadata/job_shard_id")
            .and_then(Value::as_str)
        {
            self.conn.execute(
                "
                UPDATE job_shards
                SET status = 'running',
                    node_id = ?1,
                    updated_at = ?2
                WHERE id = ?3
                ",
                params![node_id, started_at, shard_id],
            )?;
        }
        Ok(())
    }

    fn reschedule_job_task_failure(
        &self,
        task_id: &str,
        error: &Value,
        failed_at: &str,
    ) -> anyhow::Result<()> {
        let Some(task) = self.get_task(task_id)? else {
            return Ok(());
        };
        let Some(job_id) = task
            .pointer("/metadata/job_id")
            .and_then(Value::as_str)
            .map(ToString::to_string)
        else {
            return Ok(());
        };
        let Some(attempt_id) = task
            .pointer("/metadata/job_attempt_id")
            .and_then(Value::as_str)
            .map(ToString::to_string)
        else {
            return Ok(());
        };
        self.conn.execute(
            "
            UPDATE job_attempts
            SET status = 'failed',
                completed_at = ?1,
                error_json = ?2,
                updated_at = ?1
            WHERE id = ?3
            ",
            params![failed_at, serde_json::to_string(error)?, attempt_id],
        )?;
        let shard_id = task
            .pointer("/metadata/job_shard_id")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        if let Some(shard_id) = shard_id.as_deref() {
            self.conn.execute(
                "
                UPDATE job_shards
                SET status = 'failed',
                    completed_at = ?1,
                    error_json = ?2,
                    updated_at = ?1
                WHERE id = ?3
                ",
                params![failed_at, serde_json::to_string(error)?, shard_id],
            )?;
        }
        let Some(job) = self.get_job(&job_id)? else {
            return Ok(());
        };
        let attempts = self.count_job_attempts(&job_id, shard_id.as_deref())?;
        let decision = retry_reschedule_decision(&job, "process_failed", attempts, Some(error));
        if decision.get("should_reschedule").and_then(Value::as_bool) == Some(true) {
            self.create_job_attempt(&job_id, shard_id.as_deref(), "task_failed")?;
            self.audit(
                "job.attempt.retry_scheduled",
                "job-runtime",
                Some(&job_id),
                "Job Attempt 失败后已按策略重试",
                json!({
                    "job_id": job_id,
                    "attempt_id": attempt_id,
                    "task_id": task_id,
                    "retry_decision": decision,
                    "error": error
                }),
            )?;
        } else {
            if let Some(shard_id) = shard_id.as_deref() {
                self.fail_job_shard(&job_id, shard_id, error.clone())?;
            } else {
                self.fail_job(&job_id, error.clone())?;
            }
            self.audit(
                "job.attempt.retry_stopped",
                "job-runtime",
                Some(&job_id),
                "Job Attempt 失败后未重试",
                json!({
                    "job_id": job_id,
                    "attempt_id": attempt_id,
                    "task_id": task_id,
                    "retry_decision": decision,
                    "error": error
                }),
            )?;
        }
        Ok(())
    }

    fn job_attempt_shard_id(&self, attempt_id: &str) -> anyhow::Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT shard_id FROM job_attempts WHERE project_id = ?1 AND id = ?2",
                params![PROJECT_ID, attempt_id],
                |row| row.get(0),
            )
            .optional()
            .map(|value| value.flatten())
            .map_err(Into::into)
    }

    fn fail_job_shard(&self, job_id: &str, shard_id: &str, error: Value) -> anyhow::Result<()> {
        let now = now();
        self.conn.execute(
            "
            UPDATE job_shards
            SET status = 'failed',
                error_json = ?1,
                completed_at = COALESCE(completed_at, ?2),
                updated_at = ?2
            WHERE id = ?3
            ",
            params![serde_json::to_string(&error)?, now, shard_id],
        )?;
        self.fail_job(
            job_id,
            json!({
                "code": "job_shard_failed",
                "message": "Job shard exhausted retry attempts",
                "shard_id": shard_id,
                "cause": error
            }),
        )?;
        Ok(())
    }

    fn fail_job(&self, job_id: &str, error: Value) -> anyhow::Result<()> {
        let now = now();
        self.conn.execute(
            "
            UPDATE jobs
            SET status = 'failed',
                error_json = ?1,
                completed_at = COALESCE(completed_at, ?2),
                updated_at = ?2
            WHERE id = ?3
            ",
            params![serde_json::to_string(&error)?, now, job_id],
        )?;
        self.audit(
            "job.failed",
            "job-runtime",
            Some(job_id),
            "Job 执行失败",
            json!({ "job_id": job_id, "error": error }),
        )?;
        Ok(())
    }

    fn list_webhooks(&self, limit: u16) -> anyhow::Result<Vec<Value>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT * FROM webhook_subscriptions
            WHERE project_id = ?1
            ORDER BY updated_at DESC
            LIMIT ?2
            ",
        )?;
        let rows = stmt.query_map(params![PROJECT_ID, limit.min(500)], webhook_row)?;
        collect_values(rows)
    }

    fn create_webhook(&self, data: Value) -> anyhow::Result<Value> {
        let id = string_or(&data, "id", &new_id("webhook"));
        let now = now();
        self.conn.execute(
            "
            INSERT INTO webhook_subscriptions (
                id, project_id, name, url, events_json, secret, enabled,
                created_by, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)
            ",
            params![
                id,
                PROJECT_ID,
                required_string(&data, "name")?,
                required_string(&data, "url")?,
                serde_json::to_string(&array_field(&data, "events"))?,
                optional_string(&data, "secret"),
                if bool_or(&data, "enabled", true) {
                    1
                } else {
                    0
                },
                string_or(&data, "created_by", "architect-agent"),
                now,
            ],
        )?;
        self.audit(
            "webhook.created",
            &string_or(&data, "created_by", "architect-agent"),
            Some(&id),
            "Webhook 订阅已创建",
            data,
        )?;
        self.get_webhook(&id)?
            .ok_or_else(|| anyhow::anyhow!("webhook not found"))
    }

    fn get_webhook(&self, id: &str) -> anyhow::Result<Option<Value>> {
        self.conn
            .query_row(
                "SELECT * FROM webhook_subscriptions WHERE project_id = ?1 AND id = ?2",
                params![PROJECT_ID, id],
                webhook_row,
            )
            .optional()
            .map_err(Into::into)
    }

    fn get_webhook_record(&self, id: &str) -> anyhow::Result<Option<WebhookRecord>> {
        self.conn
            .query_row(
                "SELECT url, secret FROM webhook_subscriptions WHERE project_id = ?1 AND id = ?2",
                params![PROJECT_ID, id],
                |row| {
                    Ok(WebhookRecord {
                        url: row.get("url")?,
                        secret: row.get("secret")?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    fn delete_webhook(&self, id: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE webhook_subscriptions SET enabled = 0, updated_at = ?1 WHERE project_id = ?2 AND id = ?3",
            params![now(), PROJECT_ID, id],
        )?;
        self.audit(
            "webhook.disabled",
            "architect-agent",
            Some(id),
            "Webhook 订阅已停用",
            json!({ "id": id }),
        )?;
        Ok(())
    }

    fn list_webhook_deliveries(&self, limit: u16) -> anyhow::Result<Vec<Value>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT * FROM webhook_deliveries
            WHERE project_id = ?1
            ORDER BY created_at DESC
            LIMIT ?2
            ",
        )?;
        let rows = stmt.query_map(params![PROJECT_ID, limit.min(1000)], webhook_delivery_row)?;
        collect_values(rows)
    }

    fn list_webhook_deliveries_for_subject(
        &self,
        subject_id: &str,
        limit: u16,
    ) -> anyhow::Result<Vec<Value>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT * FROM webhook_deliveries
            WHERE project_id = ?1 AND subject_id = ?2
            ORDER BY created_at DESC
            LIMIT ?3
            ",
        )?;
        let rows = stmt.query_map(
            params![PROJECT_ID, subject_id, limit.min(1000)],
            webhook_delivery_row,
        )?;
        collect_values(rows)
    }

    fn enqueue_webhook_deliveries(
        &self,
        event_type: &str,
        subject_id: Option<&str>,
        payload: &Value,
    ) -> anyhow::Result<()> {
        let mut stmt = self.conn.prepare(
            "
            SELECT * FROM webhook_subscriptions
            WHERE project_id = ?1 AND enabled = 1
            ",
        )?;
        let rows = stmt.query_map(params![PROJECT_ID], webhook_row)?;
        let hooks = collect_values(rows)?;
        for hook in hooks {
            let events = hook
                .pointer("/spec/events")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect::<Vec<_>>();
            if !events.is_empty()
                && !events
                    .iter()
                    .any(|event| event == "*" || event == event_type)
            {
                continue;
            }
            let webhook_id = hook
                .pointer("/metadata/id")
                .and_then(Value::as_str)
                .unwrap_or("");
            let webhook = self
                .get_webhook_record(webhook_id)?
                .ok_or_else(|| anyhow::anyhow!("webhook not found"))?;
            let url = webhook.url.as_str();
            let secret = webhook.secret.as_deref();
            let delivery_id = new_id("whdel");
            let created_at = now();
            let delivery_payload = json!({
                "api_version": API_VERSION,
                "kind": "WebhookEvent",
                "delivery_id": delivery_id,
                "event_type": event_type,
                "subject_id": subject_id,
                "created_at": created_at,
                "payload": payload
            });
            let delivery = deliver_webhook(url, &delivery_payload, secret);
            self.conn.execute(
                "
                INSERT INTO webhook_deliveries (
                    id, project_id, webhook_id, event_type, subject_id, status,
                    status_code, error, payload_json, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                ",
                params![
                    delivery_id,
                    PROJECT_ID,
                    webhook_id,
                    event_type,
                    subject_id,
                    if delivery.ok { "delivered" } else { "failed" },
                    delivery.status_code,
                    delivery.error,
                    serde_json::to_string(&delivery_payload)?,
                    created_at,
                ],
            )?;
        }
        Ok(())
    }

    fn update_tool_probe_from_task(
        &self,
        task_id: &str,
        success: bool,
        result: Option<&Value>,
        error: Option<&Value>,
        now: &str,
    ) -> anyhow::Result<()> {
        let Some(task) = self.get_task(task_id)? else {
            return Ok(());
        };
        let labels = task
            .pointer("/spec/labels")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| item.as_str().map(ToString::to_string))
            .collect::<Vec<_>>();
        let Some(tool_id) = labels
            .iter()
            .find_map(|label| label.strip_prefix("probe:").map(ToString::to_string))
        else {
            return Ok(());
        };
        let Some(node_id) = task
            .pointer("/status/leased_by_node_id")
            .and_then(Value::as_str)
            .map(ToString::to_string)
        else {
            return Ok(());
        };
        let status = if success { "verified" } else { "failed" };
        let expires_at = if success {
            Some(
                (Utc::now() + chrono::Duration::hours(24))
                    .to_rfc3339_opts(chrono::SecondsFormat::Micros, true),
            )
        } else {
            None
        };
        self.conn.execute(
            "
            UPDATE tool_probes
            SET status = ?1,
                support_basis = 'runtime_probe',
                completed_at = ?2,
                expires_at = ?3,
                result_json = ?4,
                error_json = ?5,
                updated_at = ?2
            WHERE project_id = ?6 AND tool_id = ?7 AND node_id = ?8
            ",
            params![
                status,
                now,
                expires_at,
                serde_json::to_string(result.unwrap_or(&Value::Null))?,
                serde_json::to_string(error.unwrap_or(&Value::Null))?,
                PROJECT_ID,
                tool_id,
                node_id,
            ],
        )?;
        self.update_node_tool_probe_status(
            &node_id,
            &tool_id,
            Some(task_id),
            status,
            result.cloned(),
            error.cloned(),
        )?;
        self.audit(
            if success {
                "tool.probe.verified"
            } else {
                "tool.probe.failed"
            },
            "tool-probe-engine",
            Some(task_id),
            if success {
                "工具能力验证通过"
            } else {
                "工具能力验证失败"
            },
            json!({
                "tool_id": tool_id,
                "node_id": node_id,
                "task_id": task_id,
                "status": status,
                "result": result.cloned(),
                "error": error.cloned()
            }),
        )?;
        Ok(())
    }

    fn mark_workflow_task_running(&self, task_id: &str, now: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "
            UPDATE workflow_runs
            SET status = 'running', updated_at = ?1, started_at = COALESCE(started_at, ?1)
            WHERE task_id = ?2
            ",
            params![now, task_id],
        )?;
        Ok(())
    }

    fn mark_workflow_task_done(
        &self,
        task_id: &str,
        result: &Value,
        now: &str,
    ) -> anyhow::Result<()> {
        let workflow_id = self.workflow_id_for_task(task_id)?;
        let Some(workflow_id) = workflow_id else {
            return Ok(());
        };
        self.conn.execute(
            "
            UPDATE workflow_runs
            SET status = 'done', updated_at = ?1, completed_at = ?1, result_json = ?2, error_json = NULL
            WHERE task_id = ?3
            ",
            params![now, serde_json::to_string(result)?, task_id],
        )?;
        self.audit(
            "workflow.node.completed",
            "workflow-engine",
            Some(&workflow_id),
            "工作流节点执行完成",
            json!({ "workflow_id": workflow_id, "task_id": task_id }),
        )?;
        self.release_ready_workflow_nodes(&workflow_id)?;
        self.refresh_workflow_state(&workflow_id)?;
        Ok(())
    }

    fn mark_workflow_task_failed(
        &self,
        task_id: &str,
        error: &Value,
        now: &str,
    ) -> anyhow::Result<()> {
        let workflow_id = self.workflow_id_for_task(task_id)?;
        let Some(workflow_id) = workflow_id else {
            return Ok(());
        };
        let workflow_node_id = self
            .conn
            .query_row(
                "SELECT workflow_node_id FROM agent_tasks WHERE id = ?1",
                params![task_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();
        let node_policy = workflow_node_id
            .as_deref()
            .map(|node_id| self.workflow_node_definition(&workflow_id, node_id))
            .transpose()?
            .flatten();
        if node_policy
            .as_ref()
            .map(|node| node.optional || node.on_failure == "continue")
            .unwrap_or(false)
        {
            self.conn.execute(
                "
                UPDATE workflow_runs
                SET status = 'skipped', updated_at = ?1, completed_at = ?1, error_json = ?2
                WHERE task_id = ?3
                ",
                params![now, serde_json::to_string(error)?, task_id],
            )?;
            self.audit(
                "workflow.node.skipped",
                "workflow-engine",
                Some(&workflow_id),
                "工作流节点失败但按策略跳过",
                json!({
                    "workflow_id": workflow_id,
                    "workflow_node_id": workflow_node_id,
                    "task_id": task_id,
                    "on_failure": node_policy.as_ref().map(|node| node.on_failure.as_str()).unwrap_or("continue"),
                    "optional": node_policy.as_ref().map(|node| node.optional).unwrap_or(false),
                    "error": error
                }),
            )?;
            self.release_ready_workflow_nodes(&workflow_id)?;
            self.refresh_workflow_state(&workflow_id)?;
            return Ok(());
        }
        self.conn.execute(
            "
            UPDATE workflow_runs
            SET status = 'failed', updated_at = ?1, completed_at = ?1, error_json = ?2
            WHERE task_id = ?3
            ",
            params![now, serde_json::to_string(error)?, task_id],
        )?;
        self.conn.execute(
            "
            UPDATE workflow_runs
            SET status = 'cancelled', updated_at = ?1, completed_at = ?1
            WHERE workflow_id = ?2 AND status IN ('pending', 'ready')
            ",
            params![now, workflow_id],
        )?;
        self.audit(
            "workflow.failed",
            "workflow-engine",
            Some(&workflow_id),
            "工作流节点失败，工作流已停止推进",
            json!({ "workflow_id": workflow_id, "task_id": task_id, "error": error }),
        )?;
        self.refresh_workflow_state(&workflow_id)?;
        Ok(())
    }

    fn workflow_id_for_task(&self, task_id: &str) -> anyhow::Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT workflow_id FROM agent_tasks WHERE id = ?1",
                params![task_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map(|value| value.flatten())
            .map_err(Into::into)
    }

    fn workflow_node_definition(
        &self,
        workflow_id: &str,
        workflow_node_id: &str,
    ) -> anyhow::Result<Option<WorkflowNode>> {
        let Some(workflow) = self.get_workflow(workflow_id)? else {
            return Ok(None);
        };
        let nodes = parse_workflow_nodes(
            workflow
                .pointer("/spec/nodes")
                .ok_or_else(|| anyhow::anyhow!("workflow nodes missing"))?,
        )?;
        Ok(nodes.into_iter().find(|node| node.id == workflow_node_id))
    }

    fn bump_node_result(&self, node_id: &str, success: bool) -> anyhow::Result<()> {
        let column = if success {
            "success_count"
        } else {
            "failure_count"
        };
        self.conn.execute(
            &format!("UPDATE nodes SET {column} = {column} + 1 WHERE id = ?1"),
            params![node_id],
        )?;
        Ok(())
    }

    fn security_policy(&self) -> anyhow::Result<Value> {
        let mut policy = self
            .conn
            .query_row(
                "SELECT policy_json FROM security_policies WHERE project_id = ?1",
                params![PROJECT_ID],
                |row| {
                    let raw: String = row.get(0)?;
                    Ok(serde_json::from_str(&raw).unwrap_or_else(|_| default_security_policy()))
                },
            )
            .optional()
            .map(|value| value.unwrap_or_else(default_security_policy))
            .map_err(anyhow::Error::from)?;
        merge_json_defaults(&mut policy, default_security_policy());
        Ok(policy)
    }

    fn scheduler_config(&self) -> anyhow::Result<Value> {
        self.conn
            .query_row(
                "SELECT config_json FROM scheduler_configs WHERE project_id = ?1",
                params![PROJECT_ID],
                |row| {
                    let raw: String = row.get(0)?;
                    Ok(serde_json::from_str(&raw).unwrap_or_else(|_| default_scheduler_config()))
                },
            )
            .optional()
            .map(|value| value.unwrap_or_else(default_scheduler_config))
            .map_err(Into::into)
    }

    fn update_scheduler_config(&self, data: Value) -> anyhow::Result<Value> {
        let mut config = self.scheduler_config()?;
        merge_json_object(&mut config, data.clone());
        let now = now();
        self.conn.execute(
            "
            INSERT INTO scheduler_configs (project_id, config_json, updated_at)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(project_id) DO UPDATE SET
                config_json = excluded.config_json,
                updated_at = excluded.updated_at
            ",
            params![PROJECT_ID, serde_json::to_string(&config)?, now],
        )?;
        self.audit(
            "scheduler.config.changed",
            "architect-agent",
            Some(PROJECT_ID),
            "调度策略配置已更新",
            json!({ "input": data, "config": config.clone() }),
        )?;
        Ok(config)
    }

    fn diagnostics(&self) -> anyhow::Result<Value> {
        let nodes = self.list_nodes()?;
        let assigned = self.count_tasks_by_state("assigned")?;
        let running = self.count_tasks_by_state("in_progress")?;
        let failed = self.count_tasks_by_state("failed")?;
        let done = self.count_tasks_by_state("done")?;
        let expired_leases = self.count_expired_leases()?;
        let recent_audit = self.list_audit_events(80)?;
        let recent_failures = self.list_recent_task_failures(20)?;
        let online_nodes = nodes
            .iter()
            .filter(|node| node.pointer("/status/state").and_then(Value::as_str) == Some("online"))
            .count();
        let unknown_nodes = nodes
            .iter()
            .filter(|node| node.pointer("/status/state").and_then(Value::as_str) == Some("unknown"))
            .count();
        let offline_nodes = nodes
            .iter()
            .filter(|node| node.pointer("/status/state").and_then(Value::as_str) == Some("offline"))
            .count();

        Ok(json!({
            "generated_at": now(),
            "hub": {
                "service": "agentgrid-hub",
                "runtime": "rust",
                "high_load_score_limit": HIGH_LOAD_SCORE_LIMIT
            },
            "nodes": {
                "total": nodes.len(),
                "online": online_nodes,
                "unknown": unknown_nodes,
                "offline": offline_nodes,
                "items": nodes
            },
            "tasks": {
                "assigned": assigned,
                "in_progress": running,
                "done": done,
                "failed": failed,
                "expired_leases": expired_leases,
                "recent_failures": recent_failures
            },
            "logs": {
                "recent_audit": recent_audit
            }
        }))
    }

    fn job_reliability_status(&self) -> anyhow::Result<Value> {
        let queued_jobs = self.count_jobs_by_state("queued")?;
        let running_jobs = self.count_jobs_by_state("running")?;
        let done_jobs = self.count_jobs_by_state("done")?;
        let failed_jobs = self.count_jobs_by_state("failed")?;
        let lost_attempts = self.count_job_attempts_by_state("lost")?;
        let failed_attempts = self.count_job_attempts_by_state("failed")?;
        let checkpoints = self.count_job_checkpoints()?;
        let expired_leases = self.count_expired_leases()?;
        Ok(json!({
            "api_version": "agentgrid.reliability/v1",
            "kind": "JobReliabilityStatus",
            "generated_at": now(),
            "guarantee": {
                "delivery": "at_least_once",
                "exactly_once_requires": ["idempotent tool behavior", "stable idempotency key", "checkpoint-aware executor"],
                "node_lost_reschedule": true,
                "lease_recovery_loop_seconds": 15
            },
            "contract": job_reliability_standard_contract(),
            "retry_reschedule_contract": retry_reschedule_standard_contract(),
            "runtime": {
                "lease": {
                    "default_seconds": 120,
                    "max_seconds": 600,
                    "expired_leases": expired_leases
                },
                "jobs": {
                    "queued": queued_jobs,
                    "running": running_jobs,
                    "done": done_jobs,
                    "failed": failed_jobs
                },
                "attempts": {
                    "lost": lost_attempts,
                    "failed": failed_attempts
                },
                "checkpoints": {
                    "total": checkpoints
                }
            },
            "endpoints": {
                "plan_job": "POST /api/jobs/plan",
                "submit_job": "POST /api/jobs",
                "get_job": "GET /api/jobs/{id}",
                "job_execution": "GET /api/jobs/{id}/execution",
                "checkpoint": "POST /api/jobs/{id}/checkpoints",
                "recovery_scan": "POST /api/jobs/recovery/scan",
                "event_ingress": "POST /api/events/ingress",
                "worker_lease": "POST /api/worker/lease"
            }
        }))
    }

    fn job_recovery_scan(&self, trigger: &str) -> anyhow::Result<Value> {
        let started_at = now();
        let expired_before = self.count_expired_leases()?;
        let running_before = self.count_jobs_by_state("running")?;
        let queued_before = self.count_jobs_by_state("queued")?;
        let recovered_items = self.requeue_lost_job_attempts()?;
        let rescheduled = recovered_items
            .iter()
            .filter(|item| item.get("outcome").and_then(Value::as_str) == Some("rescheduled"))
            .count();
        let stopped = recovered_items
            .iter()
            .filter(|item| item.get("outcome").and_then(Value::as_str) == Some("failed"))
            .count();
        let expired_after = self.count_expired_leases()?;
        let running_after = self.count_jobs_by_state("running")?;
        let queued_after = self.count_jobs_by_state("queued")?;
        let completed_at = now();
        let item = json!({
            "api_version": "agentgrid.recovery/v1",
            "kind": "JobRecoveryScan",
            "trigger": trigger,
            "started_at": started_at,
            "completed_at": completed_at,
            "status": "completed",
            "inputs": {
                "expired_leases_before": expired_before,
                "running_jobs_before": running_before,
                "queued_jobs_before": queued_before
            },
            "outputs": {
                "rescheduled_attempts": rescheduled,
                "stopped_attempts": stopped,
                "recovered_items": recovered_items,
                "expired_leases_after": expired_after,
                "running_jobs_after": running_after,
                "queued_jobs_after": queued_after
            },
            "recovery_loop_seconds": 15,
            "contract": retry_reschedule_standard_contract()
        });
        if trigger != "loop" || rescheduled > 0 {
            self.audit(
                "job.recovery.scanned",
                "job-runtime",
                Some(PROJECT_ID),
                "Job recovery scan completed",
                item.clone(),
            )?;
        }
        Ok(item)
    }

    fn count_tasks_by_state(&self, state: &str) -> anyhow::Result<i64> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM agent_tasks WHERE project_id = ?1 AND status = ?2",
                params![PROJECT_ID, state],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    fn count_jobs_by_state(&self, state: &str) -> anyhow::Result<i64> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM jobs WHERE project_id = ?1 AND status = ?2",
                params![PROJECT_ID, state],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    fn count_job_attempts_by_state(&self, state: &str) -> anyhow::Result<i64> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM job_attempts WHERE project_id = ?1 AND status = ?2",
                params![PROJECT_ID, state],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    fn count_job_checkpoints(&self) -> anyhow::Result<i64> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM job_checkpoints WHERE project_id = ?1",
                params![PROJECT_ID],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    fn count_expired_leases(&self) -> anyhow::Result<i64> {
        self.conn
            .query_row(
                "
                SELECT COUNT(*) FROM agent_tasks
                WHERE project_id = ?1
                  AND status = 'in_progress'
                  AND lease_expires_at IS NOT NULL
                  AND lease_expires_at < ?2
                ",
                params![PROJECT_ID, now()],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    fn list_recent_task_failures(&self, limit: u16) -> anyhow::Result<Vec<Value>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT * FROM agent_tasks
            WHERE project_id = ?1
              AND (status = 'failed' OR error_json IS NOT NULL)
            ORDER BY updated_at DESC
            LIMIT ?2
            ",
        )?;
        let rows = stmt.query_map(params![PROJECT_ID, limit], task_row)?;
        collect_values(rows)
    }

    fn list_audit_events(&self, limit: u16) -> anyhow::Result<Vec<Value>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM audit_events WHERE project_id = ?1 ORDER BY created_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![PROJECT_ID, limit], audit_row)?;
        collect_values(rows)
    }

    fn list_audit_events_for_subject(
        &self,
        subject_id: &str,
        limit: u16,
    ) -> anyhow::Result<Vec<Value>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT * FROM audit_events
            WHERE project_id = ?1 AND subject_id = ?2
            ORDER BY created_at ASC
            LIMIT ?3
            ",
        )?;
        let rows = stmt.query_map(params![PROJECT_ID, subject_id, limit], audit_row)?;
        collect_values(rows)
    }

    fn task_event_snapshot(&self, id: &str) -> anyhow::Result<Value> {
        let task = self
            .get_task(id)?
            .ok_or_else(|| anyhow::anyhow!("task not found"))?;
        let events = self.list_audit_events_for_subject(id, 200)?;
        let logs = self.list_task_logs(id, 1000)?;
        let artifacts = self.list_artifacts_for_task(id)?;
        let result = task
            .pointer("/status/result")
            .cloned()
            .unwrap_or(Value::Null);
        let error = task
            .pointer("/status/error")
            .cloned()
            .unwrap_or(Value::Null);
        Ok(json!({
            "ok": true,
            "task_id": id,
            "time": now(),
            "state": task.pointer("/status/state").and_then(Value::as_str).unwrap_or("unknown"),
            "progress": task.pointer("/status/progress").and_then(Value::as_i64).unwrap_or(0),
            "leased_by_node_id": task.pointer("/status/leased_by_node_id").cloned().unwrap_or(Value::Null),
            "started_at": task.pointer("/status/started_at").cloned().unwrap_or(Value::Null),
            "completed_at": task.pointer("/status/completed_at").cloned().unwrap_or(Value::Null),
            "stdout": extract_result_text(&result, "stdout"),
            "stderr": extract_result_text(&result, "stderr").or_else(|| extract_result_text(&error, "message")),
            "result": result,
            "error": error,
            "logs": logs,
            "artifacts": artifacts,
            "events": events
        }))
    }

    fn task_execution_record(&self, id: &str) -> anyhow::Result<Value> {
        let task = self
            .get_task(id)?
            .ok_or_else(|| anyhow::anyhow!("task not found"))?;
        let snapshot = self.task_event_snapshot(id)?;
        let schedule_preview = self.task_schedule_preview(id).ok();
        let webhook_deliveries = self.list_webhook_deliveries_for_subject(id, 200)?;
        let input_payloads = task
            .pointer("/spec/inputs")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|item| {
                item.as_str()
                    .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
                    .unwrap_or(item)
            })
            .collect::<Vec<_>>();
        let result = task
            .pointer("/status/result")
            .cloned()
            .unwrap_or(Value::Null);
        let error = task
            .pointer("/status/error")
            .cloned()
            .unwrap_or(Value::Null);
        Ok(json!({
            "api_version": API_VERSION,
            "kind": "ExecutionRecord",
            "record_type": "task",
            "generated_at": now(),
            "task_id": id,
            "summary": {
                "title": task.pointer("/spec/title").and_then(Value::as_str),
                "state": task.pointer("/status/state").and_then(Value::as_str),
                "progress": task.pointer("/status/progress").and_then(Value::as_i64),
                "created_by": task.pointer("/metadata/created_by").and_then(Value::as_str),
                "owner": task.pointer("/spec/owner").and_then(Value::as_str),
                "priority": task.pointer("/spec/priority").and_then(Value::as_str),
                "leased_by_node_id": task.pointer("/status/leased_by_node_id").and_then(Value::as_str),
                "attempts": task.pointer("/status/attempts").and_then(Value::as_i64),
                "started_at": task.pointer("/status/started_at").cloned().unwrap_or(Value::Null),
                "completed_at": task.pointer("/status/completed_at").cloned().unwrap_or(Value::Null)
            },
            "input": {
                "raw": task.pointer("/spec/inputs").cloned().unwrap_or_else(|| json!([])),
                "payloads": input_payloads,
                "labels": task.pointer("/spec/labels").cloned().unwrap_or_else(|| json!([])),
                "acceptance_criteria": task.pointer("/spec/acceptance_criteria").cloned().unwrap_or_else(|| json!([])),
                "verify": task.pointer("/spec/verify").cloned().unwrap_or(Value::Null)
            },
            "schedule": schedule_preview,
            "execution": {
                "result": result,
                "error": error,
                "verification": task.pointer("/status/result/verification").cloned().unwrap_or(Value::Null),
                "logs": snapshot.get("logs").cloned().unwrap_or_else(|| json!([])),
                "artifacts": snapshot.get("artifacts").cloned().unwrap_or_else(|| json!([]))
            },
            "notifications": {
                "webhook_deliveries": webhook_deliveries
            },
            "audit": snapshot.get("events").cloned().unwrap_or_else(|| json!([])),
            "raw": {
                "task": task,
                "snapshot": snapshot
            }
        }))
    }

    fn workflow_execution_record(&self, id: &str) -> anyhow::Result<Value> {
        let workflow = self
            .get_workflow_detail(id)?
            .ok_or_else(|| anyhow::anyhow!("workflow not found"))?;
        let audit = self.list_audit_events_for_subject(id, 500)?;
        let deliveries = self.list_webhook_deliveries_for_subject(id, 200)?;
        let runs = workflow
            .pointer("/spec/runs")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut task_records = Vec::new();
        for run in &runs {
            if let Some(task_id) = run.pointer("/metadata/task_id").and_then(Value::as_str) {
                if let Ok(record) = self.task_execution_record(task_id) {
                    task_records.push(record);
                }
            }
        }
        Ok(json!({
            "api_version": API_VERSION,
            "kind": "ExecutionRecord",
            "record_type": "workflow",
            "generated_at": now(),
            "workflow_id": id,
            "summary": {
                "name": workflow.pointer("/spec/name").and_then(Value::as_str),
                "state": workflow.pointer("/status/state").and_then(Value::as_str),
                "progress": workflow.pointer("/status/progress").and_then(Value::as_i64),
                "created_by": workflow.pointer("/metadata/created_by").and_then(Value::as_str),
                "started_at": workflow.pointer("/status/started_at").cloned().unwrap_or(Value::Null),
                "completed_at": workflow.pointer("/status/completed_at").cloned().unwrap_or(Value::Null),
                "done_count": workflow.pointer("/status/done_count").and_then(Value::as_i64),
                "skipped_count": workflow.pointer("/status/skipped_count").and_then(Value::as_i64),
                "failed_count": workflow.pointer("/status/failed_count").and_then(Value::as_i64)
            },
            "definition": {
                "inputs": workflow.pointer("/spec/inputs").cloned().unwrap_or_else(|| json!({})),
                "nodes": workflow.pointer("/spec/nodes").cloned().unwrap_or_else(|| json!([]))
            },
            "runs": runs,
            "tasks": task_records,
            "result": workflow.pointer("/status/result").cloned().unwrap_or(Value::Null),
            "error": workflow.pointer("/status/error").cloned().unwrap_or(Value::Null),
            "notifications": {
                "webhook_deliveries": deliveries
            },
            "audit": audit,
            "raw": {
                "workflow": workflow
            }
        }))
    }

    fn append_task_log(&self, id: &str, data: Value) -> anyhow::Result<()> {
        let node_id = string_or(&data, "node_id", "worker");
        let stream = string_or(&data, "stream", "stdout");
        let line = string_or(&data, "line", "");
        let now = now();
        let sequence: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM task_logs WHERE task_id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        self.conn.execute(
            "
            INSERT INTO task_logs (id, project_id, task_id, node_id, stream, line, sequence, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ",
            params![
                new_id("log"),
                PROJECT_ID,
                id,
                node_id,
                stream,
                line,
                sequence,
                now
            ],
        )?;
        self.conn.execute(
            "UPDATE agent_tasks SET updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }

    fn list_task_logs(&self, id: &str, limit: u16) -> anyhow::Result<Vec<Value>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT * FROM task_logs
            WHERE project_id = ?1 AND task_id = ?2
            ORDER BY sequence ASC
            LIMIT ?3
            ",
        )?;
        let rows = stmt.query_map(params![PROJECT_ID, id, limit], task_log_row)?;
        collect_values(rows)
    }

    fn list_artifacts(&self, limit: u16) -> anyhow::Result<Vec<Value>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT * FROM artifacts
            WHERE project_id = ?1
            ORDER BY created_at DESC
            LIMIT ?2
            ",
        )?;
        let rows = stmt.query_map(params![PROJECT_ID, limit], artifact_row)?;
        collect_values(rows)
    }

    fn get_artifact(&self, id: &str) -> anyhow::Result<Option<Value>> {
        self.conn
            .query_row(
                "SELECT * FROM artifacts WHERE id = ?1",
                params![id],
                artifact_row,
            )
            .optional()
            .map_err(Into::into)
    }

    fn list_artifacts_for_task(&self, task_id: &str) -> anyhow::Result<Vec<Value>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT * FROM artifacts
            WHERE project_id = ?1 AND task_id = ?2
            ORDER BY created_at DESC
            ",
        )?;
        let rows = stmt.query_map(params![PROJECT_ID, task_id], artifact_row)?;
        collect_values(rows)
    }

    fn extract_artifacts_from_result(
        &self,
        task_id: &str,
        node_id: Option<&str>,
        result: &Value,
    ) -> anyhow::Result<()> {
        let tool_id = tool_id_for_task_id(self, task_id);
        let tool_id = tool_id.as_deref();
        match result.get("type").and_then(Value::as_str).unwrap_or("") {
            "file_result" => {
                if result.get("operation").and_then(Value::as_str) == Some("download") {
                    if let Some(content) = result.get("content").and_then(Value::as_str) {
                        let path = result
                            .get("path")
                            .and_then(Value::as_str)
                            .unwrap_or("download.bin");
                        self.insert_artifact(ArtifactInput {
                            task_id,
                            node_id,
                            name: file_name_from_path(path),
                            artifact_type: "file",
                            content_type: "application/octet-stream",
                            content_base64: Some(content),
                            source_path: Some(path),
                            size_bytes: result.get("bytes").and_then(Value::as_u64).unwrap_or(0),
                            tool_id,
                            metadata: json!({ "operation": "download", "path": path }),
                        })?;
                    }
                }
            }
            "command_result" | "git_result" | "docker_result" | "session_result" => {
                for stream in ["stdout", "stderr"] {
                    if let Some(text) = result.get(stream).and_then(Value::as_str) {
                        if !text.is_empty() {
                            self.insert_artifact(ArtifactInput {
                                task_id,
                                node_id,
                                name: &format!("{task_id}-{stream}.log"),
                                artifact_type: "log",
                                content_type: "text/plain; charset=utf-8",
                                content_base64: Some(
                                    &base64::engine::general_purpose::STANDARD.encode(text),
                                ),
                                source_path: None,
                                size_bytes: text.len() as u64,
                                tool_id,
                                metadata: json!({ "stream": stream, "result_type": result.get("type") }),
                            })?;
                        }
                    }
                }
            }
            "browser_result" => {
                if let Some(path) = result.get("screenshot_path").and_then(Value::as_str) {
                    self.insert_artifact(ArtifactInput {
                        task_id,
                        node_id,
                        name: file_name_from_path(path),
                        artifact_type: "screenshot",
                        content_type: "image/png",
                        content_base64: None,
                        source_path: Some(path),
                        size_bytes: 0,
                        tool_id,
                        metadata: json!({ "path": path, "note": "stored on worker node" }),
                    })?;
                }
                if let Some(text) = result.get("text").and_then(Value::as_str) {
                    if !text.is_empty() {
                        self.insert_artifact(ArtifactInput {
                            task_id,
                            node_id,
                            name: &format!("{task_id}-browser-text.txt"),
                            artifact_type: "browser_text",
                            content_type: "text/plain; charset=utf-8",
                            content_base64: Some(
                                &base64::engine::general_purpose::STANDARD.encode(text),
                            ),
                            source_path: None,
                            size_bytes: text.len() as u64,
                            tool_id,
                            metadata: json!({ "url": result.get("url") }),
                        })?;
                    }
                }
            }
            "desktop_result" => {
                if result.get("operation").and_then(Value::as_str) == Some("screenshot") {
                    let path = result
                        .get("path")
                        .and_then(Value::as_str)
                        .unwrap_or("desktop-screenshot.png");
                    let content = result.get("content_base64").and_then(Value::as_str);
                    self.insert_artifact(ArtifactInput {
                        task_id,
                        node_id,
                        name: file_name_from_path(path),
                        artifact_type: "screenshot",
                        content_type: "image/png",
                        content_base64: content,
                        source_path: Some(path),
                        size_bytes: result.get("bytes").and_then(Value::as_u64).unwrap_or(0),
                        tool_id,
                        metadata: json!({
                            "operation": "desktop.screenshot",
                            "path": path,
                            "width": result.get("width"),
                            "height": result.get("height"),
                            "stored_in_hub": content.is_some()
                        }),
                    })?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn insert_artifact(&self, input: ArtifactInput<'_>) -> anyhow::Result<()> {
        let metadata = artifact_v2_metadata(&input);
        self.conn.execute(
            "
            INSERT INTO artifacts (
                id, project_id, task_id, node_id, name, artifact_type, content_type,
                content_base64, source_path, size_bytes, metadata_json, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            ",
            params![
                new_id("artifact"),
                PROJECT_ID,
                input.task_id,
                input.node_id,
                input.name,
                input.artifact_type,
                input.content_type,
                input.content_base64,
                input.source_path,
                input.size_bytes as i64,
                serde_json::to_string(&metadata)?,
                now()
            ],
        )?;
        Ok(())
    }

    fn audit(
        &self,
        event_type: &str,
        actor: &str,
        subject_id: Option<&str>,
        summary: &str,
        payload: Value,
    ) -> anyhow::Result<()> {
        self.conn.execute(
            "
            INSERT INTO audit_events (
                id, project_id, event_type, actor, subject_id, summary, payload_json, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ",
            params![
                new_id("audit"),
                PROJECT_ID,
                event_type,
                actor,
                subject_id,
                summary,
                serde_json::to_string(&payload)?,
                now(),
            ],
        )?;
        Ok(())
    }
}

fn task_matches_capabilities(task: &Value, capabilities: &[String]) -> bool {
    let labels = task
        .pointer("/spec/labels")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let label_strings = labels.iter().filter_map(Value::as_str).collect::<Vec<_>>();
    if !label_strings.contains(&"compute") {
        return false;
    }
    if label_strings.contains(&"http_request") {
        return capabilities.iter().any(|capability| capability == "http");
    }
    if label_strings.contains(&"command") {
        return capabilities
            .iter()
            .any(|capability| capability == "command");
    }
    for (label, capability) in [
        ("file", "file"),
        ("git", "git"),
        ("docker", "docker"),
        ("browser", "browser"),
        ("desktop", "desktop"),
        ("session", "session"),
        ("agentmessage", "agentmessage"),
        ("plugin", "plugin"),
    ] {
        if label_strings.contains(&label) {
            return capabilities.iter().any(|item| item == capability);
        }
    }
    let Ok(payload) = parse_job_payload_from_task(task) else {
        return false;
    };
    let required = match payload {
        JobPayload::HttpRequest(_) => "http",
        JobPayload::Command(_) => "command",
        JobPayload::File(_) => "file",
        JobPayload::Git(_) => "git",
        JobPayload::Docker(_) => "docker",
        JobPayload::Browser(_) => "browser",
        JobPayload::Desktop(_) => "desktop",
        JobPayload::Session(_) => "session",
        JobPayload::AgentMessage(_) => "agentmessage",
        JobPayload::Plugin(_) | JobPayload::Custom { .. } => "plugin",
    };
    capabilities.iter().any(|item| item == required)
}

fn node_tool_catalog_item(tool_id: &str, items: Vec<Value>, nodes: &[Value]) -> Value {
    let first = items.first().cloned().unwrap_or_else(|| json!({}));
    let node_by_id = nodes
        .iter()
        .filter_map(|node| {
            node.pointer("/metadata/id")
                .and_then(Value::as_str)
                .map(|id| (id.to_string(), node))
        })
        .collect::<HashMap<_, _>>();
    let available_nodes = items
        .iter()
        .filter(|item| item.pointer("/status/state").and_then(Value::as_str) == Some("available"))
        .filter_map(|item| {
            let node_id = item.pointer("/metadata/node_id").and_then(Value::as_str)?;
            let node = node_by_id.get(node_id)?;
            if node.pointer("/status/state").and_then(Value::as_str) != Some("online") {
                return None;
            }
            Some(json!({
                "node_id": node_id,
                "name": node.pointer("/metadata/name").and_then(Value::as_str),
                "os": node.pointer("/spec/os").and_then(Value::as_str),
                "arch": node.pointer("/spec/arch").and_then(Value::as_str),
                "state": node.pointer("/status/state").and_then(Value::as_str),
                "version": item.pointer("/spec/version").and_then(Value::as_str),
                "executor": item.pointer("/spec/executor").and_then(Value::as_str),
                "confidence": item.pointer("/status/confidence").and_then(Value::as_str),
                "probe_state": item.pointer("/status/probe_state").and_then(Value::as_str),
                "last_probe_at": item.pointer("/status/last_probe_at").and_then(Value::as_str),
                "next_probe_at": item.pointer("/status/next_probe_at").and_then(Value::as_str),
                "probe_task_id": item.pointer("/status/probe_task_id").and_then(Value::as_str),
                "metadata": item.pointer("/spec/metadata").cloned().unwrap_or_else(|| json!({})),
                "constraints": item.pointer("/spec/constraints").cloned().unwrap_or_else(|| json!({}))
            }))
        })
        .collect::<Vec<_>>();
    let plugin_manifest = if first
        .pointer("/spec/executor")
        .and_then(Value::as_str)
        .map(|executor| executor.starts_with("plugin:"))
        .unwrap_or(false)
    {
        Some(plugin_manifest_from_node_tool(&first))
    } else {
        None
    };
    json!({
        "api_version": "agentgrid.runtime/v1",
        "kind": "NodeToolCatalogItem",
        "tool_id": tool_id,
        "name": first.pointer("/spec/name").and_then(Value::as_str).unwrap_or(tool_id),
        "version": first.pointer("/spec/version").and_then(Value::as_str).unwrap_or("0.1.0"),
        "executor": first.pointer("/spec/executor").and_then(Value::as_str).unwrap_or("plugin"),
        "plugin_id": plugin_manifest.as_ref().and_then(|manifest| manifest.get("plugin_id")).and_then(Value::as_str),
        "plugin_manifest": plugin_manifest,
        "metadata": first.pointer("/spec/metadata").cloned().unwrap_or_else(|| json!({})),
        "risk": first.pointer("/spec/metadata/risk").and_then(Value::as_str).unwrap_or("high"),
        "status": if available_nodes.is_empty() { "unavailable" } else { "available" },
        "confidence": first.pointer("/status/confidence").and_then(Value::as_str).unwrap_or("declared"),
        "input_schema": first.pointer("/spec/input_schema").cloned().unwrap_or_else(|| json!({})),
        "output_schema": first.pointer("/spec/output_schema").cloned().unwrap_or_else(|| json!({})),
        "constraints": first.pointer("/spec/constraints").cloned().unwrap_or_else(|| json!({})),
        "labels": first.pointer("/spec/labels").cloned().unwrap_or_else(|| json!(["compute", format!("tool:{tool_id}")])),
        "default_verify": first.pointer("/spec/default_verify").cloned().unwrap_or(Value::Null),
        "probe": first.pointer("/spec/probe").cloned().unwrap_or(Value::Null),
        "probe_state": aggregate_probe_state(&items),
        "node_count": available_nodes.len(),
        "nodes": available_nodes,
        "registrations": items
    })
}

fn dynamic_tool_contract_from_catalog(catalog: &Value) -> Value {
    let tool_id = catalog.get("tool_id").and_then(Value::as_str).unwrap_or("");
    let executor = catalog
        .get("executor")
        .and_then(Value::as_str)
        .unwrap_or("plugin");
    let labels = catalog
        .get("labels")
        .cloned()
        .unwrap_or_else(|| json!(["compute", "plugin", format!("tool:{tool_id}")]));
    let default_verify = catalog
        .get("default_verify")
        .cloned()
        .filter(|value| !value.is_null())
        .unwrap_or_else(|| json!({ "rules": [{ "path": "result.type", "op": "exists", "description": "动态工具必须回写结构化结果" }] }));
    enrich_tool_contract(json!({
        "id": tool_id,
        "name": catalog.get("name").and_then(Value::as_str).unwrap_or(tool_id),
        "summary": "节点动态注册工具。",
        "category": "node_tool",
        "payload_type": "dynamic_tool",
        "capability": "plugin",
        "labels": labels,
        "risk": "high",
        "requires_policy": true,
        "dynamic": true,
        "executor": executor,
        "input_schema": catalog.get("input_schema").cloned().unwrap_or_else(|| json!({})),
        "output_schema": catalog.get("output_schema").cloned().unwrap_or_else(|| json!({})),
        "constraints": catalog.get("constraints").cloned().unwrap_or_else(|| json!({})),
        "default_verify": default_verify,
        "standard_outputs": ["dynamic_tool_result", "duration_ms", "verification"],
        "examples": [{
            "type": tool_id,
            "tool_id": tool_id,
            "executor": executor,
            "input": {}
        }]
    }))
}

fn runtime_standard_document(store: &Store) -> anyhow::Result<Value> {
    let nodes = store.list_nodes()?;
    let tools = store
        .tool_registry_with_dynamic()?
        .into_iter()
        .map(|tool| store.enrich_tool_with_nodes(tool, &nodes))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let tool_contracts = tools
        .iter()
        .map(|tool| {
            tool.get("tool_contract")
                .cloned()
                .unwrap_or_else(|| tool.clone())
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "api_version": "agentgrid.runtime/v1",
        "kind": "AgentGridRuntimeStandard",
        "metadata": {
            "id": "agentgrid-runtime-standard-v1",
            "name": "AgentGrid Runtime Standard v1",
            "version": AGENTGRID_BUILD_VERSION,
            "generated_at": now(),
            "boundary": {
                "included": [
                    "AI workbench discovery",
                    "real machine and hardware station scheduling",
                    "desktop, device, browser, serial, flashing and local tool capability contracts",
                    "resource-aware scheduling",
                    "runbook templates",
                    "evidence collection",
                    "task state machine",
                    "structured result reports",
                    "execution records",
                    "audit events"
                ],
                "excluded": [
                    "natural language understanding",
                    "model reasoning",
                    "general RDP replacement",
                    "general CI/CD replacement",
                    "general configuration management replacement",
                    "authorization policy design"
                ]
            }
        },
        "entrypoints": {
            "manifest": "/api/agent-runtime/manifest",
            "submit_task": "/api/agent-runtime/tasks",
            "task_status": "/api/agent-runtime/tasks/{task_id}",
            "task_events": "/api/agent-runtime/tasks/{task_id}/events",
            "tool_contracts": "/api/runtime-standard/tool-contracts",
            "capabilities": "/api/runtime-standard/capabilities",
            "state_machine": "/api/runtime-standard/state-machine",
            "workflow_template": "/api/runtime-standard/workflow-template",
            "result_report": "/api/runtime-standard/result-report",
            "workbench": "/api/runtime-standard/workbench",
            "devices": "/api/runtime-standard/devices",
            "evidence": "/api/runtime-standard/evidence",
            "runbook": "/api/runtime-standard/runbook",
            "mobile_sdk": "/api/runtime-standard/mobile-sdk",
            "plugin_runtime": "/api/runtime-standard/plugin-runtime",
            "capability_graph": "/api/runtime-standard/capability-graph",
            "execution_contract": "/api/runtime-standard/execution-contract",
            "evidence_pipeline": "/api/runtime-standard/evidence-pipeline",
            "probe_engine": "/api/runtime-standard/probe-engine",
            "placement_engine": "/api/runtime-standard/placement-engine",
            "task_intent": "/api/runtime-standard/task-intent",
            "artifact_store": "/api/runtime-standard/artifact-store",
            "event_timeline": "/api/runtime-standard/event-timeline",
            "task_execution_record": "/api/execution-records/tasks/{task_id}",
            "workflow_execution_record": "/api/execution-records/workflows/{workflow_id}",
            "artifacts": "/api/artifacts",
            "artifact_download": "/api/artifacts/{artifact_id}/download"
        },
        "positioning": workbench_positioning_standard(),
        "tool_contracts": tool_contracts,
        "capability_registry": capability_standard(store)?,
        "workbench_standard": workbench_standard(store)?,
        "device_standard": device_standard(store)?,
        "evidence_standard": evidence_standard(store)?,
        "runbook_standard": runbook_standard(store)?,
        "mobile_sdk_standard": mobile_sdk_standard(),
        "plugin_runtime_standard": plugin_runtime_standard(store)?,
        "capability_graph_standard": capability_graph_standard(store)?,
        "execution_contract_standard": execution_contract_standard(store)?,
        "evidence_pipeline_standard": evidence_pipeline_standard(store)?,
        "probe_engine_standard": probe_engine_standard(store)?,
        "placement_engine_standard": placement_engine_standard(store)?,
        "task_intent_standard": task_intent_standard(),
        "artifact_store_standard": artifact_store_standard(store)?,
        "event_timeline_standard": event_timeline_standard(store)?,
        "capability_marketplace": capability_marketplace_standard(store)?,
        "job_reliability": job_reliability_standard_contract(),
        "task_state_machine": task_state_machine_standard(),
        "workflow_template": workflow_template_standard(store)?,
        "result_report": result_report_standard(store)?,
        "execution_record": execution_record_standard(),
        "client_rules": [
            "Do not send natural language as a task payload. Convert intent to structured JSON before calling AgentGrid.",
            "Read ToolContract input_schema before submitting a task.",
            "Use Runtime submit endpoint for AI clients and direct task endpoint for low-level operators.",
            "Use labels for routing requirements, not free text.",
            "Treat nodes as workbenches when they expose desktop, hardware, browser, serial, flashing, test, or local SDK capabilities.",
            "Choose a workbench by capability schema, verified probe status, OS, resources, and required evidence.",
            "Read schedule/audit/execution record when explaining a result.",
            "Mobile clients are console clients only. They inspect, submit, and observe; they do not execute Worker tasks.",
            "Use Capability Graph and Execution Contract before expanding new tools or plugins.",
            "Use Task Intent JSON for AI-generated requests; AgentGrid validates and schedules structured intent only."
        ]
    }))
}

fn workbench_positioning_standard() -> Value {
    json!({
        "one_sentence": "AgentGrid is the scheduling layer for AI to discover, call, verify, and audit real machines, desktop benches, hardware benches, devices, and local tools.",
        "primary_market": [
            "AI hardware test benches",
            "AI desktop operation benches",
            "AI worker capability marketplace"
        ],
        "anti_positioning": [
            "not a generic remote execution platform",
            "not an Ansible/Jenkins/RDP replacement",
            "not a natural-language automation layer"
        ],
        "killer_scenarios": [
            {
                "id": "ai.hardware_bench",
                "name": "AI 硬件测试工位",
                "flow": ["code build", "flash board", "read serial", "capture tool screenshot", "collect test report", "judge pass/fail"]
            },
            {
                "id": "ai.desktop_bench",
                "name": "AI 桌面工位",
                "flow": ["capture screen", "click/type/key", "run foreground tool", "collect screenshot/artifact", "audit operation chain"]
            },
            {
                "id": "ai.capability_marketplace",
                "name": "AI Worker 能力市场",
                "flow": ["node declares capability", "Hub normalizes schema", "Probe verifies capability", "AI calls by tool_id", "Hub records evidence"]
            }
        ]
    })
}

fn job_reliability_standard_contract() -> Value {
    json!({
        "api_version": "agentgrid.reliability/v1",
        "kind": "JobReliabilityStandard",
        "model": {
            "job": "User intent and reliability policy.",
            "attempt": "One concrete execution try represented by an AgentTask.",
            "lease": "Time-bounded task ownership granted by Hub to Worker.",
            "checkpoint": "Recoverable progress marker reported by Worker or client.",
            "journal": "Worker-side execution memory; Hub stores canonical attempts and checkpoints."
        },
        "guarantees": {
            "delivery": "at_least_once",
            "reschedule_on_node_lost": true,
            "reschedule_on_lease_expired": true,
            "exactly_once": "not guaranteed by Hub alone"
        },
        "request_fields": {
            "retry_policy": {
                "max_attempts": "1..20",
                "on_node_lost": ["reschedule", "fail"],
                "on_process_failed": ["reschedule_if_idempotent", "fail"]
            },
            "checkpoint_policy": {
                "enabled": true,
                "mode": ["worker_reported", "none"]
            },
            "idempotency": {
                "key": "stable external key supplied by caller",
                "mode": ["at_least_once", "idempotent", "external_exactly_once"]
            }
        },
        "retry_reschedule_contract": retry_reschedule_standard_contract(),
        "client_flow": [
            "GET /api/capabilities/manifest",
            "POST /api/jobs/plan",
            "POST /api/jobs",
            "GET /api/jobs/{id}",
            "POST /api/jobs/{id}/checkpoints when progress can be resumed"
        ],
        "failure_rules": [
            "Hub leases attempts only to eligible online nodes.",
            "Worker completion clears the lease and marks attempt done.",
            "Worker failure creates another attempt until max_attempts when policy allows it.",
            "Expired lease or offline node marks attempt lost and creates another attempt.",
            "Latest checkpoint is injected into the next attempt payload as resume_from."
        ]
    })
}

fn capability_standard(store: &Store) -> anyhow::Result<Value> {
    let nodes = store.list_nodes()?;
    let tools = store.tool_registry_with_dynamic()?;
    let mut capabilities = Vec::new();
    for capability in [
        "http",
        "command",
        "file",
        "git",
        "docker",
        "browser",
        "session",
        "agentmessage",
        "plugin",
    ] {
        let supported_nodes = nodes
            .iter()
            .filter(|node| {
                node.pointer("/status/state").and_then(Value::as_str) == Some("online")
                    && node
                        .pointer("/spec/capabilities")
                        .and_then(Value::as_array)
                        .map(|items| items.iter().any(|item| item.as_str() == Some(capability)))
                        .unwrap_or(false)
            })
            .map(|node| {
                json!({
                    "node_id": node.pointer("/metadata/id").and_then(Value::as_str),
                    "name": node.pointer("/metadata/name").and_then(Value::as_str),
                    "os": node.pointer("/spec/os").and_then(Value::as_str),
                    "arch": node.pointer("/spec/arch").and_then(Value::as_str),
                    "cpu_cores": node.pointer("/spec/cpu_cores").and_then(Value::as_i64),
                    "memory_mb": node.pointer("/spec/memory_mb").and_then(Value::as_i64),
                    "state": node.pointer("/status/state").and_then(Value::as_str),
                    "worker_target": node.pointer("/spec/worker_target").and_then(Value::as_str),
                    "worker_version": node.pointer("/spec/worker_version").and_then(Value::as_str)
                })
            })
            .collect::<Vec<_>>();
        let tool_ids = tools
            .iter()
            .filter(|tool| tool.get("capability").and_then(Value::as_str) == Some(capability))
            .filter_map(|tool| tool.get("id").and_then(Value::as_str))
            .collect::<Vec<_>>();
        capabilities.push(json!({
            "id": capability,
            "kind": "NodeCapability",
            "tool_ids": tool_ids,
            "node_count": supported_nodes.len(),
            "supported_nodes": supported_nodes
        }));
    }
    Ok(json!({
        "api_version": "agentgrid.runtime/v1",
        "kind": "CapabilityRegistry",
        "generated_at": now(),
        "capabilities": capabilities
    }))
}

fn workbench_standard(store: &Store) -> anyhow::Result<Value> {
    let nodes = store.list_nodes()?;
    let workbenches = nodes
        .iter()
        .map(|node| {
            let node_id = node.pointer("/metadata/id").and_then(Value::as_str).unwrap_or("");
            let capabilities = node
                .pointer("/spec/capabilities")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let capability_ids = capabilities
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>();
            let bench_type = classify_workbench(node_id, &capability_ids);
            json!({
                "id": node_id,
                "kind": "Workbench",
                "name": node.pointer("/metadata/name").and_then(Value::as_str),
                "type": bench_type,
                "state": node.pointer("/status/state").and_then(Value::as_str),
                "os": node.pointer("/spec/os").and_then(Value::as_str),
                "arch": node.pointer("/spec/arch").and_then(Value::as_str),
                "address": node.pointer("/spec/address").and_then(Value::as_str),
                "tags": node.pointer("/spec/tags").cloned().unwrap_or_else(|| json!([])),
                "capabilities": capabilities,
                "resources": {
                    "cpu_cores": node.pointer("/spec/cpu_cores").and_then(Value::as_i64),
                    "memory_mb": node.pointer("/spec/memory_mb").and_then(Value::as_i64),
                    "disk_total_mb": node.pointer("/spec/disk_total_mb").and_then(Value::as_i64),
                    "disk_free_mb": node.pointer("/spec/disk_free_mb").and_then(Value::as_i64),
                    "running_jobs": node.pointer("/status/running_jobs").and_then(Value::as_i64),
                    "max_concurrent_jobs": node.pointer("/spec/max_concurrent_jobs").and_then(Value::as_i64)
                },
                "routing": {
                    "node_label": format!("node:{node_id}"),
                    "os_label": node.pointer("/spec/os").and_then(Value::as_str).map(|os| format!("os:{}", os.to_ascii_lowercase())),
                    "target_rule": "Use node:<id> for operations tied to a physical machine, desktop, device, local SDK, or hardware station."
                }
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "api_version": "agentgrid.workbench/v1",
        "kind": "WorkbenchStandard",
        "generated_at": now(),
        "definition": "A Workbench is a real machine or station that AI can use through structured capabilities. It may represent a cloud node, local computer, desktop helper, hardware bench, browser station, serial station, or SDK/tool station.",
        "types": [
            {
                "id": "hardware_bench",
                "purpose": "Build, flash, test, observe, and collect evidence from physical hardware.",
                "typical_capabilities": ["command", "file", "serial", "flash", "test", "plugin"]
            },
            {
                "id": "desktop_bench",
                "purpose": "Operate a visible Windows/macOS desktop and foreground tools.",
                "typical_capabilities": ["desktop", "browser", "file"]
            },
            {
                "id": "compute_bench",
                "purpose": "Run background compute, build, script, container, Git, and local SDK tasks.",
                "typical_capabilities": ["command", "git", "docker", "session", "plugin"]
            }
        ],
        "routing_rules": [
            "Use a hard node:<node_id> label when the task depends on a real machine, device, desktop, account, SDK install, or local file path.",
            "Use tool_id and capability schema before selecting a node.",
            "Prefer verified tools and online workbenches.",
            "Do not route visible desktop operations to a background service Worker."
        ],
        "items": workbenches
    }))
}

fn classify_workbench(node_id: &str, capabilities: &[&str]) -> &'static str {
    if node_id.ends_with("-desktop") || capabilities.iter().any(|item| *item == "desktop") {
        "desktop_bench"
    } else if capabilities
        .iter()
        .any(|item| matches!(*item, "serial" | "flash" | "device" | "hardware" | "camera"))
    {
        "hardware_bench"
    } else {
        "compute_bench"
    }
}

fn device_standard(store: &Store) -> anyhow::Result<Value> {
    let nodes = store.list_nodes()?;
    let devices = nodes
        .iter()
        .flat_map(|node| {
            let node_id = node.pointer("/metadata/id").and_then(Value::as_str).unwrap_or("");
            let caps = node
                .pointer("/spec/capabilities")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            caps.into_iter().filter_map(move |cap| {
                let id = cap.as_str()?;
                match id {
                    "desktop" => Some(json!({
                        "id": format!("{node_id}:desktop"),
                        "kind": "Device",
                        "type": "desktop",
                        "node_id": node_id,
                        "capability": "desktop",
                        "evidence": ["screenshot", "operation_timeline"],
                        "tools": ["desktop.screenshot", "desktop.click", "desktop.type_text", "desktop.key"]
                    })),
                    "browser" => Some(json!({
                        "id": format!("{node_id}:browser"),
                        "kind": "Device",
                        "type": "browser",
                        "node_id": node_id,
                        "capability": "browser",
                        "evidence": ["page_text", "screenshot", "downloaded_file"],
                        "tools": ["browser.fetch"]
                    })),
                    "file" => Some(json!({
                        "id": format!("{node_id}:filesystem"),
                        "kind": "Device",
                        "type": "filesystem",
                        "node_id": node_id,
                        "capability": "file",
                        "evidence": ["file_artifact", "directory_listing"],
                        "tools": ["file.read", "file.write", "file.list"]
                    })),
                    "plugin" => Some(json!({
                        "id": format!("{node_id}:plugin_runtime"),
                        "kind": "Device",
                        "type": "plugin_runtime",
                        "node_id": node_id,
                        "capability": "plugin",
                        "evidence": ["plugin_result", "stdout", "stderr"],
                        "tools": ["plugin.*"]
                    })),
                    _ => None,
                }
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "api_version": "agentgrid.device/v1",
        "kind": "DeviceStandard",
        "generated_at": now(),
        "definition": "A Device is an addressable thing behind a Workbench capability: desktop, browser, filesystem, serial port, flasher, board, camera, local SDK, or plugin runtime.",
        "device_types": [
            { "id": "desktop", "operations": ["screenshot", "click", "type_text", "key"], "required_evidence": ["screenshot or desktop_result"] },
            { "id": "browser", "operations": ["fetch", "automate"], "required_evidence": ["page text, screenshot, or downloaded file"] },
            { "id": "serial", "operations": ["open", "write", "read", "capture_log"], "required_evidence": ["serial_log"] },
            { "id": "flasher", "operations": ["erase", "flash", "verify"], "required_evidence": ["flash_log", "exit_code"] },
            { "id": "test_rig", "operations": ["run_test", "collect_report"], "required_evidence": ["test_report", "pass_fail"] },
            { "id": "filesystem", "operations": ["list", "read", "write", "download", "upload"], "required_evidence": ["file_artifact or structured listing"] }
        ],
        "registration_rule": "Nodes may register arbitrary device-backed tools through Node Tool Registration; Hub normalizes them into tool contracts and probe status.",
        "items": devices
    }))
}

fn evidence_standard(store: &Store) -> anyhow::Result<Value> {
    Ok(json!({
        "api_version": "agentgrid.evidence/v1",
        "kind": "EvidenceStandard",
        "generated_at": now(),
        "definition": "Evidence is the proof an AI task leaves behind: screenshot, serial log, command log, file artifact, test report, browser result, scheduler reason, and audit timeline.",
        "evidence_types": [
            { "id": "screenshot", "artifact_type": "screenshot", "content_types": ["image/png"], "used_by": ["desktop_bench", "browser_bench"] },
            { "id": "stdout_log", "artifact_type": "log", "content_types": ["text/plain"], "used_by": ["compute_bench", "hardware_bench"] },
            { "id": "stderr_log", "artifact_type": "log", "content_types": ["text/plain"], "used_by": ["compute_bench", "hardware_bench"] },
            { "id": "serial_log", "artifact_type": "log", "content_types": ["text/plain"], "used_by": ["hardware_bench"] },
            { "id": "file_artifact", "artifact_type": "file", "content_types": ["application/octet-stream", "text/plain"], "used_by": ["all"] },
            { "id": "test_report", "artifact_type": "report", "content_types": ["application/json", "text/plain", "text/html"], "used_by": ["hardware_bench", "compute_bench"] },
            { "id": "operation_timeline", "source": "audit_events + task_events + artifacts", "used_by": ["desktop_bench", "hardware_bench"] }
        ],
        "minimum_record": {
            "task_id": "task_xxx",
            "node_id": "workbench node id",
            "created_by": "agent or human identity",
            "operation": "tool id or payload operation",
            "scheduler_reason": "why this workbench was selected",
            "artifacts": [],
            "result": {},
            "error": null,
            "audit": []
        },
        "current_cluster_snapshot": {
            "artifact_count": store.list_artifacts(1000)?.len(),
            "execution_record_endpoint": "/api/execution-records/tasks/{task_id}",
            "artifact_download_endpoint": "/api/artifacts/{artifact_id}/download"
        },
        "rules": [
            "A task without evidence is not enough for hardware or desktop automation.",
            "Screenshots, logs, reports, and output files should be stored as Hub artifacts when possible.",
            "AI clients may summarize evidence, but must preserve artifact ids and task ids.",
            "Every scheduler decision and task operation must remain auditable."
        ]
    }))
}

fn runbook_standard(store: &Store) -> anyhow::Result<Value> {
    Ok(json!({
        "api_version": "agentgrid.runbook/v1",
        "kind": "RunbookStandard",
        "generated_at": now(),
        "definition": "A Runbook is a structured multi-step procedure AI can call against workbenches. It is a product-level pattern above raw tasks and workflows.",
        "runbook_types": [
            {
                "id": "hardware.compile_flash_serial_test",
                "name": "编译-烧录-串口-测试判定",
                "steps": [
                    { "id": "build", "tool": "command.run", "evidence": ["stdout_log", "stderr_log"] },
                    { "id": "flash", "tool": "plugin.flasher or command.run", "evidence": ["flash_log"] },
                    { "id": "serial_capture", "tool": "plugin.serial", "evidence": ["serial_log"] },
                    { "id": "judge", "tool": "plugin.test_judge", "evidence": ["test_report"] }
                ]
            },
            {
                "id": "desktop.observe_operate_collect",
                "name": "桌面观察-操作-采集产物",
                "steps": [
                    { "id": "before", "tool": "desktop.screenshot", "evidence": ["screenshot"] },
                    { "id": "operate", "tool": "desktop.click/type_text/key", "evidence": ["operation_timeline"] },
                    { "id": "after", "tool": "desktop.screenshot", "evidence": ["screenshot"] },
                    { "id": "collect", "tool": "file.read or file.download", "evidence": ["file_artifact"] }
                ]
            },
            {
                "id": "capability.probe_and_use",
                "name": "能力验证后调用",
                "steps": [
                    { "id": "discover", "tool": "GET /api/capabilities/manifest" },
                    { "id": "probe", "tool": "POST /api/node-tools/{tool_id}/probe" },
                    { "id": "submit", "tool": "POST /api/agent-runtime/tasks" },
                    { "id": "record", "tool": "GET /api/execution-records/tasks/{task_id}" }
                ]
            }
        ],
        "schema": {
            "required": ["id", "name", "parameters", "steps"],
            "step_required": ["id", "tool", "payload", "evidence"]
        },
        "implementation_mapping": {
            "single_step": "AgentTask",
            "dependent_steps": "Workflow",
            "recoverable_batch": "Job Runtime",
            "custom_station_action": "Node Tool / Worker Plugin"
        },
        "existing_workflow_templates": store.list_workflow_templates(100)?
    }))
}

fn mobile_sdk_standard() -> Value {
    json!({
        "api_version": "agentgrid.mobile-sdk/v1",
        "kind": "MobileSdkStandard",
        "generated_at": now(),
        "purpose": "Mobile SDKs are console clients for iOS and Android. They let phones view the AgentGrid cluster, submit structured tasks, inspect execution records, and view artifacts such as screenshots.",
        "platforms": [
            {
                "id": "ios",
                "language": "Swift",
                "minimum": "iOS 15",
                "transport": "URLSession async/await",
                "package_path": "sdk/mobile/ios/agentgrid-mobile-sdk-swift"
            },
            {
                "id": "android",
                "language": "Kotlin",
                "minimum": "Android API 23",
                "transport": "HttpURLConnection coroutine wrapper",
                "package_path": "sdk/mobile/android/agentgrid-mobile-sdk-kotlin"
            }
        ],
        "role_boundary": {
            "is": [
                "cluster console client",
                "workbench and node viewer",
                "structured task submitter",
                "execution record and artifact viewer",
                "status polling client"
            ],
            "is_not": [
                "Worker",
                "scheduler",
                "desktop helper",
                "task executor",
                "natural-language parser"
            ]
        },
        "default_hub_url": "http://chenqi.tminos.com:20080/agentgrid",
        "authentication": {
            "current": "No full authorization design is required by this standard version.",
            "reserved_header": "Authorization: Bearer <token>",
            "rule": "SDKs must expose token injection without hard-coding credentials."
        },
        "required_methods": [
            { "name": "health", "method": "GET", "path": "/api/health", "purpose": "Check Hub availability." },
            { "name": "runtimeStandard", "method": "GET", "path": "/api/runtime-standard", "purpose": "Read the complete machine-readable standard." },
            { "name": "mobileSdkStandard", "method": "GET", "path": "/api/runtime-standard/mobile-sdk", "purpose": "Read mobile-specific SDK rules." },
            { "name": "workbenches", "method": "GET", "path": "/api/runtime-standard/workbench", "purpose": "List real machines and stations as AI workbenches." },
            { "name": "devices", "method": "GET", "path": "/api/runtime-standard/devices", "purpose": "List addressable device/tool ports behind workbenches." },
            { "name": "evidenceStandard", "method": "GET", "path": "/api/runtime-standard/evidence", "purpose": "Understand screenshot/log/file/report evidence." },
            { "name": "nodes", "method": "GET", "path": "/api/nodes", "purpose": "List node health, OS, IP/address, and resources." },
            { "name": "tools", "method": "GET", "path": "/api/tools", "purpose": "List callable tools and schemas." },
            { "name": "submitTask", "method": "POST", "path": "/api/agent-runtime/tasks", "purpose": "Submit one structured Runtime task." },
            { "name": "getTask", "method": "GET", "path": "/api/agent-runtime/tasks/{task_id}", "purpose": "Read task state and result snapshot." },
            { "name": "taskEvents", "method": "GET", "path": "/api/agent-runtime/tasks/{task_id}/events", "purpose": "Read task event timeline for polling UIs." },
            { "name": "executionRecord", "method": "GET", "path": "/api/execution-records/tasks/{task_id}", "purpose": "Read business-grade execution record." },
            { "name": "artifacts", "method": "GET", "path": "/api/artifacts", "purpose": "List recent task artifacts." },
            { "name": "artifactDownloadUrl", "method": "LOCAL", "path": "/api/artifacts/{artifact_id}/download", "purpose": "Build absolute artifact URL for image/file viewers." },
            { "name": "taskTemplates", "method": "GET", "path": "/api/task-templates", "purpose": "List reusable task templates." },
            { "name": "startTaskTemplate", "method": "POST", "path": "/api/task-templates/{template_id}/start", "purpose": "Start a task from a template." }
        ],
        "recommended_mobile_screens": [
            {
                "id": "cluster_overview",
                "title": "集群总览",
                "data": ["health", "nodes", "workbenches"],
                "shows": ["online/offline", "OS", "address/IP", "CPU cores", "memory", "disk", "running jobs"]
            },
            {
                "id": "workbench_detail",
                "title": "工位详情",
                "data": ["workbenches", "devices", "tools"],
                "shows": ["capabilities", "device ports", "verified tools", "routing labels"]
            },
            {
                "id": "submit_task",
                "title": "提交任务",
                "data": ["tools", "taskTemplates"],
                "shows": ["tool input schema", "template parameters", "target node"]
            },
            {
                "id": "task_timeline",
                "title": "任务时间线",
                "data": ["getTask", "taskEvents", "executionRecord"],
                "shows": ["state", "scheduler reason", "stdout/stderr", "error", "audit"]
            },
            {
                "id": "artifact_viewer",
                "title": "产物查看",
                "data": ["executionRecord", "artifacts", "artifactDownloadUrl"],
                "shows": ["screenshots", "logs", "reports", "downloadable files"]
            }
        ],
        "polling_policy": {
            "default_interval_ms": 2000,
            "backoff_after_seconds": 60,
            "stop_on_states": ["done", "failed", "cancelled", "stopped", "skipped"],
            "future_realtime": "SSE for task events and WebSocket only for interactive terminal/session."
        },
        "task_submission_rule": "Mobile SDKs submit structured JSON to Agent Runtime. Natural language must be converted by the mobile app or AI client before calling AgentGrid.",
        "artifact_rule": "SDKs should keep artifact ids and use Hub download URLs. Screenshots and files are Hub artifacts, not embedded in mobile task summaries.",
        "compatibility_rule": "Mobile SDKs must avoid depending on Worker internals. They only call public Hub APIs listed in this standard."
    })
}

fn plugin_runtime_standard(store: &Store) -> anyhow::Result<Value> {
    let node_tools = store.list_node_tools(None)?;
    let plugin_tools = node_tools
        .iter()
        .filter(|tool| {
            tool.pointer("/spec/executor")
                .and_then(Value::as_str)
                .map(|executor| executor.starts_with("plugin:"))
                .unwrap_or(false)
        })
        .cloned()
        .collect::<Vec<_>>();
    let plugins = plugin_tools
        .iter()
        .map(plugin_manifest_from_node_tool)
        .collect::<Vec<_>>();
    Ok(json!({
        "api_version": "agentgrid.plugin-runtime/v1",
        "kind": "PluginRuntimeStandard",
        "generated_at": now(),
        "definition": "Plugin Runtime v1 defines how node-local plugin packages declare tools, versions, dependencies, health checks, result contracts, and installation metadata.",
        "identity": {
            "plugin_id": "stable package id, for example agentgrid-plugin-document-parser",
            "tool_id": "AI-facing callable tool id, for example document.parse",
            "executor": "plugin:<plugin_id>",
            "version": "semver string"
        },
        "manifest_schema": {
            "type": "object",
            "required": ["plugin_id", "version", "tools", "entrypoint"],
            "properties": {
                "plugin_id": { "type": "string" },
                "name": { "type": "string" },
                "version": { "type": "string" },
                "author": { "type": "string" },
                "description": { "type": "string" },
                "platforms": { "type": "array", "items": { "type": "string" } },
                "entrypoint": { "type": "string" },
                "dependencies": { "type": "array" },
                "tools": { "type": "array" },
                "probe": { "type": "object" },
                "risk": { "type": "string", "enum": ["low", "medium", "high"] }
            }
        },
        "execution_request": {
            "api_version": "agentgrid.plugin/v1",
            "kind": "WorkerPluginRequest",
            "plugin_id": "plugin id",
            "action": "run/probe/custom action",
            "input": {}
        },
        "execution_result": {
            "type": "plugin_result",
            "plugin_id": "plugin id",
            "action": "action name",
            "output": {},
            "artifacts": [],
            "duration_ms": 0
        },
        "error_result": {
            "code": "plugin_not_found | plugin_failed | plugin_timeout | invalid_plugin_output | dependency_missing",
            "message": "error summary",
            "retryable": false
        },
        "install_model": {
            "package": "manifest.json + executable/script + README + examples",
            "default_directory": "/opt/agentgrid/plugins",
            "windows_directory": "C:\\\\Program Files\\\\AgentGridWorker\\\\plugins",
            "registration": "Worker or operator registers tool manifest through POST /api/nodes/{node_id}/tools",
            "health_check": "Hub schedules a probe task using manifest.probe.payload"
        },
        "rules": [
            "Plugin package id and tool id are different. Plugin is implementation; tool is callable contract.",
            "Every plugin-backed tool must publish input_schema, output_schema, version, executor, risk, and probe when possible.",
            "Plugin stdout must be JSON. Non-JSON stdout is wrapped but should be treated as low maturity.",
            "High-risk plugins should require verified probe status before broad scheduling.",
            "Plugins should return artifacts by id or artifact descriptors instead of embedding large outputs."
        ],
        "current_snapshot": {
            "plugin_tool_count": plugin_tools.len(),
            "plugins": plugins
        }
    }))
}

fn capability_graph_standard(store: &Store) -> anyhow::Result<Value> {
    let nodes = store.list_nodes()?;
    let node_tools = store.list_node_tools(None)?;
    let tools = store
        .tool_registry_with_dynamic()?
        .into_iter()
        .map(|tool| store.enrich_tool_with_nodes(tool, &nodes))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let devices = device_standard(store)?
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut graph_nodes = Vec::new();
    let mut edges = Vec::new();

    for node in &nodes {
        let node_id = node
            .pointer("/metadata/id")
            .and_then(Value::as_str)
            .unwrap_or("");
        let capabilities = node
            .pointer("/spec/capabilities")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        graph_nodes.push(json!({
            "id": node_id,
            "kind": "node",
            "name": node.pointer("/metadata/name").and_then(Value::as_str),
            "state": node.pointer("/status/state").and_then(Value::as_str),
            "os": node.pointer("/spec/os").and_then(Value::as_str),
            "address": node.pointer("/spec/address").and_then(Value::as_str),
            "capabilities": capabilities
        }));
        for capability in capabilities.iter().filter_map(Value::as_str) {
            edges.push(json!({
                "from": node_id,
                "to": format!("capability:{capability}"),
                "type": "node_has_capability"
            }));
        }
    }

    for device in &devices {
        let device_id = device.get("id").and_then(Value::as_str).unwrap_or("");
        let node_id = device.get("node_id").and_then(Value::as_str).unwrap_or("");
        graph_nodes.push(json!({
            "id": device_id,
            "kind": "device",
            "type": device.get("type").and_then(Value::as_str),
            "node_id": node_id,
            "tools": device.get("tools").cloned().unwrap_or_else(|| json!([])),
            "evidence": device.get("evidence").cloned().unwrap_or_else(|| json!([]))
        }));
        edges.push(json!({
            "from": node_id,
            "to": device_id,
            "type": "node_exposes_device"
        }));
    }

    for tool in &tools {
        let tool_id = tool.get("id").and_then(Value::as_str).unwrap_or("");
        let capability = tool
            .get("capability")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let payload_type = tool
            .get("payload_type")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        graph_nodes.push(json!({
            "id": tool_id,
            "kind": "tool",
            "name": tool.get("name").and_then(Value::as_str),
            "capability": capability,
            "payload_type": payload_type,
            "risk": tool.get("risk").and_then(Value::as_str),
            "available_nodes": tool.get("node_count").and_then(Value::as_i64).unwrap_or(0),
            "verified_nodes": tool.get("verified_node_count").and_then(Value::as_i64).unwrap_or(0)
        }));
        edges.push(json!({
            "from": format!("capability:{capability}"),
            "to": tool_id,
            "type": "capability_exposes_tool"
        }));
        if let Some(nodes) = tool.get("nodes").and_then(Value::as_array) {
            for node in nodes {
                if let Some(node_id) = node.get("id").and_then(Value::as_str) {
                    edges.push(json!({
                        "from": node_id,
                        "to": tool_id,
                        "type": "node_supports_tool",
                        "verification_status": node.get("verification_status").and_then(Value::as_str).unwrap_or("unknown")
                    }));
                }
            }
        }
        for evidence in evidence_for_payload_type(payload_type) {
            edges.push(json!({
                "from": tool_id,
                "to": format!("evidence:{evidence}"),
                "type": "tool_produces_evidence"
            }));
        }
    }

    for item in &node_tools {
        let node_id = item
            .pointer("/metadata/node_id")
            .and_then(Value::as_str)
            .unwrap_or("");
        let tool_id = item
            .pointer("/spec/tool_id")
            .and_then(Value::as_str)
            .unwrap_or("");
        if let Some(executor) = item.pointer("/spec/executor").and_then(Value::as_str) {
            if let Some(plugin_id) = executor.strip_prefix("plugin:") {
                edges.push(json!({
                    "from": tool_id,
                    "to": format!("plugin:{plugin_id}"),
                    "type": "tool_depends_on_plugin",
                    "node_id": node_id
                }));
            }
        }
    }

    Ok(json!({
        "api_version": "agentgrid.capability-graph/v1",
        "kind": "CapabilityGraphStandard",
        "generated_at": now(),
        "definition": "Capability Graph connects nodes, devices, tools, plugins, evidence, and suitable task intents so AI clients can plan against real machine capabilities instead of flat labels.",
        "node_types": ["node", "capability", "device", "tool", "plugin", "evidence", "task_intent"],
        "edge_types": [
            "node_has_capability",
            "node_exposes_device",
            "capability_exposes_tool",
            "node_supports_tool",
            "tool_depends_on_plugin",
            "tool_produces_evidence",
            "tool_satisfies_task_intent"
        ],
        "rules": [
            "A capability is not enough for scheduling when a concrete device, plugin, or local resource is required.",
            "Node-specific tools are first-class graph nodes.",
            "Probe verification status must be attached to node_supports_tool edges.",
            "AI clients should choose tools through graph relationships, not free-text guesses."
        ],
        "snapshot": {
            "nodes": graph_nodes,
            "edges": edges,
            "counts": {
                "node_count": nodes.len(),
                "device_count": devices.len(),
                "tool_count": tools.len(),
                "node_tool_count": node_tools.len()
            }
        }
    }))
}

fn execution_contract_standard(store: &Store) -> anyhow::Result<Value> {
    let tools = store.tool_registry_with_dynamic()?;
    let contract_families = tools
        .iter()
        .map(|tool| {
            let tool_id = tool.get("id").and_then(Value::as_str).unwrap_or("");
            let payload_type = tool
                .get("payload_type")
                .and_then(Value::as_str)
                .unwrap_or("custom");
            json!({
                "tool_id": tool_id,
                "payload_type": payload_type,
                "capability": tool.get("capability").and_then(Value::as_str),
                "input_schema": tool.get("input_schema").cloned().unwrap_or_else(|| json!({})),
                "output_schema": tool.get("output_schema").cloned().unwrap_or_else(|| json!({})),
                "default_timeout_seconds": default_timeout_for_payload_type(payload_type),
                "retryable": retryable_for_payload_type(payload_type),
                "recoverable": recoverable_for_payload_type(payload_type),
                "evidence": evidence_for_payload_type(payload_type),
                "audit_events": audit_events_for_payload_type(payload_type),
                "error_shape": {
                    "code": "stable machine-readable error code",
                    "message": "human readable message",
                    "retryable": false,
                    "result": "optional partial structured result"
                }
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "api_version": "agentgrid.execution-contract/v1",
        "kind": "ExecutionContractStandard",
        "generated_at": now(),
        "definition": "Execution Contract normalizes inputs, outputs, errors, timeout, retry, recovery, artifacts, and audit events for every tool family.",
        "required_sections": [
            "input_schema",
            "output_schema",
            "timeout",
            "retry_policy",
            "recovery_policy",
            "artifacts",
            "audit_events",
            "error_shape"
        ],
        "state_model": task_state_machine_standard(),
        "common_result_envelope": {
            "type": "tool-specific result type",
            "duration_ms": 0,
            "artifacts": [],
            "verification": null
        },
        "common_error_envelope": {
            "code": "policy_denied | timeout | process_failed | invalid_payload | unavailable | unknown",
            "message": "error summary",
            "retryable": false,
            "result": null
        },
        "families": contract_families,
        "rules": [
            "Every new tool or plugin must publish input_schema and output_schema.",
            "Workers must return structured result or structured error.",
            "Timeout and artifact rules belong to the execution contract, not UI copy.",
            "Retries require idempotency or a checkpoint-aware executor."
        ]
    }))
}

fn evidence_pipeline_standard(store: &Store) -> anyhow::Result<Value> {
    let artifacts = store.list_artifacts(1000)?;
    let artifact_count = artifacts.len();
    let recent = artifacts.into_iter().take(20).collect::<Vec<_>>();
    Ok(json!({
        "api_version": "agentgrid.evidence-pipeline/v1",
        "kind": "EvidencePipelineStandard",
        "generated_at": now(),
        "definition": "Evidence Pipeline turns screenshots, logs, files, reports, serial output, and operation timelines into indexed Hub artifacts connected to tasks and execution records.",
        "stages": [
            { "id": "capture", "actor": "worker/plugin/desktop-helper", "output": "raw evidence bytes or structured JSON" },
            { "id": "normalize", "actor": "hub", "output": "artifact metadata, content type, size, hash, preview hint" },
            { "id": "index", "actor": "hub", "output": "task_id, node_id, tool_id, evidence_type, created_at" },
            { "id": "preview", "actor": "web/mobile", "output": "image/log/report/file preview" },
            { "id": "audit", "actor": "hub", "output": "event timeline and execution record links" }
        ],
        "evidence_types": evidence_standard(store)?.get("evidence_types").cloned().unwrap_or_else(|| json!([])),
        "artifact_rules": {
            "screenshots": "Store as image/png artifact with task_id and node_id.",
            "logs": "Store stdout/stderr/serial as text artifacts or structured result fields.",
            "reports": "Store JSON/text/html reports with content_type.",
            "files": "Store binary files with size, sha256, content_type, and download_url.",
            "operation_timeline": "Build from task events, audit events, and artifacts."
        },
        "current_snapshot": {
            "artifact_count": artifact_count,
            "recent_artifacts": recent
        }
    }))
}

fn probe_engine_standard(store: &Store) -> anyhow::Result<Value> {
    let node_tools = store.list_node_tools(None)?;
    let due = store.due_node_tools_for_probe(50)?;
    let mut states: HashMap<String, usize> = HashMap::new();
    for tool in &node_tools {
        let state = tool
            .pointer("/status/probe_state")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        *states.entry(state.to_string()).or_default() += 1;
    }
    Ok(json!({
        "api_version": "agentgrid.probe-engine/v1",
        "kind": "NodeCapabilityProbeEngineStandard",
        "generated_at": now(),
        "definition": "Probe Engine verifies that a declared node tool or capability is currently usable by scheduling structured probe tasks and updating per-node health.",
        "probe_states": [
            "declared_unverified",
            "pending",
            "verified",
            "failed",
            "expired",
            "unsupported",
            "unavailable"
        ],
        "triggers": [
            "manual",
            "registration",
            "interval_due",
            "before_high_value_task",
            "after_worker_restart",
            "after_contract_changed"
        ],
        "flow": [
            "Hub reads node tool registration.",
            "Hub creates probe AgentTask with hard node placement.",
            "Worker executes the same tool/plugin path that real tasks use.",
            "Hub verifies result with probe.verify rules.",
            "Hub updates node_tools.probe_state and tool_probes history.",
            "Scheduler prefers verified tools and avoids failed/unavailable tools."
        ],
        "api": {
            "list_node_tools": "GET /api/node-tools",
            "probe_all": "POST /api/node-tools/probe",
            "probe_tool": "POST /api/node-tools/{tool_id}/probe",
            "probe_tool_node": "POST /api/node-tools/{tool_id}/nodes/{node_id}/probe"
        },
        "current_snapshot": {
            "registered_node_tools": node_tools.len(),
            "due_for_probe": due.len(),
            "probe_state_counts": states,
            "due_items": due
        }
    }))
}

fn placement_engine_standard(store: &Store) -> anyhow::Result<Value> {
    let scheduler_config = store.scheduler_config()?;
    Ok(json!({
        "api_version": "agentgrid.placement-engine/v1",
        "kind": "PlacementEngineStandard",
        "generated_at": now(),
        "definition": "Placement Engine turns task intent, tool contracts, node state, probe status, resources, and business constraints into a scheduler decision.",
        "constraint_types": {
            "hard": [
                "node_id",
                "node:<id> label",
                "os",
                "required_tool_id",
                "required_capability",
                "device_id",
                "online_state",
                "policy_allowed"
            ],
            "soft": [
                "preferred_nodes",
                "avoid_nodes",
                "low_cpu",
                "low_memory",
                "low_disk_pressure",
                "free_concurrency_slots",
                "verified_probe_status",
                "historical_success_rate",
                "node_weight",
                "risk_level",
                "cost"
            ]
        },
        "scoring": {
            "lower_is_better": true,
            "resource_score": "cpu_usage * 0.45 + memory_usage * 0.30 + disk_usage * 0.15 + running_jobs * 0.8",
            "probe_bonus": "verified tools are preferred after hard constraints pass",
            "high_load_limit": HIGH_LOAD_SCORE_LIMIT
        },
        "decision_record": {
            "required": [
                "selected_node_id",
                "eligible_nodes",
                "rejected_nodes",
                "scores",
                "hard_constraints",
                "soft_preferences",
                "reason"
            ],
            "stored_in": "task execution record schedule section"
        },
        "current_config": scheduler_config,
        "rules": [
            "Offline and unknown nodes cannot receive new work.",
            "Hard placement always wins over resource optimization.",
            "Visible desktop operations must target Desktop Helper nodes.",
            "High-risk tools should prefer verified tools and explicit node placement."
        ]
    }))
}

fn task_intent_standard() -> Value {
    json!({
        "api_version": "agentgrid.task-intent/v1",
        "kind": "TaskIntentSchemaStandard",
        "generated_at": now(),
        "definition": "Task Intent is structured JSON produced by an AI client or application. AgentGrid does not parse natural language; it validates and schedules this intent.",
        "schema": {
            "type": "object",
            "required": ["intent_type", "tool_id", "payload"],
            "properties": {
                "intent_type": {
                    "type": "string",
                    "examples": [
                        "desktop.screenshot",
                        "hardware.flash",
                        "browser.extract",
                        "document.parse",
                        "command.run"
                    ]
                },
                "title": { "type": "string" },
                "tool_id": { "type": "string" },
                "payload": { "type": "object" },
                "placement": {
                    "type": "object",
                    "properties": {
                        "node_id": { "type": "string" },
                        "os": { "type": "string" },
                        "required_capabilities": { "type": "array" },
                        "required_devices": { "type": "array" },
                        "preferred_nodes": { "type": "array" },
                        "avoid_nodes": { "type": "array" }
                    }
                },
                "evidence": { "type": "array" },
                "reliability": {
                    "type": "object",
                    "properties": {
                        "timeout_seconds": { "type": "integer" },
                        "idempotency_key": { "type": "string" },
                        "retry_policy": { "type": "object" },
                        "checkpoint_policy": { "type": "object" }
                    }
                }
            }
        },
        "examples": [
            {
                "intent_type": "desktop.screenshot",
                "title": "Capture Windows desktop",
                "tool_id": "desktop.screenshot",
                "placement": { "node_id": "ZZH0610-windows-desktop", "required_capabilities": ["desktop"] },
                "payload": { "type": "desktop", "operation": "screenshot" },
                "evidence": ["screenshot"]
            },
            {
                "intent_type": "document.parse",
                "title": "Parse uploaded contract",
                "tool_id": "document.parse",
                "placement": { "required_capabilities": ["plugin"] },
                "payload": { "type": "document", "file_artifact_id": "artifact_xxx", "extract_mode": "text" },
                "evidence": ["file_artifact", "test_report"]
            },
            {
                "intent_type": "hardware.flash",
                "title": "Flash board and capture serial",
                "tool_id": "hardware.flash",
                "placement": { "required_devices": ["flasher", "serial"] },
                "payload": { "type": "hardware", "operation": "flash", "firmware_artifact_id": "artifact_xxx" },
                "evidence": ["flash_log", "serial_log"]
            }
        ],
        "conversion_rule": "Natural language belongs to the AI client. AgentGrid receives only TaskIntent JSON or direct Runtime task JSON.",
        "mapping": {
            "single_step": "POST /api/agent-runtime/tasks",
            "recoverable_batch": "POST /api/jobs",
            "dependent_steps": "POST /api/workflows"
        }
    })
}

fn artifact_store_standard(store: &Store) -> anyhow::Result<Value> {
    let artifacts = store.list_artifacts(1000)?;
    let total_bytes = artifacts
        .iter()
        .filter_map(|artifact| artifact.pointer("/spec/size_bytes").and_then(Value::as_i64))
        .sum::<i64>();
    Ok(json!({
        "api_version": "agentgrid.artifact-store/v2",
        "kind": "ArtifactStoreStandard",
        "generated_at": now(),
        "definition": "Artifact Store v2 is the normalized storage contract for screenshots, logs, reports, binary files, and large task outputs.",
        "metadata_schema": {
            "required": ["id", "task_id", "node_id", "artifact_type", "content_type", "size_bytes", "sha256", "created_at"],
            "optional": ["tool_id", "job_id", "workflow_id", "filename", "preview_url", "download_url", "retention_policy", "labels"]
        },
        "capabilities": [
            "content_type_detection",
            "sha256_hash",
            "task_node_linking",
            "download_url",
            "image_preview",
            "text_log_preview",
            "report_preview",
            "future_large_file_chunks",
            "future_retention_policy"
        ],
        "api": {
            "list": "GET /api/artifacts",
            "download": "GET /api/artifacts/{artifact_id}/download",
            "execution_record_path": "execution.artifacts"
        },
        "rules": [
            "Artifacts should be referenced by id and download_url, not copied into summaries.",
            "Large stdout/stderr should become log artifacts.",
            "Screenshots must use image content types.",
            "Artifacts must remain linked to task_id and node_id for audit."
        ],
        "current_snapshot": {
            "artifact_count": artifacts.len(),
            "total_bytes": total_bytes,
            "recent_artifacts": artifacts.into_iter().take(20).collect::<Vec<_>>()
        }
    }))
}

fn event_timeline_standard(store: &Store) -> anyhow::Result<Value> {
    let events = store.list_events(
        EventQuery {
            limit: Some(50),
            event_type: None,
            type_alias: None,
            subject_id: None,
        },
        50,
    )?;
    let audit = store.list_audit_events(50)?;
    Ok(json!({
        "api_version": "agentgrid.event-timeline/v1",
        "kind": "EventTimelineStandard",
        "generated_at": now(),
        "definition": "Event Timeline is the shared event stream for tasks, jobs, nodes, plugins, probes, evidence, scheduler decisions, Web, Mobile, Webhook, and MCP clients.",
        "sources": [
            "task_events",
            "audit_events",
            "node_heartbeats",
            "probe_events",
            "artifact_events",
            "job_events",
            "workflow_events",
            "webhook_delivery_events"
        ],
        "event_shape": {
            "id": "evt_xxx",
            "type": "task.completed",
            "subject_id": "task_xxx",
            "project_id": PROJECT_ID,
            "created_at": "RFC3339 timestamp",
            "data": {}
        },
        "subscriptions": {
            "polling": "GET /api/events?limit=100",
            "sse": "GET /api/events/stream",
            "task_events": "GET /api/agent-runtime/tasks/{task_id}/events",
            "webhooks": "GET/POST /api/webhooks",
            "interactive_terminal": "WebSocket /api/terminal/ws"
        },
        "rules": [
            "All important state changes should be represented as events.",
            "Console, Mobile SDK, Webhook, and MCP should read the same event stream.",
            "Long-running command/session output should use task events or future log stream events.",
            "Execution records are built from task state, artifacts, notifications, audit, and timeline events."
        ],
        "current_snapshot": {
            "recent_events": events,
            "recent_audit_events": audit
        }
    }))
}

fn evidence_for_payload_type(payload_type: &str) -> Vec<&'static str> {
    match payload_type {
        "desktop" => vec!["screenshot", "operation_timeline"],
        "browser" => vec!["screenshot", "file_artifact"],
        "command" | "session" | "docker" | "git" => vec!["stdout_log", "stderr_log"],
        "file" => vec!["file_artifact"],
        "plugin" => vec!["plugin_result", "stdout_log", "stderr_log"],
        "http_request" => vec!["test_report"],
        "agentmessage" => vec!["operation_timeline"],
        _ => vec!["test_report"],
    }
}

fn plugin_manifest_from_node_tool(tool: &Value) -> Value {
    let tool_id = tool
        .pointer("/spec/tool_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let executor = tool
        .pointer("/spec/executor")
        .and_then(Value::as_str)
        .unwrap_or("");
    let plugin_id = executor.strip_prefix("plugin:").unwrap_or(executor);
    let metadata = tool
        .pointer("/spec/metadata")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let manifest = metadata
        .get("manifest")
        .cloned()
        .or_else(|| metadata.get("plugin_manifest").cloned())
        .unwrap_or_else(|| json!({}));
    json!({
        "plugin_id": manifest.get("plugin_id").and_then(Value::as_str).unwrap_or(plugin_id),
        "tool_id": tool_id,
        "name": manifest.get("name").and_then(Value::as_str).or_else(|| tool.pointer("/spec/name").and_then(Value::as_str)),
        "version": manifest.get("version").and_then(Value::as_str).or_else(|| tool.pointer("/spec/version").and_then(Value::as_str)),
        "executor": executor,
        "node_id": tool.pointer("/metadata/node_id").and_then(Value::as_str),
        "status": tool.pointer("/status/state").and_then(Value::as_str),
        "probe_state": tool.pointer("/status/probe_state").and_then(Value::as_str),
        "risk": metadata.get("risk").and_then(Value::as_str).unwrap_or("high"),
        "author": manifest.get("author").and_then(Value::as_str).unwrap_or("unknown"),
        "platforms": manifest.get("platforms").cloned().unwrap_or_else(|| json!(["node-local"])),
        "entrypoint": manifest.get("entrypoint").and_then(Value::as_str).unwrap_or(plugin_id),
        "dependencies": manifest.get("dependencies").cloned().unwrap_or_else(|| json!([])),
        "input_schema": tool.pointer("/spec/input_schema").cloned().unwrap_or_else(|| json!({})),
        "output_schema": tool.pointer("/spec/output_schema").cloned().unwrap_or_else(|| json!({})),
        "probe": tool.pointer("/spec/probe").cloned().unwrap_or(Value::Null),
        "install": manifest.get("install").cloned().unwrap_or_else(|| json!({
            "type": "node_local",
            "path": format!("$AGENTGRID_PLUGIN_DIR/{plugin_id}")
        }))
    })
}

fn artifact_v2_metadata(input: &ArtifactInput<'_>) -> Value {
    let mut metadata = input.metadata.clone();
    ensure_object(&mut metadata);
    let content_hash = input
        .content_base64
        .map(|content| sha256_hex(content.as_bytes()));
    let preview = artifact_preview_kind(input.artifact_type, input.content_type, input.size_bytes);
    let retention = json!({
        "policy": "default",
        "ttl_days": 30,
        "reason": "agentgrid.artifact-store/v2 default retention"
    });
    if let Some(map) = metadata.as_object_mut() {
        map.insert("artifact_store_version".to_string(), json!("v2"));
        map.insert(
            "tool_id".to_string(),
            input.tool_id.map(Value::from).unwrap_or(Value::Null),
        );
        map.insert(
            "sha256".to_string(),
            content_hash.map(Value::from).unwrap_or(Value::Null),
        );
        map.insert("preview".to_string(), preview);
        map.insert("retention".to_string(), retention);
        map.insert(
            "large_file".to_string(),
            json!({
                "enabled": input.size_bytes > 1_048_576,
                "chunk_size_bytes": 262_144,
                "chunk_count": if input.size_bytes > 1_048_576 {
                    ((input.size_bytes + 262_143) / 262_144) as i64
                } else {
                    1
                }
            }),
        );
    }
    metadata
}

fn artifact_preview_kind(artifact_type: &str, content_type: &str, size_bytes: u64) -> Value {
    let kind = if content_type.starts_with("image/") {
        "image"
    } else if content_type.starts_with("text/") || artifact_type == "log" {
        "text"
    } else if content_type.contains("json") || artifact_type == "report" {
        "json"
    } else {
        "download"
    };
    json!({
        "kind": kind,
        "inline": size_bytes <= 1_048_576 && matches!(kind, "image" | "text" | "json"),
        "max_inline_bytes": 1_048_576
    })
}

fn artifact_v2_view(metadata: &Value) -> Value {
    json!({
        "sha256": metadata.get("sha256").cloned().unwrap_or(Value::Null),
        "preview": metadata.get("preview").cloned().unwrap_or_else(|| json!({ "kind": "download", "inline": false })),
        "retention": metadata.get("retention").cloned().unwrap_or_else(|| json!({ "policy": "default" })),
        "large_file": metadata.get("large_file").cloned().unwrap_or_else(|| json!({ "enabled": false }))
    })
}

fn audit_events_for_payload_type(payload_type: &str) -> Vec<&'static str> {
    match payload_type {
        "desktop" => vec![
            "task.created",
            "task.leased",
            "desktop.operation",
            "artifact.created",
            "task.completed",
        ],
        "plugin" => vec![
            "task.created",
            "task.leased",
            "plugin.executed",
            "task.completed",
        ],
        "file" => vec![
            "task.created",
            "task.leased",
            "file.operation",
            "artifact.created",
            "task.completed",
        ],
        _ => vec![
            "task.created",
            "task.leased",
            "task.completed",
            "task.failed",
        ],
    }
}

fn default_timeout_for_payload_type(payload_type: &str) -> i64 {
    match payload_type {
        "http_request" => 30,
        "desktop" => 30,
        "file" => 60,
        "command" => 120,
        "browser" => 120,
        "git" => 300,
        "docker" => 600,
        "session" => 3600,
        "plugin" => 300,
        _ => 120,
    }
}

fn retryable_for_payload_type(payload_type: &str) -> bool {
    matches!(
        payload_type,
        "http_request" | "browser" | "git" | "docker" | "plugin"
    )
}

fn recoverable_for_payload_type(payload_type: &str) -> bool {
    matches!(payload_type, "session" | "docker" | "git" | "plugin")
}

fn capability_marketplace_standard(store: &Store) -> anyhow::Result<Value> {
    let nodes = store.list_nodes()?;
    let tools = store
        .tool_registry_with_dynamic()?
        .into_iter()
        .map(|tool| store.enrich_tool_with_nodes(tool, &nodes))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let marketplace_tools = tools
        .into_iter()
        .map(|tool| {
            json!({
                "tool_id": tool.get("id").and_then(Value::as_str),
                "name": tool.get("name").and_then(Value::as_str),
                "category": tool.get("category").and_then(Value::as_str),
                "capability": tool.get("capability").and_then(Value::as_str),
                "risk": tool.get("risk").and_then(Value::as_str),
                "available_nodes": tool.get("node_count").and_then(Value::as_i64).unwrap_or(0),
                "verified_nodes": tool.get("verified_node_count").and_then(Value::as_i64).unwrap_or(0),
                "input_schema": tool.get("input_schema").cloned().unwrap_or_else(|| json!({})),
                "examples": tool.get("examples").cloned().unwrap_or_else(|| json!([]))
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "api_version": "agentgrid.marketplace/v1",
        "kind": "CapabilityMarketplaceStandard",
        "generated_at": now(),
        "definition": "The marketplace is the AI-facing catalog of what real machines and stations can do right now.",
        "rules": [
            "Every marketplace item must have a stable tool_id.",
            "Every callable tool must publish input_schema and output_schema.",
            "Node-specific tools are allowed and expected.",
            "Probe status should influence scheduling and AI trust.",
            "Capabilities are heterogeneous; AgentGrid standardizes contracts, not the machines themselves."
        ],
        "items": marketplace_tools
    }))
}

fn task_state_machine_standard() -> Value {
    json!({
        "api_version": "agentgrid.runtime/v1",
        "kind": "TaskStateMachine",
        "states": [
            { "id": "assigned", "terminal": false, "description": "Task exists and waits for scheduler lease." },
            { "id": "in_progress", "terminal": false, "description": "Task is leased by a Worker and may be running." },
            { "id": "review", "terminal": false, "description": "Human or AI review is expected before final closure." },
            { "id": "blocked", "terminal": false, "description": "Task cannot proceed until an external condition changes." },
            { "id": "stopping", "terminal": false, "description": "Stop signal has been requested." },
            { "id": "stopped", "terminal": true, "description": "Running task was stopped." },
            { "id": "done", "terminal": true, "description": "Task finished successfully and result is available." },
            { "id": "failed", "terminal": true, "description": "Task failed and is not retried automatically." },
            { "id": "cancelled", "terminal": true, "description": "Queued task was cancelled before successful completion." },
            { "id": "skipped", "terminal": true, "description": "Workflow run was intentionally skipped by failure policy." }
        ],
        "transitions": [
            { "from": "assigned", "to": "in_progress", "actor": "scheduler", "event": "task.leased" },
            { "from": "assigned", "to": "cancelled", "actor": "operator", "event": "task.cancelled" },
            { "from": "in_progress", "to": "done", "actor": "worker", "event": "task.completed" },
            { "from": "in_progress", "to": "failed", "actor": "worker", "event": "task.failed" },
            { "from": "in_progress", "to": "stopping", "actor": "operator", "event": "task.stop_requested" },
            { "from": "stopping", "to": "stopped", "actor": "worker", "event": "task.stopped" },
            { "from": "failed", "to": "skipped", "actor": "workflow-engine", "event": "workflow.node.skipped", "scope": "workflow_run" },
            { "from": "review", "to": "done", "actor": "operator_or_agent", "event": "task.approved" }
        ],
        "terminal_states": ["done", "failed", "cancelled", "stopped", "skipped"],
        "rules": [
            "Worker complete endpoint may only produce done.",
            "Worker fail endpoint produces failed or stopped.",
            "Non-zero command/git/docker/session exit_code is failed.",
            "Hub does not retry failed tasks automatically.",
            "Workflow dependencies are satisfied by done or skipped runs."
        ]
    })
}

fn workflow_template_standard(store: &Store) -> anyhow::Result<Value> {
    Ok(json!({
        "api_version": "agentgrid.runtime/v1",
        "kind": "WorkflowTemplateStandard",
        "endpoints": {
            "list": "/api/workflow-templates",
            "create": "POST /api/workflow-templates",
            "start": "POST /api/workflow-templates/{template_id}/start"
        },
        "schema": {
            "type": "object",
            "required": ["id", "name", "nodes"],
            "properties": {
                "id": { "type": "string" },
                "name": { "type": "string" },
                "summary": { "type": "string" },
                "parameters": { "type": "array" },
                "nodes": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["id", "title", "payload"],
                        "properties": {
                            "id": { "type": "string" },
                            "title": { "type": "string" },
                            "summary": { "type": "string" },
                            "payload": { "type": "object" },
                            "depends_on": { "type": "array", "items": { "type": "string" } },
                            "labels": { "type": "array", "items": { "type": "string" } },
                            "optional": { "type": "boolean" },
                            "on_failure": { "type": "string", "enum": ["fail_workflow", "continue"] }
                        }
                    }
                }
            }
        },
        "template_expression": "${parameter_name}",
        "failure_policy": {
            "default": "fail_workflow",
            "continue": "mark failed workflow run as skipped and release downstream nodes"
        },
        "items": store.list_workflow_templates(100)?
    }))
}

fn result_report_standard(store: &Store) -> anyhow::Result<Value> {
    let diagnostics = store.diagnostics()?;
    Ok(json!({
        "api_version": "agentgrid.runtime/v1",
        "kind": "ResultReportStandard",
        "generated_at": now(),
        "task_report": {
            "source": "/api/execution-records/tasks/{task_id}",
            "required_sections": ["summary", "input", "schedule", "execution", "notifications", "audit", "raw"],
            "result_path": "execution.result",
            "error_path": "execution.error",
            "verification_path": "execution.verification",
            "artifacts_path": "execution.artifacts"
        },
        "workflow_report": {
            "source": "/api/execution-records/workflows/{workflow_id}",
            "required_sections": ["summary", "definition", "runs", "tasks", "result", "error", "notifications", "audit", "raw"],
            "step_result_path": "result.steps[]",
            "skipped_count_path": "summary.skipped_count"
        },
        "cluster_report": {
            "source": "/api/diagnostics",
            "snapshot": diagnostics
        },
        "rules": [
            "Reports are structured JSON, not natural language summaries.",
            "AI clients may summarize reports externally, but must preserve task_id/workflow_id references.",
            "Failures must include error.code and error.message when available.",
            "Artifacts must be referenced by artifact id or download_url."
        ]
    }))
}

fn execution_record_standard() -> Value {
    json!({
        "api_version": "agentgrid.runtime/v1",
        "kind": "ExecutionRecordStandard",
        "task_endpoint": "/api/execution-records/tasks/{task_id}",
        "workflow_endpoint": "/api/execution-records/workflows/{workflow_id}",
        "sections": {
            "summary": "small stable overview",
            "input": "original and parsed payloads",
            "schedule": "scheduler decision and candidate scoring",
            "execution": "result, error, verification, logs, artifacts",
            "notifications": "webhook delivery history",
            "audit": "ordered audit events",
            "raw": "raw task/workflow objects"
        }
    })
}

fn tool_registry() -> Vec<Value> {
    tool_registry_base()
        .into_iter()
        .map(enrich_tool_contract)
        .collect()
}

fn tool_registry_base() -> Vec<Value> {
    vec![
        json!({
            "id": "job.reduce",
            "name": "Job Reduce 汇总",
            "summary": "Hub 内置 reducer，用于把 sharded job 的多个 shard 结果汇总为最终 Job result。",
            "category": "runtime",
            "payload_type": "job_reduce",
            "capability": "job_reduce",
            "labels": ["job", "reduce"],
            "risk": "low",
            "requires_policy": false,
            "input_schema": {
                "type": "object",
                "required": ["type", "job_id", "shards"],
                "properties": {
                    "type": { "const": "job_reduce" },
                    "job_id": { "type": "string" },
                    "reduce": { "type": "object" },
                    "shards": { "type": "array" }
                }
            },
            "output_schema": {
                "type": "object",
                "properties": {
                    "type": { "const": "job_reduce_result" },
                    "summary": { "type": "object" },
                    "reducer_result": {},
                    "shards": { "type": "array" },
                    "artifacts": { "type": "array" }
                }
            },
            "examples": [{ "type": "job_reduce", "job_id": "job_xxx", "reduce": { "type": "summary" }, "shards": [] }]
        }),
        json!({
            "id": "http.request",
            "name": "HTTP 请求",
            "summary": "由 Worker 发起 HTTP/HTTPS 请求并结构化回写状态码、响应头、响应体和耗时。",
            "category": "network",
            "payload_type": "http_request",
            "capability": "http",
            "labels": ["compute", "http_request"],
            "risk": "medium",
            "requires_policy": true,
            "input_schema": {
                "type": "object",
                "required": ["type", "method", "url"],
                "properties": {
                    "type": { "const": "http_request" },
                    "method": { "type": "string", "enum": ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD"] },
                    "url": { "type": "string", "format": "uri" },
                    "headers": { "type": "array", "items": { "type": "array", "prefixItems": [{ "type": "string" }, { "type": "string" }] } },
                    "body": { "type": ["object", "array", "string", "number", "boolean", "null"] },
                    "timeout_seconds": { "type": "integer", "minimum": 1, "maximum": 300, "default": 30 },
                    "max_response_bytes": { "type": "integer", "minimum": 1, "maximum": 10485760, "default": 65536 }
                }
            },
            "output_schema": {
                "type": "object",
                "properties": {
                    "type": { "const": "http_response" },
                    "status_code": { "type": "integer" },
                    "headers": { "type": "array" },
                    "body": {},
                    "duration_ms": { "type": "integer" }
                }
            },
            "examples": [{ "type": "http_request", "method": "GET", "url": "https://httpbin.org/get", "headers": [], "body": null, "timeout_seconds": 30, "max_response_bytes": 65536 }]
        }),
        json!({
            "id": "command.run",
            "name": "执行主机命令",
            "summary": "在被调度节点上执行 allowlist 内的系统命令，回写 exit_code/stdout/stderr。",
            "category": "compute",
            "payload_type": "command",
            "capability": "command",
            "labels": ["compute", "command"],
            "risk": "high",
            "requires_policy": true,
            "input_schema": {
                "type": "object",
                "required": ["type", "program"],
                "properties": {
                    "type": { "const": "command" },
                    "program": { "type": "string" },
                    "args": { "type": "array", "items": { "type": "string" }, "default": [] },
                    "working_dir": { "type": ["string", "null"] },
                    "timeout_seconds": { "type": "integer", "minimum": 1, "maximum": 3600, "default": 30 }
                }
            },
            "output_schema": command_output_schema(),
            "examples": [{ "type": "command", "program": "hostname", "args": [], "working_dir": null, "timeout_seconds": 30 }]
        }),
        json!({
            "id": "file.read",
            "name": "读取文件",
            "summary": "读取节点本地文本或二进制文件内容，适合 AI 收集配置、日志和产物。",
            "category": "filesystem",
            "payload_type": "file",
            "capability": "file",
            "labels": ["compute", "file"],
            "risk": "medium",
            "requires_policy": true,
            "input_schema": file_schema("read"),
            "output_schema": file_output_schema(),
            "examples": [{ "type": "file", "operation": "read", "path": "/tmp/agentgrid.txt", "max_bytes": 65536 }]
        }),
        json!({
            "id": "file.write",
            "name": "写入文件",
            "summary": "向节点本地路径写入文本内容，可用于生成脚本、配置和临时输入文件。",
            "category": "filesystem",
            "payload_type": "file",
            "capability": "file",
            "labels": ["compute", "file"],
            "risk": "high",
            "requires_policy": true,
            "input_schema": file_schema("write"),
            "output_schema": file_output_schema(),
            "examples": [{ "type": "file", "operation": "write", "path": "/tmp/agentgrid.txt", "content": "hello", "append": false, "create_dirs": true }]
        }),
        json!({
            "id": "file.list",
            "name": "列出目录",
            "summary": "列出节点本地目录内容，支持限制条数和递归开关。",
            "category": "filesystem",
            "payload_type": "file",
            "capability": "file",
            "labels": ["compute", "file"],
            "risk": "medium",
            "requires_policy": true,
            "input_schema": file_schema("list"),
            "output_schema": file_output_schema(),
            "examples": [{ "type": "file", "operation": "list", "path": "/tmp", "recursive": false, "max_entries": 200 }]
        }),
        json!({
            "id": "git.status",
            "name": "Git 状态检查",
            "summary": "在节点本地仓库执行 git status，用于代码类工作流前置检查。",
            "category": "source_control",
            "payload_type": "git",
            "capability": "git",
            "labels": ["compute", "git"],
            "risk": "low",
            "requires_policy": true,
            "input_schema": git_schema("status"),
            "output_schema": command_output_schema(),
            "examples": [{ "type": "git", "operation": "status", "repo_dir": "/srv/project" }]
        }),
        json!({
            "id": "git.clone",
            "name": "Git 克隆仓库",
            "summary": "在节点上克隆远端仓库到指定目录。",
            "category": "source_control",
            "payload_type": "git",
            "capability": "git",
            "labels": ["compute", "git"],
            "risk": "medium",
            "requires_policy": true,
            "input_schema": git_schema("clone"),
            "output_schema": command_output_schema(),
            "examples": [{ "type": "git", "operation": "clone", "repo": "https://github.com/example/repo.git", "dest": "/tmp/repo", "branch": null, "depth": 1 }]
        }),
        json!({
            "id": "docker.run",
            "name": "运行容器",
            "summary": "在支持 Docker 的节点上启动一次性容器命令。",
            "category": "container",
            "payload_type": "docker",
            "capability": "docker",
            "labels": ["compute", "docker"],
            "risk": "high",
            "requires_policy": true,
            "input_schema": {
                "type": "object",
                "required": ["type", "operation", "image"],
                "properties": {
                    "type": { "const": "docker" },
                    "operation": { "const": "run" },
                    "image": { "type": "string" },
                    "args": { "type": "array", "items": { "type": "string" }, "default": [] },
                    "timeout_seconds": { "type": "integer", "minimum": 1, "maximum": 7200, "default": 60 }
                }
            },
            "output_schema": command_output_schema(),
            "examples": [{ "type": "docker", "operation": "run", "image": "alpine:latest", "args": ["echo", "hello"], "timeout_seconds": 60 }]
        }),
        json!({
            "id": "browser.fetch",
            "name": "浏览器抓取",
            "summary": "用浏览器能力打开页面并提取标题、文本或指定选择器内容。",
            "category": "browser",
            "payload_type": "browser",
            "capability": "browser",
            "labels": ["compute", "browser"],
            "risk": "medium",
            "requires_policy": true,
            "input_schema": {
                "type": "object",
                "required": ["type", "operation", "url"],
                "properties": {
                    "type": { "const": "browser" },
                    "operation": { "const": "fetch" },
                    "url": { "type": "string", "format": "uri" },
                    "selector": { "type": ["string", "null"] },
                    "timeout_seconds": { "type": "integer", "minimum": 1, "maximum": 300, "default": 30 },
                    "max_response_bytes": { "type": "integer", "minimum": 1, "maximum": 10485760, "default": 65536 }
                }
            },
            "output_schema": {
                "type": "object",
                "properties": {
                    "type": { "const": "browser_result" },
                    "status_code": { "type": "integer" },
                    "title": { "type": "string" },
                    "text": { "type": "string" },
                    "duration_ms": { "type": "integer" }
                }
            },
            "examples": [{ "type": "browser", "operation": "fetch", "url": "https://example.com", "selector": "body", "timeout_seconds": 30, "max_response_bytes": 65536 }]
        }),
        json!({
            "id": "desktop.screenshot",
            "name": "交互桌面截图",
            "summary": "在 Windows Desktop Helper 节点上截取当前登录用户的真实桌面。",
            "category": "desktop",
            "payload_type": "desktop",
            "capability": "desktop",
            "labels": ["compute", "desktop"],
            "risk": "high",
            "requires_policy": false,
            "input_schema": {
                "type": "object",
                "required": ["type", "operation"],
                "properties": {
                    "type": { "const": "desktop" },
                    "operation": { "const": "screenshot" },
                    "path": { "type": ["string", "null"] },
                    "timeout_seconds": { "type": "integer", "minimum": 1, "maximum": 300, "default": 30 }
                }
            },
            "output_schema": {
                "type": "object",
                "properties": {
                    "type": { "const": "desktop_result" },
                    "operation": { "type": "string" },
                    "path": { "type": ["string", "null"] },
                    "width": { "type": ["integer", "null"] },
                    "height": { "type": ["integer", "null"] },
                    "duration_ms": { "type": "integer" }
                }
            },
            "examples": [{ "type": "desktop", "operation": "screenshot", "path": "C:\\\\Users\\\\Public\\\\Pictures\\\\agentgrid-screen.png", "timeout_seconds": 30 }]
        }),
        json!({
            "id": "desktop.click",
            "name": "交互桌面点击",
            "summary": "在 Windows Desktop Helper 节点上移动鼠标并点击屏幕坐标。",
            "category": "desktop",
            "payload_type": "desktop",
            "capability": "desktop",
            "labels": ["compute", "desktop"],
            "risk": "high",
            "requires_policy": false,
            "input_schema": {
                "type": "object",
                "required": ["type", "operation", "x", "y"],
                "properties": {
                    "type": { "const": "desktop" },
                    "operation": { "const": "click" },
                    "x": { "type": "integer", "minimum": 0 },
                    "y": { "type": "integer", "minimum": 0 },
                    "button": { "type": "string", "enum": ["left", "right", "middle"], "default": "left" },
                    "timeout_seconds": { "type": "integer", "minimum": 1, "maximum": 60, "default": 10 }
                }
            },
            "output_schema": {
                "type": "object",
                "properties": {
                    "type": { "const": "desktop_result" },
                    "operation": { "const": "click" },
                    "message": { "type": "string" },
                    "duration_ms": { "type": "integer" }
                }
            },
            "examples": [{ "type": "desktop", "operation": "click", "x": 100, "y": 100, "button": "left", "timeout_seconds": 10 }]
        }),
        json!({
            "id": "desktop.type_text",
            "name": "交互桌面输入文本",
            "summary": "向 Windows Desktop Helper 当前前台窗口输入文本。",
            "category": "desktop",
            "payload_type": "desktop",
            "capability": "desktop",
            "labels": ["compute", "desktop"],
            "risk": "high",
            "requires_policy": false,
            "input_schema": {
                "type": "object",
                "required": ["type", "operation", "text"],
                "properties": {
                    "type": { "const": "desktop" },
                    "operation": { "const": "type_text" },
                    "text": { "type": "string" },
                    "interval_ms": { "type": ["integer", "null"], "minimum": 0, "maximum": 5000 },
                    "timeout_seconds": { "type": "integer", "minimum": 1, "maximum": 300, "default": 30 }
                }
            },
            "output_schema": {
                "type": "object",
                "properties": {
                    "type": { "const": "desktop_result" },
                    "operation": { "const": "type_text" },
                    "message": { "type": "string" },
                    "duration_ms": { "type": "integer" }
                }
            },
            "examples": [{ "type": "desktop", "operation": "type_text", "text": "hello from AgentGrid", "interval_ms": 0, "timeout_seconds": 30 }]
        }),
        json!({
            "id": "desktop.key",
            "name": "交互桌面按键",
            "summary": "向 Windows Desktop Helper 当前前台窗口发送快捷键或单个按键。",
            "category": "desktop",
            "payload_type": "desktop",
            "capability": "desktop",
            "labels": ["compute", "desktop"],
            "risk": "high",
            "requires_policy": false,
            "input_schema": {
                "type": "object",
                "required": ["type", "operation", "key"],
                "properties": {
                    "type": { "const": "desktop" },
                    "operation": { "const": "key" },
                    "key": { "type": "string" },
                    "modifiers": { "type": "array", "items": { "type": "string", "enum": ["ctrl", "alt", "shift"] }, "default": [] },
                    "timeout_seconds": { "type": "integer", "minimum": 1, "maximum": 60, "default": 10 }
                }
            },
            "output_schema": {
                "type": "object",
                "properties": {
                    "type": { "const": "desktop_result" },
                    "operation": { "const": "key" },
                    "message": { "type": "string" },
                    "duration_ms": { "type": "integer" }
                }
            },
            "examples": [{ "type": "desktop", "operation": "key", "key": "ESC", "modifiers": [], "timeout_seconds": 10 }]
        }),
        json!({
            "id": "session.run",
            "name": "长命令 Session",
            "summary": "运行长时间命令并支持停止控制；实时交互终端由 terminal 通道承担。",
            "category": "session",
            "payload_type": "session",
            "capability": "session",
            "labels": ["compute", "session"],
            "risk": "high",
            "requires_policy": true,
            "input_schema": {
                "type": "object",
                "required": ["type", "operation", "program"],
                "properties": {
                    "type": { "const": "session" },
                    "operation": { "const": "run" },
                    "session_id": { "type": ["string", "null"] },
                    "program": { "type": "string" },
                    "args": { "type": "array", "items": { "type": "string" }, "default": [] },
                    "working_dir": { "type": ["string", "null"] },
                    "timeout_seconds": { "type": "integer", "minimum": 1, "maximum": 86400, "default": 600 }
                }
            },
            "output_schema": command_output_schema(),
            "examples": [{ "type": "session", "operation": "run", "session_id": null, "program": "sh", "args": ["-lc", "sleep 5 && echo done"], "working_dir": null, "timeout_seconds": 600 }]
        }),
        json!({
            "id": "agentmessage.send",
            "name": "发送 AgentMessage",
            "summary": "通过 AgentGrid 员工消息标准向一个或多个 AI 员工发送结构化消息。",
            "category": "collaboration",
            "payload_type": "agent_message",
            "capability": "agentmessage",
            "labels": ["compute", "agentmessage"],
            "risk": "low",
            "requires_policy": false,
            "input_schema": {
                "type": "object",
                "required": ["type", "from", "to", "message_type", "subject", "summary"],
                "properties": {
                    "type": { "const": "agent_message" },
                    "from": { "type": "string" },
                    "to": { "type": "array", "items": { "type": "string" } },
                    "message_type": { "type": "string" },
                    "subject": { "type": "string" },
                    "summary": { "type": "string" },
                    "payload": { "type": "object", "default": {} }
                }
            },
            "output_schema": {
                "type": "object",
                "properties": {
                    "type": { "const": "agent_message_result" },
                    "delivered": { "type": "boolean" },
                    "message_id": { "type": ["string", "null"] },
                    "summary": { "type": "string" },
                    "duration_ms": { "type": "integer" }
                }
            },
            "examples": [{ "type": "agent_message", "from": "workflow-engine", "to": ["architect-agent"], "message_type": "notice", "subject": "完成", "summary": "任务已完成。", "payload": {} }]
        }),
        json!({
            "id": "plugin.run",
            "name": "Worker 插件执行",
            "summary": "调用节点本地 AGENTGRID_PLUGIN_DIR 中的插件可执行文件，使用 JSON stdin/stdout 交换数据。",
            "category": "extension",
            "payload_type": "plugin",
            "capability": "plugin",
            "labels": ["compute", "plugin"],
            "risk": "high",
            "requires_policy": true,
            "input_schema": {
                "type": "object",
                "required": ["type", "plugin_id", "action"],
                "properties": {
                    "type": { "const": "plugin" },
                    "plugin_id": { "type": "string" },
                    "action": { "type": "string", "default": "run" },
                    "input": { "type": "object", "default": {} },
                    "timeout_seconds": { "type": "integer", "minimum": 1, "maximum": 3600, "default": 60 }
                }
            },
            "output_schema": {
                "type": "object",
                "properties": {
                    "type": { "const": "plugin_result" },
                    "plugin_id": { "type": "string" },
                    "action": { "type": "string" },
                    "output": {},
                    "duration_ms": { "type": "integer" }
                }
            },
            "examples": [{ "type": "plugin", "plugin_id": "hello-plugin", "action": "run", "input": { "name": "AgentGrid" }, "timeout_seconds": 60 }]
        }),
    ]
}

fn enrich_tool_contract(mut tool: Value) -> Value {
    let tool_id = tool
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let default_verify = tool
        .get("default_verify")
        .cloned()
        .unwrap_or_else(|| default_verify_for_tool(&tool_id));
    let standard_outputs = tool
        .get("standard_outputs")
        .cloned()
        .unwrap_or_else(|| standard_outputs_for_tool(&tool_id));
    let payload_type = tool.get("payload_type").cloned().unwrap_or(Value::Null);
    let labels = tool.get("labels").cloned().unwrap_or_else(|| json!([]));
    let input_schema = tool
        .get("input_schema")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let output_schema = tool
        .get("output_schema")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let risk = tool.get("risk").cloned().unwrap_or_else(|| json!("medium"));
    let requires_policy = tool
        .get("requires_policy")
        .cloned()
        .unwrap_or_else(|| json!(true));
    let example_payload = tool
        .get("examples")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .cloned()
        .unwrap_or_else(|| json!({}));
    if let Some(map) = tool.as_object_mut() {
        map.insert("contract_version".to_string(), json!("agentgrid.tool/v1"));
        map.insert(
            "tool_contract".to_string(),
            json!({
                "api_version": "agentgrid.tool/v1",
                "kind": "ToolContract",
                "id": tool_id,
                "payload_type": payload_type,
                "labels": labels,
                "input_schema": input_schema,
                "output_schema": output_schema,
                "default_verify": default_verify.clone(),
                "risk": risk,
                "requires_policy": requires_policy,
                "standard_outputs": standard_outputs.clone(),
                "agent_runtime_submit_example": {
                    "tool_id": tool_id,
                    "payload": example_payload,
                    "verify": default_verify
                }
            }),
        );
        map.insert("default_verify".to_string(), default_verify);
        map.insert("standard_outputs".to_string(), standard_outputs);
    }
    tool
}

fn default_verify_for_tool(tool_id: &str) -> Value {
    match tool_id {
        "http.request" => json!({ "presets": ["http.status_2xx"] }),
        "command.run" | "git.status" | "git.clone" | "docker.run" | "session.run" => {
            json!({ "presets": ["command.exit_zero"] })
        }
        "file.read" | "file.list" => {
            json!({ "rules": [{ "path": "result.type", "op": "exists", "description": "文件任务必须回写结果类型" }] })
        }
        "file.write" => {
            json!({ "rules": [{ "path": "result.type", "op": "exists", "description": "文件写入必须回写结果类型" }] })
        }
        "browser.fetch" => json!({ "presets": ["browser.has_text"] }),
        "agentmessage.send" => json!({ "presets": ["agentmessage.delivered"] }),
        "plugin.run" => {
            json!({ "rules": [{ "path": "result.output", "op": "exists", "description": "插件必须输出 JSON 结果" }] })
        }
        _ => json!({ "rules": [{ "path": "result.type", "op": "exists" }] }),
    }
}

fn standard_outputs_for_tool(tool_id: &str) -> Value {
    match tool_id {
        "job.reduce" => json!(["summary", "reducer_result", "shards", "artifacts"]),
        "http.request" => json!([
            "status_code",
            "headers",
            "body",
            "duration_ms",
            "verification"
        ]),
        "command.run" | "git.status" | "git.clone" | "docker.run" | "session.run" => {
            json!([
                "exit_code",
                "stdout",
                "stderr",
                "duration_ms",
                "verification"
            ])
        }
        "file.read" | "file.write" | "file.list" => {
            json!([
                "operation",
                "path",
                "content",
                "entries",
                "bytes",
                "duration_ms",
                "verification"
            ])
        }
        "browser.fetch" => json!([
            "status_code",
            "title",
            "text",
            "duration_ms",
            "verification"
        ]),
        "agentmessage.send" => json!([
            "delivered",
            "message_id",
            "summary",
            "duration_ms",
            "verification"
        ]),
        "plugin.run" => json!([
            "plugin_id",
            "action",
            "output",
            "duration_ms",
            "verification"
        ]),
        _ => json!(["structured_result", "duration_ms", "verification"]),
    }
}

fn supports_partition_for_tool(tool_id: &str) -> bool {
    matches!(
        tool_id,
        "http.request"
            | "command.run"
            | "file.read"
            | "file.write"
            | "file.list"
            | "git.status"
            | "docker.run"
            | "browser.fetch"
            | "plugin.run"
    ) || is_dynamic_tool_id(tool_id)
}

fn recommended_reduce_for_tool(tool_id: &str) -> &'static str {
    match tool_id {
        "command.run" | "session.run" => "stdout_concat",
        "http.request" | "file.read" | "file.list" | "browser.fetch" | "plugin.run" => "json_array",
        _ => "summary",
    }
}

fn capability_job_example(tool_id: &str, reduce: &str) -> Value {
    let payload = match tool_id {
        "http.request" => json!({
            "type": "http_request",
            "method": "GET",
            "url": "${partition.items[0]}",
            "headers": [],
            "body": null,
            "timeout_seconds": 30,
            "max_response_bytes": 65536
        }),
        "command.run" => json!({
            "type": "command",
            "program": "echo",
            "args": ["${partition.items[0]}", "shard-${shard.index}"],
            "timeout_seconds": 30
        }),
        _ => json!({
            "type": tool_id,
            "input": "${partition.items[0]}"
        }),
    };
    json!({
        "title": format!("{tool_id} batch job"),
        "tool_id": tool_id,
        "payload": payload,
        "placement": { "os": "linux" },
        "strategy": {
            "type": "sharded",
            "shard_count": 2,
            "max_parallelism": 2,
            "payload_mode": "inject_shard"
        },
        "partition": {
            "type": "items",
            "items": ["item-a", "item-b"]
        },
        "reduce": {
            "type": reduce
        },
        "retry_policy": {
            "max_attempts": 3,
            "on_node_lost": "reschedule",
            "on_process_failed": "reschedule_if_idempotent"
        },
        "checkpoint_policy": {
            "enabled": true,
            "mode": "worker_reported"
        },
        "created_by": "ai-client"
    })
}

fn agent_runtime_submit_schema() -> Value {
    json!({
        "type": "object",
        "required": ["tool_id", "payload"],
        "properties": {
            "tool_id": { "type": "string", "description": "ToolContract id, for example command.run" },
            "payload": { "type": "object", "description": "Payload matching selected tool.input_schema" },
            "title": { "type": "string" },
            "summary": { "type": "string" },
            "created_by": { "type": "string", "default": "agent-runtime" },
            "owner": { "type": "string", "default": "worker-agent" },
            "priority": { "type": "string", "enum": ["p0", "urgent", "high", "normal", "p2", "low"], "default": "normal" },
            "node_id": { "type": "string" },
            "os": { "type": "string" },
            "group": { "type": "string" },
            "prefer_node_id": { "type": "string" },
            "avoid_node_id": { "type": "string" },
            "correlation_id": { "type": "string" },
            "verify": { "type": "object", "description": "Optional override for result verification rules" }
        }
    })
}

fn agent_runtime_result_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "task_id": { "type": "string" },
            "state": { "type": "string" },
            "leased_by_node_id": { "type": ["string", "null"] },
            "result": { "type": ["object", "null"] },
            "error": { "type": ["object", "null"] },
            "verification": { "path": "result.verification" },
            "events": { "type": "array" },
            "logs": { "type": "array" },
            "artifacts": { "type": "array" }
        }
    })
}

fn agent_runtime_examples() -> Value {
    json!([
        {
            "title": "Run hostname with verification",
            "request": {
                "tool_id": "command.run",
                "title": "hostname",
                "payload": { "type": "command", "program": "hostname", "args": [], "working_dir": null, "timeout_seconds": 30 },
                "verify": { "presets": ["command.exit_zero"] }
            }
        },
        {
            "title": "HTTP smoke test",
            "request": {
                "tool_id": "http.request",
                "title": "GET health endpoint",
                "payload": { "type": "http_request", "method": "GET", "url": "http://chenqi.tminos.com:20080/agentgrid/api/health", "headers": [], "body": null, "timeout_seconds": 30, "max_response_bytes": 65536 }
            }
        }
    ])
}

struct WebhookDeliveryResult {
    ok: bool,
    status_code: Option<i64>,
    error: Option<String>,
}

struct WebhookRecord {
    url: String,
    secret: Option<String>,
}

fn deliver_webhook(url: &str, payload: &Value, secret: Option<&str>) -> WebhookDeliveryResult {
    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return WebhookDeliveryResult {
                ok: false,
                status_code: None,
                error: Some(error.to_string()),
            }
        }
    };
    let body = match serde_json::to_vec(payload) {
        Ok(body) => body,
        Err(error) => {
            return WebhookDeliveryResult {
                ok: false,
                status_code: None,
                error: Some(error.to_string()),
            }
        }
    };
    let mut request = client
        .post(url)
        .header("content-type", "application/json")
        .header(
            "x-agentgrid-event",
            payload
                .get("event_type")
                .and_then(Value::as_str)
                .unwrap_or(""),
        )
        .header(
            "x-agentgrid-delivery",
            payload
                .get("delivery_id")
                .and_then(Value::as_str)
                .unwrap_or(""),
        );
    if let Some(secret) = secret.filter(|value| !value.is_empty()) {
        request = request.header("x-agentgrid-signature", webhook_signature(secret, &body));
    }
    match request.body(body).send() {
        Ok(response) => {
            let status = response.status();
            WebhookDeliveryResult {
                ok: status.is_success(),
                status_code: Some(status.as_u16() as i64),
                error: if status.is_success() {
                    None
                } else {
                    Some(format!("webhook returned HTTP {status}"))
                },
            }
        }
        Err(error) => WebhookDeliveryResult {
            ok: false,
            status_code: None,
            error: Some(error.to_string()),
        },
    }
}

fn webhook_signature(secret: &str, body: &[u8]) -> String {
    type HmacSha256 = Hmac<Sha256>;
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts secrets of any size");
    mac.update(body);
    format!("sha256={:x}", mac.finalize().into_bytes())
}

fn nodes_for_tool(tool: &Value, nodes: &[Value]) -> Vec<Value> {
    let capability = tool.get("capability").and_then(Value::as_str).unwrap_or("");
    nodes
        .iter()
        .filter(|node| node.pointer("/status/state").and_then(Value::as_str) == Some("online"))
        .filter(|node| {
            node.pointer("/spec/capabilities")
                .and_then(Value::as_array)
                .map(|items| items.iter().any(|item| item.as_str() == Some(capability)))
                .unwrap_or(false)
        })
        .map(|node| {
            json!({
                "id": node.pointer("/metadata/id").and_then(Value::as_str),
                "name": node.pointer("/metadata/name").and_then(Value::as_str),
                "state": node.pointer("/status/state").and_then(Value::as_str),
                "os": node.pointer("/spec/os").and_then(Value::as_str),
                "arch": node.pointer("/spec/arch").and_then(Value::as_str),
                "address": node.pointer("/spec/address").and_then(Value::as_str),
                "cpu_cores": node.pointer("/spec/cpu_cores").and_then(Value::as_i64),
                "memory_mb": node.pointer("/spec/memory_mb").and_then(Value::as_i64),
                "running_jobs": node.pointer("/status/running_jobs").and_then(Value::as_i64),
                "max_concurrent_jobs": node.pointer("/spec/max_concurrent_jobs").and_then(Value::as_i64),
                "worker_version": node.pointer("/spec/worker_version").and_then(Value::as_str),
                "worker_target": node.pointer("/spec/worker_target").and_then(Value::as_str),
                "support_basis": "node_heartbeat_capabilities",
                "verification_status": "declared_unverified"
            })
        })
        .collect()
}

fn node_tool_probe_payload(tool: &Value) -> Option<Value> {
    let configured = tool.pointer("/spec/probe/payload").cloned()?;
    let tool_id = tool.pointer("/spec/tool_id").and_then(Value::as_str)?;
    let executor = tool.pointer("/spec/executor").and_then(Value::as_str)?;
    let mut payload = configured;
    let Some(map) = payload.as_object_mut() else {
        return None;
    };
    map.entry("type".to_string())
        .or_insert_with(|| json!(tool_id));
    map.insert("tool_id".to_string(), json!(tool_id));
    map.insert("executor".to_string(), json!(executor));
    Some(payload)
}

fn node_tool_probe_verify(tool: &Value) -> Value {
    tool.pointer("/spec/probe/verify")
        .cloned()
        .or_else(|| tool.pointer("/spec/default_verify").cloned())
        .filter(|value| !value.is_null())
        .unwrap_or_else(|| {
            json!({
                "rules": [
                    {
                        "path": "result.type",
                        "op": "exists",
                        "description": "节点工具 Probe 必须回写结构化结果"
                    }
                ]
            })
        })
}

fn node_tool_probe_labels(tool: &Value, node_id: &str, tool_id: &str) -> Vec<String> {
    let mut labels = tool
        .pointer("/spec/labels")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_else(|| {
            vec![
                json!("compute"),
                json!("plugin"),
                json!(format!("tool:{tool_id}")),
            ]
        })
        .into_iter()
        .filter_map(|item| item.as_str().map(ToString::to_string))
        .collect::<Vec<_>>();
    ensure_label(&mut labels, "compute");
    ensure_label(&mut labels, "plugin");
    ensure_label(&mut labels, &format!("tool:{tool_id}"));
    ensure_label(&mut labels, &format!("node:{node_id}"));
    ensure_label(&mut labels, &format!("probe:{tool_id}"));
    ensure_label(&mut labels, "node_tool_probe");
    labels
}

fn initial_node_tool_probe_state(data: &Value) -> &'static str {
    if data.get("probe").is_some() {
        "declared_unverified"
    } else {
        "unsupported"
    }
}

fn initial_node_tool_next_probe_at(data: &Value) -> Option<String> {
    data.get("probe").map(|_| now())
}

fn normalize_node_tool_metadata(data: &Value, tool_id: &str, node_id: &str) -> Value {
    let mut metadata = data.get("metadata").cloned().unwrap_or_else(|| json!({}));
    ensure_object(&mut metadata);
    let executor = data
        .get("executor")
        .and_then(Value::as_str)
        .unwrap_or("plugin");
    if executor.starts_with("plugin:") {
        let plugin_id = executor.trim_start_matches("plugin:");
        let manifest = metadata
            .get("manifest")
            .cloned()
            .or_else(|| data.get("manifest").cloned())
            .or_else(|| data.get("plugin_manifest").cloned())
            .unwrap_or_else(|| {
                json!({
                    "plugin_id": plugin_id,
                    "name": data.get("name").and_then(Value::as_str).unwrap_or(tool_id),
                    "version": data.get("version").and_then(Value::as_str).unwrap_or("0.1.0"),
                    "author": "unknown",
                    "platforms": ["node-local"],
                    "entrypoint": plugin_id,
                    "tools": [tool_id],
                    "dependencies": []
                })
            });
        if let Some(map) = metadata.as_object_mut() {
            map.insert("runtime".to_string(), json!("agentgrid.plugin-runtime/v1"));
            map.insert("node_id".to_string(), json!(node_id));
            map.insert("plugin_id".to_string(), json!(plugin_id));
            map.insert("manifest".to_string(), manifest);
            map.entry("risk".to_string())
                .or_insert_with(|| json!("high"));
        }
    }
    metadata
}

fn aggregate_probe_state(items: &[Value]) -> Value {
    let mut counts = HashMap::<String, usize>::new();
    for item in items {
        let state = item
            .pointer("/status/probe_state")
            .and_then(Value::as_str)
            .unwrap_or("declared_unverified");
        *counts.entry(state.to_string()).or_default() += 1;
    }
    let state = if counts.get("failed").copied().unwrap_or(0) > 0 {
        "failed"
    } else if counts.get("pending").copied().unwrap_or(0) > 0 {
        "pending"
    } else if counts.get("expired").copied().unwrap_or(0) > 0 {
        "expired"
    } else if counts.get("verified").copied().unwrap_or(0) > 0 {
        "verified"
    } else if counts.get("unsupported").copied().unwrap_or(0) == items.len() {
        "unsupported"
    } else {
        "declared_unverified"
    };
    json!({ "state": state, "counts": counts })
}

fn probe_payload_for_tool(tool_id: &str) -> Option<Value> {
    match tool_id {
        "http.request" => Some(json!({
            "type": "http_request",
            "method": "GET",
            "url": "http://chenqi.tminos.com:20080/agentgrid/api/health",
            "headers": [],
            "body": null,
            "timeout_seconds": 15,
            "max_response_bytes": 65536
        })),
        "command.run" => Some(json!({
            "type": "command",
            "program": "hostname",
            "args": [],
            "working_dir": null,
            "timeout_seconds": 30
        })),
        "file.read" => Some(json!({
            "type": "file",
            "operation": "read",
            "path": "/tmp/agentgrid-probe.txt",
            "max_bytes": 65536
        })),
        "file.write" => Some(json!({
            "type": "file",
            "operation": "write",
            "path": "/tmp/agentgrid-probe.txt",
            "content": "agentgrid probe\n",
            "append": false,
            "create_dirs": true
        })),
        "file.list" => Some(json!({
            "type": "file",
            "operation": "list",
            "path": "/tmp",
            "recursive": false,
            "max_entries": 20
        })),
        "git.status" | "git.clone" => Some(json!({
            "type": "command",
            "program": "git",
            "args": ["--version"],
            "working_dir": null,
            "timeout_seconds": 30
        })),
        "docker.run" => Some(json!({
            "type": "docker",
            "operation": "run",
            "image": "alpine:latest",
            "args": ["echo", "agentgrid-probe"],
            "timeout_seconds": 60
        })),
        "browser.fetch" => Some(json!({
            "type": "browser",
            "operation": "fetch",
            "url": "https://example.com",
            "selector": "body",
            "timeout_seconds": 30,
            "max_response_bytes": 65536
        })),
        "session.run" => Some(json!({
            "type": "session",
            "operation": "run",
            "session_id": null,
            "program": "hostname",
            "args": [],
            "working_dir": null,
            "timeout_seconds": 30
        })),
        "agentmessage.send" => Some(json!({
            "type": "agent_message",
            "from": "tool-probe-engine",
            "to": ["architect-agent"],
            "message_type": "tool.probe",
            "subject": "Tool Probe",
            "summary": "AgentGrid Tool Probe 验证 AgentMessage 发送能力。",
            "payload": {}
        })),
        "plugin.run" => None,
        _ => None,
    }
}

fn probe_labels_for_tool(tool_id: &str, node_id: &str) -> Vec<String> {
    let mut labels = tool_registry()
        .into_iter()
        .find(|tool| tool.get("id").and_then(Value::as_str) == Some(tool_id))
        .and_then(|tool| {
            tool.get("labels")
                .and_then(Value::as_array)
                .cloned()
                .map(|items| {
                    items
                        .into_iter()
                        .filter_map(|item| item.as_str().map(ToString::to_string))
                        .collect::<Vec<_>>()
                })
        })
        .unwrap_or_else(|| vec!["compute".to_string()]);
    if matches!(tool_id, "git.status" | "git.clone") {
        labels.retain(|label| label != "git");
        if !labels.iter().any(|label| label == "command") {
            labels.push("command".to_string());
        }
    }
    labels.push(format!("node:{node_id}"));
    labels.push(format!("probe:{tool_id}"));
    labels
}

fn command_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "type": { "const": "command_result" },
            "exit_code": { "type": "integer" },
            "stdout": { "type": "string" },
            "stderr": { "type": "string" },
            "duration_ms": { "type": "integer" }
        }
    })
}

fn file_schema(operation: &str) -> Value {
    match operation {
        "read" => json!({
            "type": "object",
            "required": ["type", "operation", "path"],
            "properties": {
                "type": { "const": "file" },
                "operation": { "const": "read" },
                "path": { "type": "string" },
                "max_bytes": { "type": ["integer", "null"], "minimum": 1 }
            }
        }),
        "write" => json!({
            "type": "object",
            "required": ["type", "operation", "path", "content"],
            "properties": {
                "type": { "const": "file" },
                "operation": { "const": "write" },
                "path": { "type": "string" },
                "content": { "type": "string" },
                "append": { "type": "boolean", "default": false },
                "create_dirs": { "type": "boolean", "default": true }
            }
        }),
        _ => json!({
            "type": "object",
            "required": ["type", "operation", "path"],
            "properties": {
                "type": { "const": "file" },
                "operation": { "const": "list" },
                "path": { "type": "string" },
                "recursive": { "type": "boolean", "default": false },
                "max_entries": { "type": ["integer", "null"], "minimum": 1 }
            }
        }),
    }
}

fn file_output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "type": { "const": "file_result" },
            "operation": { "type": "string" },
            "path": { "type": "string" },
            "content": { "type": ["string", "null"] },
            "entries": { "type": "array" },
            "bytes": { "type": "integer" },
            "duration_ms": { "type": "integer" }
        }
    })
}

fn git_schema(operation: &str) -> Value {
    match operation {
        "clone" => json!({
            "type": "object",
            "required": ["type", "operation", "repo", "dest"],
            "properties": {
                "type": { "const": "git" },
                "operation": { "const": "clone" },
                "repo": { "type": "string" },
                "dest": { "type": "string" },
                "branch": { "type": ["string", "null"] },
                "depth": { "type": ["integer", "null"], "minimum": 1 }
            }
        }),
        _ => json!({
            "type": "object",
            "required": ["type", "operation", "repo_dir"],
            "properties": {
                "type": { "const": "git" },
                "operation": { "const": operation },
                "repo_dir": { "type": "string" }
            }
        }),
    }
}

fn extract_result_text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn apply_result_verification(task: &Value, result: Value, checked_at: &str) -> Value {
    let Some(verify) = task
        .pointer("/spec/verify")
        .filter(|value| !value.is_null())
    else {
        return result;
    };
    let verification = verify_result(verify, &result, checked_at);
    match result {
        Value::Object(mut map) => {
            map.insert("verification".to_string(), verification);
            Value::Object(map)
        }
        other => json!({
            "type": "wrapped_result",
            "value": other,
            "verification": verification
        }),
    }
}

fn verify_result(verify: &Value, result: &Value, checked_at: &str) -> Value {
    let mut rules = Vec::new();
    for preset in verification_presets(verify) {
        rules.extend(preset_verification_rules(&preset));
    }
    if let Some(items) = verify.get("rules").and_then(Value::as_array) {
        rules.extend(items.iter().cloned());
    }
    if rules.is_empty() {
        return json!({
            "state": "skipped",
            "passed": true,
            "checked_at": checked_at,
            "summary": "未配置验证规则",
            "rules": []
        });
    }

    let context = json!({ "result": result });
    let evaluated = rules
        .iter()
        .map(|rule| evaluate_verification_rule(rule, &context))
        .collect::<Vec<_>>();
    let passed = evaluated
        .iter()
        .all(|item| item.get("ok").and_then(Value::as_bool).unwrap_or(false));
    let passed_count = evaluated
        .iter()
        .filter(|item| item.get("ok").and_then(Value::as_bool).unwrap_or(false))
        .count();
    json!({
        "state": if passed { "passed" } else { "failed" },
        "passed": passed,
        "checked_at": checked_at,
        "summary": format!("{passed_count}/{} 条规则通过", evaluated.len()),
        "rules": evaluated
    })
}

fn verification_presets(verify: &Value) -> Vec<String> {
    let mut presets = Vec::new();
    if let Some(preset) = verify.get("preset").and_then(Value::as_str) {
        presets.push(preset.to_string());
    }
    if let Some(items) = verify.get("presets").and_then(Value::as_array) {
        presets.extend(
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string),
        );
    }
    presets
}

fn preset_verification_rules(preset: &str) -> Vec<Value> {
    match preset {
        "command.exit_zero" => vec![json!({
            "path": "result.exit_code",
            "op": "eq",
            "value": 0,
            "description": "命令退出码必须为 0"
        })],
        "http.status_2xx" => vec![
            json!({
                "path": "result.status_code",
                "op": "gte",
                "value": 200,
                "description": "HTTP 状态码必须大于等于 200"
            }),
            json!({
                "path": "result.status_code",
                "op": "lt",
                "value": 300,
                "description": "HTTP 状态码必须小于 300"
            }),
        ],
        "file.non_empty" => vec![json!({
            "path": "result.bytes",
            "op": "gt",
            "value": 0,
            "description": "文件结果不能为空"
        })],
        "browser.has_text" => vec![json!({
            "path": "result.text",
            "op": "neq",
            "value": "",
            "description": "浏览器结果必须包含文本"
        })],
        "agentmessage.delivered" => vec![json!({
            "path": "result.delivered",
            "op": "eq",
            "value": true,
            "description": "AgentMessage 必须投递成功"
        })],
        _ => vec![json!({
            "path": "result.type",
            "op": "exists",
            "description": format!("未知 preset：{preset}，降级检查 result.type 是否存在")
        })],
    }
}

fn evaluate_verification_rule(rule: &Value, context: &Value) -> Value {
    let path = rule
        .get("path")
        .and_then(Value::as_str)
        .unwrap_or("result")
        .trim();
    let op = rule
        .get("op")
        .and_then(Value::as_str)
        .unwrap_or("exists")
        .trim();
    let expected = rule
        .get("value")
        .or_else(|| rule.get("expected"))
        .cloned()
        .unwrap_or(Value::Null);
    let actual = resolve_dot_path(context, path);
    let (ok, message) = match op {
        "exists" => (actual.is_some(), "字段必须存在".to_string()),
        "eq" => (
            actual.map(|value| value == &expected).unwrap_or(false),
            "必须等于期望值".to_string(),
        ),
        "neq" => (
            actual.map(|value| value != &expected).unwrap_or(false),
            "必须不等于期望值".to_string(),
        ),
        "contains" => (
            actual
                .map(value_to_match_text)
                .map(|value| value.contains(expected.as_str().unwrap_or("")))
                .unwrap_or(false),
            "文本必须包含期望内容".to_string(),
        ),
        "not_contains" => (
            actual
                .map(value_to_match_text)
                .map(|value| !value.contains(expected.as_str().unwrap_or("")))
                .unwrap_or(false),
            "文本不能包含期望内容".to_string(),
        ),
        "gt" | "gte" | "lt" | "lte" => (
            compare_numbers(actual, expected.as_f64(), op),
            "数值必须满足比较条件".to_string(),
        ),
        "regex" => (
            actual
                .map(value_to_match_text)
                .zip(expected.as_str())
                .and_then(|(actual, pattern)| {
                    Regex::new(pattern)
                        .ok()
                        .map(|regex| regex.is_match(&actual))
                })
                .unwrap_or(false),
            "文本必须匹配正则表达式".to_string(),
        ),
        "json_type" => (
            actual
                .map(json_type_name)
                .zip(expected.as_str())
                .map(|(actual_type, expected_type)| actual_type == expected_type)
                .unwrap_or(false),
            "JSON 类型必须匹配".to_string(),
        ),
        other => (false, format!("不支持的验证操作：{other}")),
    };
    json!({
        "ok": ok,
        "path": path,
        "op": op,
        "expected": expected,
        "actual": actual.cloned().unwrap_or(Value::Null),
        "description": rule.get("description").and_then(Value::as_str).unwrap_or(""),
        "message": message
    })
}

fn resolve_dot_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;
    for part in path.trim_start_matches("$.").split('.') {
        if part.is_empty() {
            continue;
        }
        match current {
            Value::Object(map) => current = map.get(part)?,
            Value::Array(items) => current = items.get(part.parse::<usize>().ok()?)?,
            _ => return None,
        }
    }
    Some(current)
}

fn compare_numbers(actual: Option<&Value>, expected: Option<f64>, op: &str) -> bool {
    let Some(actual) = actual.and_then(Value::as_f64) else {
        return false;
    };
    let Some(expected) = expected else {
        return false;
    };
    match op {
        "gt" => actual > expected,
        "gte" => actual >= expected,
        "lt" => actual < expected,
        "lte" => actual <= expected,
        _ => false,
    }
}

fn value_to_match_text(value: &Value) -> String {
    value
        .as_str()
        .map(ToString::to_string)
        .unwrap_or_else(|| serde_json::to_string(value).unwrap_or_default())
}

fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn file_name_from_path(path: &str) -> &str {
    path.rsplit(['/', '\\'])
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or("artifact.bin")
}

fn task_to_job(task: &Value) -> anyhow::Result<Job> {
    let task_id = task
        .pointer("/metadata/id")
        .and_then(Value::as_str)
        .unwrap_or("unknown-task")
        .to_string();
    let labels = task
        .pointer("/spec/labels")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| item.as_str().map(ToString::to_string))
        .collect::<Vec<_>>();
    let payload = parse_job_payload_from_task(task)?;
    let label_tool_id = labels
        .iter()
        .find_map(|label| label.strip_prefix("tool:").map(ToString::to_string));
    let capability = match payload {
        JobPayload::HttpRequest(_) => "http",
        JobPayload::Command(_) => "command",
        JobPayload::File(_) => "file",
        JobPayload::Git(_) => "git",
        JobPayload::Docker(_) => "docker",
        JobPayload::Browser(_) => "browser",
        JobPayload::Desktop(_) => "desktop",
        JobPayload::Session(_) => "session",
        JobPayload::AgentMessage(_) => "agentmessage",
        JobPayload::Plugin(_) => "plugin",
        JobPayload::Custom { .. } => "plugin",
    };
    let created_at = task
        .pointer("/metadata/created_at")
        .and_then(Value::as_str)
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);
    Ok(Job {
        api_version: agentgrid_protocol::AGENTGRID_V1.to_string(),
        kind: "Job".to_string(),
        metadata: JobMetadata {
            id: task_id,
            project_id: PROJECT_ID.to_string(),
            client_id: task
                .pointer("/metadata/created_by")
                .and_then(Value::as_str)
                .unwrap_or("hub")
                .to_string(),
            created_at,
        },
        spec: JobSpec {
            name: task
                .pointer("/spec/title")
                .and_then(Value::as_str)
                .unwrap_or("AgentGrid task")
                .to_string(),
            priority: parse_priority(
                task.pointer("/spec/priority")
                    .and_then(Value::as_str)
                    .unwrap_or("normal"),
            ),
            requirements: JobRequirements {
                tags: labels
                    .iter()
                    .filter_map(|label| label.strip_prefix("tag:").map(ToString::to_string))
                    .collect(),
                capabilities: vec![capability.to_string()],
                os: labels
                    .iter()
                    .filter_map(|label| label.strip_prefix("os:").map(ToString::to_string))
                    .collect(),
                groups: labels
                    .iter()
                    .filter_map(|label| label.strip_prefix("group:").map(ToString::to_string))
                    .collect(),
                preferred_node_ids: labels
                    .iter()
                    .filter_map(|label| {
                        label
                            .strip_prefix("prefer:")
                            .or_else(|| label.strip_prefix("preferred:"))
                            .map(ToString::to_string)
                    })
                    .collect(),
                avoid_node_ids: labels
                    .iter()
                    .filter_map(|label| label.strip_prefix("avoid:").map(ToString::to_string))
                    .collect(),
                exclusive_key: labels
                    .iter()
                    .find_map(|label| label.strip_prefix("exclusive:").map(ToString::to_string)),
                node_id: labels
                    .iter()
                    .find_map(|label| label.strip_prefix("node:").map(ToString::to_string)),
                ..JobRequirements::default()
            },
            payload: match payload {
                JobPayload::Custom { name, mut value } => {
                    if value.get("tool_id").is_none() {
                        if let Some(tool_id) = label_tool_id {
                            if let Some(map) = value.as_object_mut() {
                                map.insert("tool_id".to_string(), json!(tool_id));
                            }
                        }
                    }
                    JobPayload::Custom { name, value }
                }
                other => other,
            },
        },
        status: JobStatus {
            state: JobState::Queued,
            assigned_node_id: None,
            started_at: None,
            finished_at: None,
            result: None,
        },
    })
}

fn task_type(task: &Value) -> String {
    task.pointer("/spec/labels")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().filter_map(Value::as_str).find(|label| {
                matches!(
                    *label,
                    "http_request"
                        | "command"
                        | "file"
                        | "git"
                        | "docker"
                        | "browser"
                        | "session"
                        | "agentmessage"
                        | "plugin"
                )
            })
        })
        .unwrap_or("unknown")
        .to_string()
}

fn tool_id_for_job(job: &Job) -> Option<String> {
    if let JobPayload::Custom { name, value } = &job.spec.payload {
        return value
            .get("tool_id")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .or_else(|| Some(name.clone()));
    }
    match &job.spec.payload {
        JobPayload::HttpRequest(_) => Some("http.request".to_string()),
        JobPayload::Command(_) => Some("command.run".to_string()),
        JobPayload::File(payload) => match payload {
            FilePayload::Read { .. } | FilePayload::Download { .. } => {
                Some("file.read".to_string())
            }
            FilePayload::Write { .. } | FilePayload::Upload { .. } => {
                Some("file.write".to_string())
            }
            FilePayload::List { .. } => Some("file.list".to_string()),
        },
        JobPayload::Git(payload) => match payload {
            GitPayload::Status { .. } => Some("git.status".to_string()),
            GitPayload::Clone { .. } => Some("git.clone".to_string()),
            GitPayload::Pull { .. } => Some("git.status".to_string()),
            GitPayload::Checkout { .. } => Some("git.status".to_string()),
        },
        JobPayload::Docker(_) => Some("docker.run".to_string()),
        JobPayload::Browser(_) => Some("browser.fetch".to_string()),
        JobPayload::Desktop(payload) => match payload {
            agentgrid_protocol::DesktopPayload::Screenshot { .. } => {
                Some("desktop.screenshot".to_string())
            }
            agentgrid_protocol::DesktopPayload::Click { .. } => Some("desktop.click".to_string()),
            agentgrid_protocol::DesktopPayload::TypeText { .. } => {
                Some("desktop.type_text".to_string())
            }
            agentgrid_protocol::DesktopPayload::Key { .. } => Some("desktop.key".to_string()),
        },
        JobPayload::Session(_) => Some("session.run".to_string()),
        JobPayload::AgentMessage(_) => Some("agentmessage.send".to_string()),
        JobPayload::Plugin(payload) => Some(format!("plugin.{}", payload.plugin_id)),
        JobPayload::Custom { .. } => None,
    }
}

fn tool_id_from_task_labels(task: &Value) -> Option<String> {
    task.pointer("/spec/labels")
        .and_then(Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .find_map(|label| label.strip_prefix("tool:").map(ToString::to_string))
        })
}

fn tool_id_for_task(task: &Value) -> Option<String> {
    tool_id_from_task_labels(task)
        .or_else(|| task_to_job(task).ok().and_then(|job| tool_id_for_job(&job)))
}

fn tool_id_for_task_id(store: &Store, task_id: &str) -> Option<String> {
    store
        .get_task(task_id)
        .ok()
        .flatten()
        .and_then(|task| tool_id_for_task(&task))
}

fn is_dynamic_tool_id(tool_id: &str) -> bool {
    !tool_registry()
        .iter()
        .any(|tool| tool.get("id").and_then(Value::as_str) == Some(tool_id))
}

fn dynamic_runtime_payload(tool_id: &str, tool: &Value, payload: Value) -> anyhow::Result<Value> {
    let executor = tool
        .get("executor")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("dynamic tool executor missing: {tool_id}"))?;
    let mut payload = payload;
    let map = payload
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("dynamic tool payload must be a JSON object"))?;
    if !map.contains_key("type") {
        map.insert("type".to_string(), json!(tool_id));
    }
    map.insert("tool_id".to_string(), json!(tool_id));
    map.insert("executor".to_string(), json!(executor));
    Ok(payload)
}

fn default_trust_evaluation(tool_id: Option<String>) -> TrustEvaluation {
    default_trust_evaluation_with_risk(tool_id, "medium".to_string())
}

fn default_trust_evaluation_with_risk(tool_id: Option<String>, risk: String) -> TrustEvaluation {
    let reason = tool_id
        .as_ref()
        .map(|tool_id| format!("{tool_id} declared but not runtime verified"))
        .unwrap_or_else(|| "no tool registry mapping available".to_string());
    let state = "declared_unverified".to_string();
    let risk_multiplier = risk_multiplier(&risk, &state);
    TrustEvaluation {
        tool_id,
        state,
        support_basis: "node_heartbeat_capabilities".to_string(),
        multiplier: trust_multiplier("declared_unverified"),
        risk,
        risk_multiplier,
        reason,
    }
}

fn trust_multiplier(state: &str) -> f64 {
    match state {
        "verified" => 0.72,
        "pending" => 1.12,
        "declared_unverified" => 1.35,
        "expired" => 1.45,
        "unsupported" => 1.75,
        "failed" => 999.0,
        _ => 1.35,
    }
}

fn risk_multiplier(risk: &str, probe_state: &str) -> f64 {
    match (risk, probe_state) {
        ("high", "verified") => 0.95,
        ("high", "pending") => 1.35,
        ("high", "declared_unverified" | "expired") => 1.75,
        ("high", "failed" | "unsupported") => 999.0,
        ("medium", "verified") => 0.98,
        ("medium", "declared_unverified" | "expired") => 1.20,
        ("low", "verified") => 0.90,
        ("low", _) => 1.0,
        _ => 1.05,
    }
}

fn graph_multiplier_for_job(job: &Job, trust: &TrustEvaluation) -> f64 {
    let mut multiplier = 1.0;
    if matches!(job.spec.payload, JobPayload::Desktop(_)) {
        multiplier *= if trust.state == "verified" {
            0.85
        } else {
            1.35
        };
    }
    if matches!(
        job.spec.payload,
        JobPayload::Plugin(_) | JobPayload::Custom { .. }
    ) {
        multiplier *= if trust.state == "verified" {
            0.90
        } else {
            1.45
        };
    }
    if trust
        .tool_id
        .as_deref()
        .map(is_dynamic_tool_id)
        .unwrap_or(false)
    {
        multiplier *= if trust.state == "verified" {
            0.90
        } else {
            1.30
        };
    }
    multiplier
}

fn json_node_to_protocol(value: &Value) -> anyhow::Result<Node> {
    let status = match value
        .pointer("/status/state")
        .and_then(Value::as_str)
        .unwrap_or("offline")
    {
        "online" => NodeState::Online,
        "unknown" => NodeState::Unknown,
        "busy" => NodeState::Busy,
        "draining" => NodeState::Draining,
        "disabled" => NodeState::Disabled,
        "untrusted" => NodeState::Untrusted,
        _ => NodeState::Offline,
    };
    Ok(Node {
        id: value
            .pointer("/metadata/id")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        name: value
            .pointer("/metadata/name")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        os: value
            .pointer("/spec/os")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        arch: value
            .pointer("/spec/arch")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        tags: value
            .pointer("/spec/tags")
            .map(string_array_from_value)
            .unwrap_or_default(),
        capabilities: value
            .pointer("/spec/capabilities")
            .map(string_array_from_value)
            .unwrap_or_default(),
        cpu_cores: value
            .pointer("/spec/cpu_cores")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .max(0) as u16,
        memory_mb: value
            .pointer("/spec/memory_mb")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .max(0) as u64,
        cpu_usage_percent: value
            .pointer("/spec/cpu_usage_percent")
            .and_then(Value::as_f64)
            .unwrap_or(0.0) as f32,
        memory_used_mb: value
            .pointer("/spec/memory_used_mb")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .max(0) as u64,
        disk_total_mb: value
            .pointer("/spec/disk_total_mb")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .max(0) as u64,
        disk_free_mb: value
            .pointer("/spec/disk_free_mb")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .max(0) as u64,
        running_jobs: value
            .pointer("/status/running_jobs")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .max(0) as u16,
        max_concurrent_jobs: value
            .pointer("/spec/max_concurrent_jobs")
            .and_then(Value::as_i64)
            .unwrap_or(1)
            .max(1) as u16,
        weight: value
            .pointer("/spec/weight")
            .and_then(Value::as_f64)
            .unwrap_or(1.0),
        groups: value
            .pointer("/spec/groups")
            .map(string_array_from_value)
            .unwrap_or_default(),
        success_rate: node_success_rate(
            value
                .pointer("/status/success_count")
                .and_then(Value::as_i64)
                .unwrap_or(0),
            value
                .pointer("/status/failure_count")
                .and_then(Value::as_i64)
                .unwrap_or(0),
        ),
        status,
        last_heartbeat_at: value
            .pointer("/status/last_heartbeat_at")
            .and_then(Value::as_str)
            .and_then(|raw| chrono::DateTime::parse_from_rfc3339(raw).ok())
            .map(|time| time.with_timezone(&Utc))
            .unwrap_or_else(|| Utc::now() - chrono::Duration::days(365)),
    })
}

fn evaluate_node_for_job(node_value: &Value, node: &Node, job: &Job) -> Value {
    let mut reasons = Vec::new();
    if node.status != NodeState::Online {
        reasons.push(format!("节点状态是 {:?}，不能接任务", node.status));
    }
    let load_score = score_node(node);
    if load_score >= HIGH_LOAD_SCORE_LIMIT {
        reasons.push(format!(
            "资源评分 {:.2} 超过高负载阈值 {:.2}",
            load_score, HIGH_LOAD_SCORE_LIMIT
        ));
    }
    if let Some(required) = job.spec.requirements.node_id.as_ref() {
        if required != &node.id {
            reasons.push(format!("任务指定节点 {required}，当前节点不匹配"));
        }
    }
    for os in &job.spec.requirements.os {
        if !os_value_matches(&node.os, os) {
            reasons.push(format!("操作系统不匹配，需要 {os}，当前 {}", node.os));
        }
    }
    for group in &job.spec.requirements.groups {
        if !node.groups.contains(group) {
            reasons.push(format!("缺少节点分组 {group}"));
        }
    }
    for tag in &job.spec.requirements.tags {
        if !node.tags.contains(tag) {
            reasons.push(format!("缺少节点标签 {tag}"));
        }
    }
    for capability in &job.spec.requirements.capabilities {
        if !node.capabilities.contains(capability) {
            reasons.push(format!("缺少执行能力 {capability}"));
        }
    }
    if let Some(tool_id) = tool_id_for_job(job) {
        if is_dynamic_tool_id(&tool_id) {
            let registered = node_value
                .pointer("/spec/node_tools")
                .and_then(Value::as_array)
                .map(|items| {
                    items.iter().any(|item| {
                        item.pointer("/spec/tool_id").and_then(Value::as_str)
                            == Some(tool_id.as_str())
                            && item.pointer("/status/state").and_then(Value::as_str)
                                == Some("available")
                    })
                })
                .unwrap_or(false);
            if !registered {
                reasons.push(format!("节点未注册动态工具 {tool_id}"));
            }
        }
    }
    if job.spec.requirements.avoid_node_ids.contains(&node.id) {
        reasons.push("任务明确要求避开该节点".to_string());
    }
    if node.running_jobs >= node.max_concurrent_jobs {
        reasons.push("并发槽位已满".to_string());
    }
    let eligible = reasons.is_empty();
    json!({
        "node_id": node.id,
        "node_name": node.name,
        "eligible": eligible,
        "score": load_score,
        "available_slots": node.max_concurrent_jobs.saturating_sub(node.running_jobs),
        "state": node_value.pointer("/status/state").cloned().unwrap_or_else(|| json!("offline")),
        "os": node.os,
        "arch": node.arch,
        "worker": {
            "version": node_value.pointer("/spec/worker_version").cloned().unwrap_or(Value::Null),
            "target": node_value.pointer("/spec/worker_target").cloned().unwrap_or(Value::Null),
            "glibc_version": node_value.pointer("/spec/glibc_version").cloned().unwrap_or(Value::Null),
            "auto_update_enabled": node_value.pointer("/spec/auto_update_enabled").cloned().unwrap_or(Value::Null)
        },
        "reasons": if eligible { vec!["满足任务要求，可参与调度".to_string()] } else { reasons }
    })
}

fn preview_task_value(data: &Value) -> Value {
    let now = now();
    let id = string_or(data, "id", "job_plan_preview");
    json!({
        "api_version": API_VERSION,
        "kind": "AgentTask",
        "metadata": {
            "id": id,
            "project_id": PROJECT_ID,
            "created_by": string_or(data, "created_by", "job-plan"),
            "created_at": now,
            "updated_at": now,
            "assigned_to": data.get("assigned_to").cloned().unwrap_or_else(|| json!([])),
            "workflow_id": data.get("workflow_id").cloned().unwrap_or(Value::Null),
            "workflow_node_id": data.get("workflow_node_id").cloned().unwrap_or(Value::Null),
            "job_id": data.get("job_id").cloned().unwrap_or(Value::Null),
            "job_attempt_id": data.get("job_attempt_id").cloned().unwrap_or(Value::Null),
            "job_shard_id": data.get("job_shard_id").cloned().unwrap_or(Value::Null),
            "correlation_id": data.get("correlation_id").cloned().unwrap_or(Value::Null),
            "last_message_id": Value::Null
        },
        "spec": {
            "title": string_or(data, "title", "Job plan preview"),
            "summary": string_or(data, "summary", ""),
            "owner": string_or(data, "owner", "worker-agent"),
            "priority": string_or(data, "priority", "normal"),
            "labels": data.get("labels").cloned().unwrap_or_else(|| json!([])),
            "inputs": data.get("inputs").cloned().unwrap_or_else(|| json!([])),
            "outputs": data.get("outputs").cloned().unwrap_or_else(|| json!([])),
            "acceptance_criteria": data.get("acceptance_criteria").cloned().unwrap_or_else(|| json!([])),
            "depends_on": data.get("depends_on").cloned().unwrap_or_else(|| json!([])),
            "due_at": data.get("due_at").cloned().unwrap_or(Value::Null),
            "verify": data.get("verify").cloned().unwrap_or(Value::Null)
        },
        "status": {
            "state": "assigned",
            "progress": number_or(data, "progress", 0),
            "attempts": 0,
            "leased_by_node_id": Value::Null,
            "lease_expires_at": Value::Null,
            "started_at": Value::Null,
            "completed_at": Value::Null,
            "result": Value::Null,
            "error": Value::Null,
            "control": Value::Null,
            "blocked_reason": Value::Null
        }
    })
}

fn job_reliability_contract_from_request(data: &Value) -> Value {
    let retry_policy = data
        .get("retry_policy")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let checkpoint_policy = data
        .get("checkpoint_policy")
        .cloned()
        .unwrap_or_else(|| json!({ "enabled": true }));
    let idempotency = data
        .get("idempotency")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let idempotency_key = idempotency
        .get("key")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let idempotency_mode = idempotency
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("at_least_once");
    json!({
        "delivery": if idempotency_mode == "external_exactly_once" { "external_exactly_once" } else { "at_least_once" },
        "max_attempts": retry_policy.get("max_attempts").and_then(Value::as_i64).unwrap_or(3),
        "on_node_lost": retry_policy.get("on_node_lost").and_then(Value::as_str).unwrap_or("reschedule"),
        "on_process_failed": retry_policy.get("on_process_failed").and_then(Value::as_str).unwrap_or("reschedule_if_idempotent"),
        "checkpoint_enabled": checkpoint_policy.get("enabled").and_then(Value::as_bool).unwrap_or(true),
        "checkpoint_mode": checkpoint_policy.get("mode").and_then(Value::as_str).unwrap_or("worker_reported"),
        "idempotency_key": idempotency_key,
        "idempotency_mode": idempotency_mode,
        "safe_for_retry": idempotency_key.is_some() || matches!(idempotency_mode, "idempotent" | "external_exactly_once")
    })
}

fn retry_reschedule_standard_contract() -> Value {
    json!({
        "api_version": "agentgrid.retry-reschedule/v1",
        "kind": "RetryRescheduleContract",
        "decision_inputs": [
            "retry_policy.max_attempts",
            "retry_policy.on_node_lost",
            "retry_policy.on_process_failed",
            "idempotency.key",
            "idempotency.mode",
            "checkpoint_policy.enabled",
            "current attempt count",
            "failure reason"
        ],
        "failure_reasons": {
            "node_lost": {
                "examples": ["lease_expired", "node_offline", "heartbeat_lost"],
                "default_policy": "reschedule",
                "requires_idempotency": false
            },
            "process_failed": {
                "examples": ["non_zero_exit", "worker_error", "tool_error"],
                "default_policy": "reschedule_if_idempotent",
                "requires_idempotency": true
            }
        },
        "policies": {
            "on_node_lost": ["reschedule", "fail"],
            "on_process_failed": ["reschedule_if_idempotent", "fail"]
        },
        "decision_outputs": [
            "should_reschedule",
            "reason",
            "next_attempt_number",
            "attempts_remaining",
            "safe_for_retry",
            "checkpoint_enabled"
        ],
        "guarantee": "Hub only retries when the retry policy allows it and max_attempts is not exhausted."
    })
}

fn retry_reschedule_contract_from_request(data: &Value) -> Value {
    let retry_policy = data
        .get("retry_policy")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let checkpoint_policy = data
        .get("checkpoint_policy")
        .cloned()
        .unwrap_or_else(|| json!({ "enabled": true, "mode": "worker_reported" }));
    let idempotency = data
        .get("idempotency")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let max_attempts = retry_policy
        .get("max_attempts")
        .and_then(Value::as_i64)
        .unwrap_or(3)
        .clamp(1, 20);
    let safe_for_retry = idempotency_safe_for_retry(&idempotency);
    json!({
        "api_version": "agentgrid.retry-reschedule/v1",
        "kind": "RetryReschedulePlan",
        "max_attempts": max_attempts,
        "safe_for_retry": safe_for_retry,
        "idempotency": idempotency,
        "checkpoint": {
            "enabled": checkpoint_policy.get("enabled").and_then(Value::as_bool).unwrap_or(true),
            "mode": checkpoint_policy.get("mode").and_then(Value::as_str).unwrap_or("worker_reported")
        },
        "decisions": {
            "node_lost": retry_reschedule_decision_from_parts(
                max_attempts,
                retry_policy.get("on_node_lost").and_then(Value::as_str).unwrap_or("reschedule"),
                safe_for_retry,
                checkpoint_policy.get("enabled").and_then(Value::as_bool).unwrap_or(true),
                0,
                "node_lost",
                None
            ),
            "process_failed": retry_reschedule_decision_from_parts(
                max_attempts,
                retry_policy.get("on_process_failed").and_then(Value::as_str).unwrap_or("reschedule_if_idempotent"),
                safe_for_retry,
                checkpoint_policy.get("enabled").and_then(Value::as_bool).unwrap_or(true),
                0,
                "process_failed",
                None
            )
        }
    })
}

fn retry_reschedule_decision(
    job: &Value,
    failure_reason: &str,
    attempts_so_far: i64,
    error: Option<&Value>,
) -> Value {
    let retry_policy = job
        .pointer("/spec/retry_policy")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let idempotency = job
        .pointer("/spec/idempotency")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let checkpoint_policy = job
        .pointer("/spec/checkpoint_policy")
        .cloned()
        .unwrap_or_else(|| json!({ "enabled": true, "mode": "worker_reported" }));
    let max_attempts = retry_policy
        .get("max_attempts")
        .and_then(Value::as_i64)
        .or_else(|| job.pointer("/status/max_attempts").and_then(Value::as_i64))
        .unwrap_or(3)
        .clamp(1, 20);
    let policy = match failure_reason {
        "node_lost" => retry_policy
            .get("on_node_lost")
            .and_then(Value::as_str)
            .unwrap_or("reschedule"),
        "process_failed" => retry_policy
            .get("on_process_failed")
            .and_then(Value::as_str)
            .unwrap_or("reschedule_if_idempotent"),
        _ => "fail",
    };
    retry_reschedule_decision_from_parts(
        max_attempts,
        policy,
        idempotency_safe_for_retry(&idempotency),
        checkpoint_policy
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        attempts_so_far,
        failure_reason,
        error,
    )
}

fn retry_reschedule_decision_from_parts(
    max_attempts: i64,
    policy: &str,
    safe_for_retry: bool,
    checkpoint_enabled: bool,
    attempts_so_far: i64,
    failure_reason: &str,
    error: Option<&Value>,
) -> Value {
    let attempts_remaining = (max_attempts - attempts_so_far).max(0);
    let has_capacity = attempts_so_far < max_attempts;
    let policy_allows = match (failure_reason, policy) {
        ("node_lost", "reschedule") => true,
        ("process_failed", "reschedule_if_idempotent") => safe_for_retry,
        (_, "fail") => false,
        _ => false,
    };
    let should_reschedule = has_capacity && policy_allows;
    let reason = if !has_capacity {
        "max_attempts_exhausted"
    } else if !policy_allows && failure_reason == "process_failed" && !safe_for_retry {
        "process_failed_requires_idempotency"
    } else if !policy_allows {
        "retry_policy_disallows_reschedule"
    } else {
        "retry_policy_allows_reschedule"
    };
    json!({
        "failure_reason": failure_reason,
        "policy": policy,
        "should_reschedule": should_reschedule,
        "reason": reason,
        "attempts_so_far": attempts_so_far,
        "max_attempts": max_attempts,
        "attempts_remaining": attempts_remaining,
        "next_attempt_number": if should_reschedule { attempts_so_far + 1 } else { attempts_so_far },
        "safe_for_retry": safe_for_retry,
        "checkpoint_enabled": checkpoint_enabled,
        "error_code": error.and_then(|value| value.get("code")).and_then(Value::as_str)
    })
}

fn job_execution_summary(attempts: &[Value], checkpoints: &[Value], events: &[Value]) -> Value {
    let count_attempts = |state: &str| {
        attempts
            .iter()
            .filter(|attempt| {
                attempt.pointer("/status/state").and_then(Value::as_str) == Some(state)
            })
            .count()
    };
    json!({
        "attempts": {
            "total": attempts.len(),
            "queued": count_attempts("queued"),
            "running": count_attempts("running"),
            "done": count_attempts("done"),
            "failed": count_attempts("failed"),
            "lost": count_attempts("lost")
        },
        "checkpoints": {
            "total": checkpoints.len(),
            "latest": checkpoints.first().cloned().unwrap_or(Value::Null)
        },
        "events": {
            "total": events.len(),
            "latest": events.last().cloned().unwrap_or(Value::Null)
        }
    })
}

fn job_recovery_view(job: &Value, attempts: &[Value], checkpoints: &[Value]) -> Value {
    let attempts_so_far = attempts.len() as i64;
    let latest_attempt = attempts.last();
    let latest_state = latest_attempt
        .and_then(|attempt| attempt.pointer("/status/state").and_then(Value::as_str))
        .unwrap_or_else(|| {
            job.pointer("/status/state")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        });
    let latest_error = latest_attempt
        .and_then(|attempt| attempt.pointer("/status/error"))
        .or_else(|| job.pointer("/status/error"));
    let failure_reason = match latest_state {
        "lost" => "node_lost",
        "failed" => "process_failed",
        _ => "none",
    };
    let retry_decision = if failure_reason == "none" {
        Value::Null
    } else {
        retry_reschedule_decision(job, failure_reason, attempts_so_far, latest_error)
    };
    json!({
        "state": latest_state,
        "failure_reason": failure_reason,
        "latest_attempt_id": latest_attempt.and_then(|attempt| attempt.pointer("/metadata/id").and_then(Value::as_str)),
        "latest_task_id": latest_attempt.and_then(|attempt| attempt.pointer("/metadata/task_id").and_then(Value::as_str)),
        "latest_checkpoint": checkpoints.first().cloned().unwrap_or(Value::Null),
        "retry_decision": retry_decision,
        "contract": retry_reschedule_contract_from_request(&json!({
            "retry_policy": job.pointer("/spec/retry_policy").cloned().unwrap_or_else(|| json!({})),
            "checkpoint_policy": job.pointer("/spec/checkpoint_policy").cloned().unwrap_or_else(|| json!({})),
            "idempotency": job.pointer("/spec/idempotency").cloned().unwrap_or_else(|| json!({}))
        }))
    })
}

fn job_execution_timeline(
    job: &Value,
    attempts: &[Value],
    checkpoints: &[Value],
    events: &[Value],
) -> Vec<Value> {
    let mut items = Vec::new();
    if let Some(created_at) = job.pointer("/metadata/created_at").and_then(Value::as_str) {
        items.push(json!({
            "time": created_at,
            "type": "job.created",
            "subject_id": job.pointer("/metadata/id").and_then(Value::as_str),
            "summary": "Job created"
        }));
    }
    for attempt in attempts {
        if let Some(created_at) = attempt
            .pointer("/metadata/created_at")
            .and_then(Value::as_str)
        {
            items.push(json!({
                "time": created_at,
                "type": "job.attempt.created",
                "attempt_id": attempt.pointer("/metadata/id").and_then(Value::as_str),
                "task_id": attempt.pointer("/metadata/task_id").and_then(Value::as_str),
                "state": attempt.pointer("/status/state").and_then(Value::as_str),
                "summary": format!(
                    "Attempt {} created",
                    attempt.pointer("/spec/attempt_number").and_then(Value::as_i64).unwrap_or(0)
                )
            }));
        }
        if let Some(started_at) = attempt
            .pointer("/status/started_at")
            .and_then(Value::as_str)
        {
            items.push(json!({
                "time": started_at,
                "type": "job.attempt.started",
                "attempt_id": attempt.pointer("/metadata/id").and_then(Value::as_str),
                "task_id": attempt.pointer("/metadata/task_id").and_then(Value::as_str),
                "node_id": attempt.pointer("/status/node_id").and_then(Value::as_str),
                "summary": "Attempt started"
            }));
        }
        if let Some(completed_at) = attempt
            .pointer("/status/completed_at")
            .and_then(Value::as_str)
        {
            items.push(json!({
                "time": completed_at,
                "type": "job.attempt.finished",
                "attempt_id": attempt.pointer("/metadata/id").and_then(Value::as_str),
                "task_id": attempt.pointer("/metadata/task_id").and_then(Value::as_str),
                "state": attempt.pointer("/status/state").and_then(Value::as_str),
                "summary": "Attempt finished"
            }));
        }
    }
    for checkpoint in checkpoints {
        if let Some(created_at) = checkpoint
            .pointer("/metadata/created_at")
            .and_then(Value::as_str)
        {
            items.push(json!({
                "time": created_at,
                "type": "job.checkpoint",
                "checkpoint_id": checkpoint.pointer("/metadata/id").and_then(Value::as_str),
                "attempt_id": checkpoint.pointer("/metadata/attempt_id").and_then(Value::as_str),
                "task_id": checkpoint.pointer("/metadata/task_id").and_then(Value::as_str),
                "node_id": checkpoint.pointer("/metadata/node_id").and_then(Value::as_str),
                "progress": checkpoint.pointer("/status/progress").and_then(Value::as_i64),
                "sequence": checkpoint.pointer("/status/sequence").and_then(Value::as_i64),
                "summary": "Checkpoint reported"
            }));
        }
    }
    for event in events {
        if let Some(created_at) = event
            .pointer("/metadata/created_at")
            .and_then(Value::as_str)
        {
            items.push(json!({
                "time": created_at,
                "type": event.pointer("/spec/type").and_then(Value::as_str).unwrap_or("audit.event"),
                "event_id": event.pointer("/metadata/id").and_then(Value::as_str),
                "actor": event.pointer("/spec/actor").and_then(Value::as_str),
                "summary": event.pointer("/spec/summary").and_then(Value::as_str).unwrap_or("")
            }));
        }
    }
    items.sort_by(|left, right| {
        left.get("time")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(right.get("time").and_then(Value::as_str).unwrap_or(""))
    });
    items
}

fn idempotency_safe_for_retry(idempotency: &Value) -> bool {
    let has_key = idempotency
        .get("key")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some();
    let mode = idempotency
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("at_least_once");
    has_key || matches!(mode, "idempotent" | "external_exactly_once")
}

fn job_plan_warnings(data: &Value, eligible_nodes: &[Value]) -> Vec<Value> {
    let mut warnings = Vec::new();
    let reliability = job_reliability_contract_from_request(data);
    if !reliability
        .get("safe_for_retry")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && reliability
            .get("max_attempts")
            .and_then(Value::as_i64)
            .unwrap_or(1)
            > 1
    {
        warnings.push(json!({
            "code": "retry_without_idempotency_key",
            "severity": "medium",
            "message": "Job 允许多次尝试，但没有 idempotency.key；外部副作用工具可能重复执行。"
        }));
    }
    if eligible_nodes.is_empty() {
        warnings.push(json!({
            "code": "no_eligible_node",
            "severity": "high",
            "message": "当前没有可执行该 Job 的在线节点。"
        }));
    }
    if data.pointer("/strategy/type").and_then(Value::as_str) == Some("sharded")
        && data
            .pointer("/checkpoint_policy/enabled")
            .and_then(Value::as_bool)
            == Some(false)
    {
        warnings.push(json!({
            "code": "sharded_without_checkpoint",
            "severity": "low",
            "message": "分片 Job 未启用 checkpoint；节点丢失后只能从分片开头重跑。"
        }));
    }
    warnings
}

fn reconcile_action(
    journal_event: &str,
    hub_state: &str,
    leased_by_node_id: Option<&str>,
    node_id: &str,
) -> &'static str {
    match (journal_event, hub_state) {
        ("started" | "leased", "in_progress") if leased_by_node_id == Some(node_id) => {
            "worker_should_confirm_running_or_finish"
        }
        ("started" | "leased", "missing") => "hub_missing_task",
        ("started" | "leased", "assigned" | "todo") => "hub_does_not_know_worker_started",
        ("reported", "done" | "failed" | "stopped" | "cancelled") => "none",
        ("report_failed", _) => "worker_report_failed",
        (_, "done" | "failed" | "stopped" | "cancelled") => "none",
        _ => "none",
    }
}

fn reconcile_hub_snapshot(task: Option<&Value>) -> Value {
    let Some(task) = task else {
        return json!({
            "exists": false
        });
    };
    json!({
        "exists": true,
        "state": task.pointer("/status/state").and_then(Value::as_str),
        "progress": task.pointer("/status/progress").and_then(Value::as_i64),
        "leased_by_node_id": task.pointer("/status/leased_by_node_id").and_then(Value::as_str),
        "lease_expires_at": task.pointer("/status/lease_expires_at").and_then(Value::as_str),
        "started_at": task.pointer("/status/started_at").and_then(Value::as_str),
        "completed_at": task.pointer("/status/completed_at").and_then(Value::as_str),
        "job_id": task.pointer("/metadata/job_id").and_then(Value::as_str),
        "job_attempt_id": task.pointer("/metadata/job_attempt_id").and_then(Value::as_str),
        "job_shard_id": task.pointer("/metadata/job_shard_id").and_then(Value::as_str)
    })
}

fn reconcile_recovery(action: &str, task: Option<&Value>, journal: &Value, node_id: &str) -> Value {
    let task_id = journal.get("task_id").and_then(Value::as_str).unwrap_or("");
    let is_job_attempt = task
        .and_then(|task| {
            task.pointer("/metadata/job_attempt_id")
                .and_then(Value::as_str)
        })
        .or_else(|| journal.get("job_attempt_id").and_then(Value::as_str))
        .filter(|value| !value.is_empty())
        .is_some();
    let lease_expires_at = task.and_then(|task| {
        task.pointer("/status/lease_expires_at")
            .and_then(Value::as_str)
    });
    let lease_expired = lease_expires_at
        .and_then(|raw| chrono::DateTime::parse_from_rfc3339(raw).ok())
        .map(|value| value.with_timezone(&Utc) < Utc::now())
        .unwrap_or(false);
    let (severity, recommendation, automation, retryable, operator_action) = match action {
        "none" => (
            "info",
            "No recovery required.",
            "none",
            false,
            "No action.",
        ),
        "worker_should_confirm_running_or_finish" if lease_expired => (
            "warning",
            "Lease is expired while Worker journal says the task started. Hub recovery loop may reschedule Job Attempts; non-Job tasks should be inspected before manual retry.",
            if is_job_attempt {
                "eligible_for_job_reschedule"
            } else {
                "manual_inspection_required"
            },
            is_job_attempt,
            "Check Worker process and task logs; if the task is not running, let Hub reschedule or create a replacement task.",
        ),
        "worker_should_confirm_running_or_finish" => (
            "info",
            "Worker journal and Hub both show the task is running. Worker should keep renewing the lease and eventually report completion or failure.",
            "wait_for_worker_report",
            false,
            "Monitor lease renewal and task logs.",
        ),
        "hub_missing_task" => (
            "critical",
            "Worker journal references a task that Hub no longer stores. Treat the journal entry as orphaned execution evidence.",
            "manual_audit_required",
            false,
            "Inspect Worker journal and external side effects before deleting local evidence.",
        ),
        "hub_does_not_know_worker_started" => (
            "warning",
            "Worker journal says execution started, but Hub still shows the task as queued or assigned.",
            "manual_state_repair_required",
            false,
            "Confirm whether a Worker is still running this task. Avoid assigning duplicate side-effecting work.",
        ),
        "worker_report_failed" => (
            "warning",
            "Worker could not report execution result to Hub. The task result may exist only in Worker logs or artifacts.",
            if is_job_attempt {
                "recover_result_or_reschedule"
            } else {
                "manual_result_recovery"
            },
            is_job_attempt,
            "Recover result from Worker journal/logs. If no result can be recovered and the work is idempotent, resubmit or let Job retry policy handle it.",
        ),
        _ => (
            "info",
            "No standard recovery rule matched this journal and Hub state combination.",
            "none",
            false,
            "No action.",
        ),
    };
    json!({
        "severity": severity,
        "recommendation": recommendation,
        "automation": automation,
        "retryable": retryable,
        "operator_action": operator_action,
        "task_id": task_id,
        "node_id": node_id,
        "is_job_attempt": is_job_attempt,
        "lease_expired": lease_expired,
        "lease_expires_at": lease_expires_at
    })
}

fn reconcile_summary(items: &[Value]) -> Value {
    let mut by_action = serde_json::Map::new();
    let mut by_severity = serde_json::Map::new();
    for item in items {
        let action = item.get("action").and_then(Value::as_str).unwrap_or("none");
        let severity = item
            .get("severity")
            .and_then(Value::as_str)
            .unwrap_or("info");
        increment_json_count(&mut by_action, action);
        increment_json_count(&mut by_severity, severity);
    }
    let needs_attention = items
        .iter()
        .filter(|item| item.get("action").and_then(Value::as_str) != Some("none"))
        .count();
    json!({
        "total": items.len(),
        "needs_attention": needs_attention,
        "by_action": by_action,
        "by_severity": by_severity
    })
}

fn increment_json_count(map: &mut serde_json::Map<String, Value>, key: &str) {
    let next = map.get(key).and_then(Value::as_u64).unwrap_or(0) + 1;
    map.insert(key.to_string(), json!(next));
}

fn os_value_matches(reported: &str, required: &str) -> bool {
    let reported = reported.to_ascii_lowercase();
    let required = required.to_ascii_lowercase();
    reported.contains(&required)
        || (required == "linux"
            && ["ubuntu", "debian", "centos", "alibaba", "rocky", "rhel"]
                .iter()
                .any(|alias| reported.contains(alias)))
        || (matches!(required.as_str(), "mac" | "macos" | "darwin")
            && (reported.contains("darwin") || reported.contains("mac")))
        || (matches!(required.as_str(), "windows" | "win") && reported.contains("win"))
}

fn parse_job_payload_from_task(task: &Value) -> anyhow::Result<JobPayload> {
    let raw = task
        .pointer("/spec/inputs/0")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("task input payload missing"))?;
    let value: Value = serde_json::from_str(raw)?;
    match value.get("type").and_then(Value::as_str).unwrap_or("") {
        "http_request" => Ok(JobPayload::HttpRequest(HttpRequestPayload {
            method: value
                .get("method")
                .and_then(Value::as_str)
                .unwrap_or("GET")
                .to_string(),
            url: value
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            headers: value
                .get("headers")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| {
                            item.as_array().and_then(|pair| {
                                Some((
                                    pair.first()?.as_str()?.to_string(),
                                    pair.get(1)?.as_str()?.to_string(),
                                ))
                            })
                        })
                        .collect()
                })
                .unwrap_or_default(),
            body: value.get("body").cloned().filter(|body| !body.is_null()),
            timeout_seconds: value
                .get("timeout_seconds")
                .and_then(Value::as_u64)
                .unwrap_or(30),
            max_response_bytes: value
                .get("max_response_bytes")
                .and_then(Value::as_u64)
                .unwrap_or(65_536),
        })),
        "command" => Ok(JobPayload::Command(agentgrid_protocol::CommandPayload {
            program: value
                .get("program")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            args: value
                .get("args")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(ToString::to_string)
                        .collect()
                })
                .unwrap_or_default(),
            working_dir: value
                .get("working_dir")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            timeout_seconds: value
                .get("timeout_seconds")
                .and_then(Value::as_u64)
                .unwrap_or(30),
        })),
        "file" => Ok(JobPayload::File(parse_file_payload(&value)?)),
        "git" => Ok(JobPayload::Git(parse_git_payload(&value)?)),
        "docker" => Ok(JobPayload::Docker(parse_docker_payload(&value)?)),
        "browser" => Ok(JobPayload::Browser(parse_browser_payload(&value)?)),
        "desktop" => Ok(JobPayload::Desktop(parse_desktop_payload(&value)?)),
        "session" => Ok(JobPayload::Session(parse_session_payload(&value)?)),
        "agent_message" => Ok(JobPayload::AgentMessage(AgentMessagePayload {
            from: value
                .get("from")
                .and_then(Value::as_str)
                .unwrap_or("worker-agent")
                .to_string(),
            to: value
                .get("to")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(ToString::to_string)
                        .collect()
                })
                .unwrap_or_default(),
            message_type: value
                .get("message_type")
                .or_else(|| value.get("type_name"))
                .and_then(Value::as_str)
                .unwrap_or("broadcast.notice")
                .to_string(),
            subject: value
                .get("subject")
                .and_then(Value::as_str)
                .unwrap_or("AgentMessage")
                .to_string(),
            summary: value
                .get("summary")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            payload: value.get("payload").cloned().unwrap_or_else(|| json!({})),
        })),
        "plugin" => Ok(JobPayload::Plugin(agentgrid_protocol::PluginPayload {
            plugin_id: required_json_string(&value, "plugin_id")?,
            action: optional_json_string(&value, "action").unwrap_or_else(|| "run".to_string()),
            input: value.get("input").cloned().unwrap_or_else(|| json!({})),
            timeout_seconds: value
                .get("timeout_seconds")
                .and_then(Value::as_u64)
                .unwrap_or(60),
        })),
        other => {
            let name = value
                .get("tool_id")
                .and_then(Value::as_str)
                .unwrap_or(other)
                .to_string();
            Ok(JobPayload::Custom { name, value })
        }
    }
}

fn parse_file_payload(value: &Value) -> anyhow::Result<FilePayload> {
    match value.get("operation").and_then(Value::as_str).unwrap_or("") {
        "read" => Ok(FilePayload::Read {
            path: required_json_string(value, "path")?,
            max_bytes: value.get("max_bytes").and_then(Value::as_u64),
        }),
        "write" => Ok(FilePayload::Write {
            path: required_json_string(value, "path")?,
            content: value
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            append: value
                .get("append")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            create_dirs: value
                .get("create_dirs")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        }),
        "list" => Ok(FilePayload::List {
            path: required_json_string(value, "path")?,
            recursive: value
                .get("recursive")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            max_entries: value.get("max_entries").and_then(Value::as_u64),
        }),
        "upload" => Ok(FilePayload::Upload {
            path: required_json_string(value, "path")?,
            content_base64: required_json_string(value, "content_base64")?,
            create_dirs: value
                .get("create_dirs")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        }),
        "download" => Ok(FilePayload::Download {
            path: required_json_string(value, "path")?,
            max_bytes: value.get("max_bytes").and_then(Value::as_u64),
        }),
        other => anyhow::bail!("unsupported file operation: {other}"),
    }
}

fn parse_git_payload(value: &Value) -> anyhow::Result<GitPayload> {
    match value.get("operation").and_then(Value::as_str).unwrap_or("") {
        "clone" => Ok(GitPayload::Clone {
            repo: required_json_string(value, "repo")?,
            dest: required_json_string(value, "dest")?,
            branch: optional_json_string(value, "branch"),
            depth: value
                .get("depth")
                .and_then(Value::as_u64)
                .map(|depth| depth as u32),
        }),
        "pull" => Ok(GitPayload::Pull {
            repo_dir: required_json_string(value, "repo_dir")?,
        }),
        "status" => Ok(GitPayload::Status {
            repo_dir: required_json_string(value, "repo_dir")?,
        }),
        "checkout" => Ok(GitPayload::Checkout {
            repo_dir: required_json_string(value, "repo_dir")?,
            reference: required_json_string(value, "reference")?,
        }),
        other => anyhow::bail!("unsupported git operation: {other}"),
    }
}

fn parse_docker_payload(value: &Value) -> anyhow::Result<DockerPayload> {
    match value.get("operation").and_then(Value::as_str).unwrap_or("") {
        "ps" => Ok(DockerPayload::Ps),
        "images" => Ok(DockerPayload::Images),
        "run" => Ok(DockerPayload::Run {
            image: required_json_string(value, "image")?,
            args: array_field(value, "args"),
            timeout_seconds: value
                .get("timeout_seconds")
                .and_then(Value::as_u64)
                .unwrap_or(60),
        }),
        other => anyhow::bail!("unsupported docker operation: {other}"),
    }
}

fn parse_browser_payload(value: &Value) -> anyhow::Result<BrowserPayload> {
    match value
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("fetch")
    {
        "fetch" => Ok(BrowserPayload::Fetch {
            url: required_json_string(value, "url")?,
            selector: optional_json_string(value, "selector"),
            timeout_seconds: value
                .get("timeout_seconds")
                .and_then(Value::as_u64)
                .unwrap_or(30),
            max_response_bytes: value
                .get("max_response_bytes")
                .and_then(Value::as_u64)
                .unwrap_or(65_536),
        }),
        "automate" => Ok(BrowserPayload::Automate {
            url: required_json_string(value, "url")?,
            actions: value
                .get("actions")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
            screenshot_path: optional_json_string(value, "screenshot_path"),
            timeout_seconds: value
                .get("timeout_seconds")
                .and_then(Value::as_u64)
                .unwrap_or(60),
        }),
        other => anyhow::bail!("unsupported browser operation: {other}"),
    }
}

fn parse_desktop_payload(value: &Value) -> anyhow::Result<agentgrid_protocol::DesktopPayload> {
    match value
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("screenshot")
    {
        "screenshot" => Ok(agentgrid_protocol::DesktopPayload::Screenshot {
            path: optional_json_string(value, "path"),
            timeout_seconds: value
                .get("timeout_seconds")
                .and_then(Value::as_u64)
                .unwrap_or(30),
        }),
        "click" => Ok(agentgrid_protocol::DesktopPayload::Click {
            x: value
                .get("x")
                .and_then(Value::as_i64)
                .ok_or_else(|| anyhow::anyhow!("desktop click requires x"))? as i32,
            y: value
                .get("y")
                .and_then(Value::as_i64)
                .ok_or_else(|| anyhow::anyhow!("desktop click requires y"))? as i32,
            button: value
                .get("button")
                .and_then(Value::as_str)
                .unwrap_or("left")
                .to_string(),
            timeout_seconds: value
                .get("timeout_seconds")
                .and_then(Value::as_u64)
                .unwrap_or(10),
        }),
        "type_text" => Ok(agentgrid_protocol::DesktopPayload::TypeText {
            text: required_json_string(value, "text")?,
            interval_ms: value.get("interval_ms").and_then(Value::as_u64),
            timeout_seconds: value
                .get("timeout_seconds")
                .and_then(Value::as_u64)
                .unwrap_or(30),
        }),
        "key" => Ok(agentgrid_protocol::DesktopPayload::Key {
            key: required_json_string(value, "key")?,
            modifiers: value
                .get("modifiers")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(ToString::to_string)
                        .collect()
                })
                .unwrap_or_default(),
            timeout_seconds: value
                .get("timeout_seconds")
                .and_then(Value::as_u64)
                .unwrap_or(10),
        }),
        other => anyhow::bail!("unsupported desktop operation: {other}"),
    }
}

fn parse_session_payload(value: &Value) -> anyhow::Result<agentgrid_protocol::SessionPayload> {
    match value
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("run")
    {
        "run" => Ok(agentgrid_protocol::SessionPayload::Run {
            session_id: optional_json_string(value, "session_id"),
            program: required_json_string(value, "program")?,
            args: value
                .get("args")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(ToString::to_string)
                        .collect()
                })
                .unwrap_or_default(),
            working_dir: optional_json_string(value, "working_dir"),
            timeout_seconds: value
                .get("timeout_seconds")
                .and_then(Value::as_u64)
                .unwrap_or(300),
        }),
        other => anyhow::bail!("unsupported session operation: {other}"),
    }
}

fn parse_priority(value: &str) -> Priority {
    match value.to_ascii_lowercase().as_str() {
        "p0" => Priority::P0,
        "p1" => Priority::P1,
        "p2" => Priority::P2,
        "high" | "urgent" => Priority::High,
        "low" => Priority::Low,
        _ => Priority::Normal,
    }
}

fn required_json_string(value: &Value, key: &str) -> anyhow::Result<String> {
    optional_json_string(value, key)
        .filter(|item| !item.is_empty())
        .ok_or_else(|| anyhow::anyhow!("{key} missing"))
}

fn optional_json_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn default_security_policy() -> Value {
    json!({
        "http": {
            "allowed_domains": [],
            "blocked_ips": ["127.0.0.1", "::1", "0.0.0.0"],
            "allow_private_network": false,
            "max_response_bytes": 65536
        },
        "command": {
            "enabled": true,
            "command_allowlist": [
                "pwd", "whoami", "hostname", "uname", "date",
                "ls", "df", "du", "free", "uptime",
                "sh", "bash",
                "echo", "cat", "head", "tail", "wc",
                "git", "cargo", "node", "npm", "pnpm", "python3", "python",
                "cmd", "powershell", "pwsh"
            ],
            "max_stdout_bytes": 65536,
            "max_stderr_bytes": 65536
        },
        "secrets": {
            "allow_env": false,
            "allowed_secret_refs": []
        }
    })
}

fn default_scheduler_config() -> Value {
    json!({
        "priority_order": ["p0", "urgent", "high", "p1", "normal", "p2", "low"],
        "high_load_score_limit": HIGH_LOAD_SCORE_LIMIT,
        "weights": {
            "cpu": 0.38,
            "memory": 0.26,
            "disk": 0.12,
            "slot_pressure": 0.18,
            "success_rate": 0.2
        },
        "routing": {
            "honor_node_label": true,
            "honor_os_label": true,
            "honor_group_label": true,
            "honor_prefer_avoid": true
        },
        "lease": {
            "default_seconds": 120,
            "max_seconds": 600,
            "recover_expired_leases": true
        }
    })
}

fn default_task_templates() -> Vec<Value> {
    vec![
        json!({
            "id": "server.hostname",
            "name": "主机名检查",
            "summary": "在最优节点执行 hostname，验证节点命令执行能力。",
            "category": "server",
            "tool_id": "command.run",
            "payload": { "type": "command", "program": "hostname", "args": [], "working_dir": null, "timeout_seconds": 30 },
            "parameters": [],
            "verify": { "presets": ["command.exit_zero"] }
        }),
        json!({
            "id": "server.disk",
            "name": "磁盘空间检查",
            "summary": "检查 Linux/macOS 节点磁盘空间。",
            "category": "server",
            "tool_id": "command.run",
            "payload": { "type": "command", "program": "df", "args": ["-h"], "working_dir": null, "timeout_seconds": 30 },
            "parameters": [],
            "verify": { "presets": ["command.exit_zero"] }
        }),
        json!({
            "id": "git.status",
            "name": "Git 仓库状态",
            "summary": "检查指定仓库工作区状态。",
            "category": "source_control",
            "tool_id": "git.status",
            "payload": { "type": "git", "operation": "status", "repo_dir": "{{repo_dir}}" },
            "parameters": [{ "name": "repo_dir", "label": "仓库目录", "default": "/srv/project", "required": true }],
            "verify": { "presets": ["command.exit_zero"] }
        }),
        json!({
            "id": "http.healthcheck",
            "name": "HTTP 健康检查",
            "summary": "请求一个 HTTP 健康检查地址并验证 2xx。",
            "category": "network",
            "tool_id": "http.request",
            "payload": { "type": "http_request", "method": "GET", "url": "{{url}}", "headers": [], "body": null, "timeout_seconds": 30, "max_response_bytes": 65536 },
            "parameters": [{ "name": "url", "label": "URL", "default": "https://example.com", "required": true }],
            "verify": { "presets": ["http.status_2xx"] }
        }),
        json!({
            "id": "browser.fetch",
            "name": "网页文本抓取",
            "summary": "抓取网页标题和正文文本。",
            "category": "browser",
            "tool_id": "browser.fetch",
            "payload": { "type": "browser", "operation": "fetch", "url": "{{url}}", "selector": "{{selector}}", "timeout_seconds": 30, "max_response_bytes": 65536 },
            "parameters": [
                { "name": "url", "label": "URL", "default": "https://example.com", "required": true },
                { "name": "selector", "label": "CSS 选择器", "default": "body", "required": false }
            ],
            "verify": { "presets": ["browser.has_text"] }
        }),
        json!({
            "id": "agentmessage.notice",
            "name": "发送 AI 协作消息",
            "summary": "向一个或多个 AI 员工发送 AgentMessage。",
            "category": "collaboration",
            "tool_id": "agentmessage.send",
            "payload": {
                "type": "agent_message",
                "from": "template-store",
                "to": ["{{to}}"],
                "message_type": "{{message_type}}",
                "subject": "{{subject}}",
                "summary": "{{summary}}",
                "payload": {}
            },
            "parameters": [
                { "name": "to", "label": "接收员工", "default": "architect-agent", "required": true },
                { "name": "message_type", "label": "消息类型", "default": "broadcast.notice", "required": true },
                { "name": "subject", "label": "主题", "default": "AgentGrid 通知", "required": true },
                { "name": "summary", "label": "内容", "default": "任务模板发送的协作消息。", "required": true }
            ],
            "verify": { "presets": ["agentmessage.delivered"] }
        }),
    ]
}

#[derive(Clone)]
struct SeedAgent {
    id: &'static str,
    name: &'static str,
    role: &'static str,
    skills: &'static [&'static str],
    permissions: &'static [&'static str],
    responsibility: &'static str,
    auth_type: &'static str,
    bootstrap_token: Option<&'static str>,
    credential_status: &'static str,
    account_username: &'static str,
    credential_refs: Value,
    node_scope: Value,
    tool_scope: Value,
}

fn seed_agents() -> Vec<SeedAgent> {
    vec![
        SeedAgent {
            id: "architect-agent",
            name: "项目负责人",
            role: "项目负责人 / 架构负责人",
            skills: &["总体架构", "任务拆解"],
            permissions: &["创建任务", "发送消息"],
            responsibility: "负责 AgentGrid 的整体方向、模块边界、里程碑和协调。",
            auth_type: "bearer_token",
            bootstrap_token: None,
            credential_status: "not_configured",
            account_username: "architect-agent",
            credential_refs: json!({}),
            node_scope: json!({
                "mode": "all",
                "nodes": [],
                "groups": [],
                "os": [],
                "reason": "项目负责人需要看全局资源、任务和节点状态"
            }),
            tool_scope: json!({
                "mode": "all",
                "tools": [],
                "reason": "项目负责人负责调度和验收，不直接绕过任务协议执行"
            }),
        },
        SeedAgent {
            id: "worker-agent",
            name: "节点工程师",
            role: "Worker 工程师",
            skills: &["节点运行时", "任务拉取"],
            permissions: &["执行测试任务", "发送消息"],
            responsibility: "负责 Worker 注册、心跳、领取任务、执行任务、回传结果。",
            auth_type: "bearer_token",
            bootstrap_token: None,
            credential_status: "not_configured",
            account_username: "worker-agent",
            credential_refs: json!({}),
            node_scope: json!({
                "mode": "group",
                "nodes": [],
                "groups": ["default", "linux", "windows", "macos"],
                "os": [],
                "reason": "Worker 工程师维护节点运行链路"
            }),
            tool_scope: json!({
                "mode": "tools",
                "tools": ["http.request", "command.run", "file.manage", "desktop.screenshot"],
                "reason": "只覆盖节点执行和回传相关能力"
            }),
        },
        SeedAgent {
            id: "executor-agent",
            name: "执行器工程师",
            role: "Executor 工程师",
            skills: &["HTTP 执行器", "Command 执行器"],
            permissions: &["编辑执行器"],
            responsibility: "负责 HTTP、命令等具体任务执行。",
            auth_type: "bearer_token",
            bootstrap_token: None,
            credential_status: "not_configured",
            account_username: "executor-agent",
            credential_refs: json!({}),
            node_scope: json!({
                "mode": "group",
                "nodes": [],
                "groups": ["default"],
                "os": [],
                "reason": "执行器工程师默认只操作可执行任务节点"
            }),
            tool_scope: json!({
                "mode": "tools",
                "tools": ["http.request", "command.run", "file.manage", "git.run", "docker.run", "browser.run"],
                "reason": "负责具体执行器协议"
            }),
        },
        SeedAgent {
            id: "scheduler-agent",
            name: "调度工程师",
            role: "Scheduler 工程师",
            skills: &["资源匹配", "负载评分"],
            permissions: &["调整调度策略"],
            responsibility: "负责选择任务派给哪台节点。",
            auth_type: "bearer_token",
            bootstrap_token: None,
            credential_status: "not_configured",
            account_username: "scheduler-agent",
            credential_refs: json!({}),
            node_scope: json!({
                "mode": "all",
                "nodes": [],
                "groups": [],
                "os": [],
                "reason": "调度必须看到所有节点资源，才能做最优选择"
            }),
            tool_scope: json!({
                "mode": "declared",
                "tools": [],
                "reason": "调度工程师调整路由，不直接执行业务工具"
            }),
        },
        SeedAgent {
            id: "qa-agent",
            name: "测试工程师",
            role: "QA 工程师",
            skills: &["集成测试", "回归测试"],
            permissions: &["运行测试"],
            responsibility: "负责验证任务和节点运行链路。",
            auth_type: "bearer_token",
            bootstrap_token: None,
            credential_status: "not_configured",
            account_username: "qa-agent",
            credential_refs: json!({}),
            node_scope: json!({
                "mode": "group",
                "nodes": [],
                "groups": ["default", "test"],
                "os": [],
                "reason": "测试工程师默认覆盖测试组和默认组"
            }),
            tool_scope: json!({
                "mode": "tools",
                "tools": ["http.request", "command.run", "file.manage", "desktop.screenshot"],
                "reason": "验证常用任务执行结果"
            }),
        },
        SeedAgent {
            id: "review-agent",
            name: "代码审查工程师",
            role: "Review 工程师",
            skills: &["代码审查", "风险识别"],
            permissions: &["审查变更"],
            responsibility: "负责审查架构一致性和安全风险。",
            auth_type: "bearer_token",
            bootstrap_token: None,
            credential_status: "not_configured",
            account_username: "review-agent",
            credential_refs: json!({}),
            node_scope: json!({
                "mode": "none",
                "nodes": [],
                "groups": [],
                "os": [],
                "reason": "审查角色默认不直接操作节点"
            }),
            tool_scope: json!({
                "mode": "declared",
                "tools": [],
                "reason": "审查角色主要查看审计和结果"
            }),
        },
        SeedAgent {
            id: "ops-agent",
            name: "运维员工",
            role: "Ops 运维工程师",
            skills: &["节点运维", "软件安装", "服务重启", "远程排障"],
            permissions: &["管理全部节点", "下发命令", "文件操作", "桌面协助"],
            responsibility: "负责所有 AgentGrid 节点的纳管、巡检、安装、更新和故障处理。",
            auth_type: "bearer_token",
            bootstrap_token: None,
            credential_status: "not_configured",
            account_username: "ops-agent",
            credential_refs: json!({
                "ssh": "operator-provided-session",
                "windows": "interactive-user-or-service"
            }),
            node_scope: json!({
                "mode": "all",
                "nodes": [],
                "groups": [],
                "os": [],
                "reason": "运维员工负责全节点维护，允许挂所有节点"
            }),
            tool_scope: json!({
                "mode": "all",
                "tools": [],
                "reason": "运维需要按任务协议调用命令、文件、安装、桌面等能力"
            }),
        },
    ]
}

fn collect_values<F>(rows: rusqlite::MappedRows<'_, F>) -> anyhow::Result<Vec<Value>>
where
    F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<Value>,
{
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn agent_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let token_hash: Option<String> = row.get("token_hash")?;
    Ok(json!({
        "api_version": API_VERSION,
        "kind": "Agent",
        "metadata": {
            "id": row.get::<_, String>("id")?,
            "project_id": row.get::<_, String>("project_id")?,
            "name": row.get::<_, String>("name")?,
            "created_at": row.get::<_, String>("created_at")?,
            "updated_at": row.get::<_, String>("updated_at")?
        },
        "spec": {
            "role": row.get::<_, String>("role")?,
            "skills": json_text(row, "skills_json", json!([]))?,
            "permissions": json_text(row, "permissions_json", json!([]))?,
            "responsibility": row.get::<_, String>("responsibility")?
        },
        "credentials": {
            "auth_type": row.get::<_, String>("auth_type")?,
            "token_configured": token_hash.as_deref().is_some_and(|value| !value.is_empty()),
            "token_hint": row.get::<_, String>("token_hint")?,
            "credential_status": row.get::<_, String>("credential_status")?,
            "account_username": row.get::<_, String>("account_username")?,
            "credential_refs": json_text(row, "credential_refs_json", json!({}))?
        },
        "access": {
            "node_scope": json_text(row, "node_scope_json", json!({"mode": "none", "nodes": [], "groups": [], "os": []}))?,
            "tool_scope": json_text(row, "tool_scope_json", json!({"mode": "declared", "tools": []}))?
        },
        "status": { "state": row.get::<_, String>("status")? }
    }))
}

fn hub_user_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "api_version": API_VERSION,
        "kind": "HubUser",
        "metadata": {
            "id": row.get::<_, String>("id")?,
            "project_id": row.get::<_, String>("project_id")?,
            "organization_id": row.get::<_, String>("organization_id")?,
            "created_at": row.get::<_, String>("created_at")?,
            "updated_at": row.get::<_, String>("updated_at")?
        },
        "spec": {
            "email": row.get::<_, String>("email")?,
            "name": row.get::<_, String>("name")?,
            "role": row.get::<_, String>("role")?
        },
        "credentials": {
            "password_hash": row.get::<_, String>("password_hash")?
        },
        "status": {
            "state": row.get::<_, String>("status")?
        }
    }))
}

fn user_public(mut user: Value) -> Value {
    if let Some(map) = user.as_object_mut() {
        map.remove("credentials");
    }
    user
}

fn node_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let stored_state: String = row.get("status")?;
    let last_heartbeat_at: String = row.get("last_heartbeat_at")?;
    let effective_state = effective_node_state(&stored_state, &last_heartbeat_at);
    Ok(json!({
        "api_version": API_VERSION,
        "kind": "Node",
        "metadata": {
            "id": row.get::<_, String>("id")?,
            "project_id": row.get::<_, String>("project_id")?,
            "name": row.get::<_, String>("name")?,
            "created_at": row.get::<_, String>("created_at")?,
            "updated_at": row.get::<_, String>("updated_at")?
        },
        "spec": {
            "os": row.get::<_, String>("os")?,
            "arch": row.get::<_, String>("arch")?,
            "address": row.get::<_, String>("address")?,
            "tags": json_text(row, "tags_json", json!([]))?,
            "capabilities": json_text(row, "capabilities_json", json!([]))?,
            "groups": json_text(row, "groups_json", json!([]))?,
            "weight": row.get::<_, f64>("weight")?,
            "max_concurrent_jobs": row.get::<_, i64>("max_concurrent_jobs")?,
            "cpu_cores": row.get::<_, i64>("cpu_cores")?,
            "memory_mb": row.get::<_, i64>("memory_mb")?,
            "cpu_usage_percent": row.get::<_, f64>("cpu_usage_percent")?,
            "memory_used_mb": row.get::<_, i64>("memory_used_mb")?,
            "disk_total_mb": row.get::<_, i64>("disk_total_mb")?,
            "disk_free_mb": row.get::<_, i64>("disk_free_mb")?,
            "worker_version": row.get::<_, Option<String>>("worker_version")?,
            "worker_target": row.get::<_, Option<String>>("worker_target")?,
            "glibc_version": row.get::<_, Option<String>>("glibc_version")?,
            "machine_fingerprint": row.get::<_, Option<String>>("machine_fingerprint")?,
            "auth_status": row.get::<_, String>("auth_status")?,
            "join_token_hint": row.get::<_, String>("join_token_hint")?,
            "authorized_at": row.get::<_, Option<String>>("authorized_at")?,
            "auto_update_enabled": row.get::<_, i64>("auto_update_enabled")? == 1,
            "update_channel": row.get::<_, String>("update_channel")?
        },
        "status": {
            "state": effective_state,
            "reported_state": stored_state,
            "running_jobs": row.get::<_, i64>("running_jobs")?,
            "success_count": row.get::<_, i64>("success_count")?,
            "failure_count": row.get::<_, i64>("failure_count")?,
            "last_heartbeat_at": last_heartbeat_at
        }
    }))
}

fn node_tool_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let node_id: String = row.get("node_id")?;
    let tool_id: String = row.get("tool_id")?;
    Ok(json!({
        "api_version": "agentgrid.runtime/v1",
        "kind": "NodeTool",
        "metadata": {
            "id": row.get::<_, String>("id")?,
            "project_id": row.get::<_, String>("project_id")?,
            "node_id": node_id,
            "created_at": row.get::<_, String>("created_at")?,
            "updated_at": row.get::<_, String>("updated_at")?
        },
        "spec": {
            "tool_id": tool_id,
            "name": row.get::<_, String>("name")?,
            "version": row.get::<_, String>("version")?,
            "executor": row.get::<_, String>("executor")?,
            "input_schema": json_text(row, "input_schema_json", json!({}))?,
            "output_schema": json_text(row, "output_schema_json", json!({}))?,
            "constraints": json_text(row, "constraints_json", json!({}))?,
            "labels": json_text(row, "labels_json", json!([]))?,
            "default_verify": json_optional_text(row, "default_verify_json")?,
            "probe": json_optional_text(row, "probe_json")?,
            "metadata": json_text(row, "metadata_json", json!({}))?
        },
        "status": {
            "state": row.get::<_, String>("status")?,
            "confidence": row.get::<_, String>("confidence")?,
            "probe_state": row.get::<_, String>("probe_state")?,
            "last_probe_at": row.get::<_, Option<String>>("last_probe_at")?,
            "next_probe_at": row.get::<_, Option<String>>("next_probe_at")?,
            "probe_task_id": row.get::<_, Option<String>>("probe_task_id")?,
            "probe_error": json_optional_text(row, "probe_error_json")?
        }
    }))
}

fn node_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Node> {
    let stored_state: String = row.get("status")?;
    let last_heartbeat_raw: String = row.get("last_heartbeat_at")?;
    Ok(Node {
        id: row.get("id")?,
        name: row.get("name")?,
        os: row.get("os")?,
        arch: row.get("arch")?,
        tags: parse_json_column(row, "tags_json", Vec::<String>::new())?,
        capabilities: parse_json_column(row, "capabilities_json", Vec::<String>::new())?,
        groups: parse_json_column(row, "groups_json", Vec::<String>::new())?,
        cpu_cores: row.get::<_, i64>("cpu_cores")?.max(0) as u16,
        memory_mb: row.get::<_, i64>("memory_mb")?.max(0) as u64,
        cpu_usage_percent: row.get::<_, f64>("cpu_usage_percent")? as f32,
        memory_used_mb: row.get::<_, i64>("memory_used_mb")?.max(0) as u64,
        disk_total_mb: row.get::<_, i64>("disk_total_mb")?.max(0) as u64,
        disk_free_mb: row.get::<_, i64>("disk_free_mb")?.max(0) as u64,
        running_jobs: row.get::<_, i64>("running_jobs")?.max(0) as u16,
        max_concurrent_jobs: row.get::<_, i64>("max_concurrent_jobs")?.max(1) as u16,
        weight: row.get::<_, f64>("weight")?,
        success_rate: node_success_rate(
            row.get::<_, i64>("success_count")?,
            row.get::<_, i64>("failure_count")?,
        ),
        status: match effective_node_state(&stored_state, &last_heartbeat_raw).as_str() {
            "online" => NodeState::Online,
            "unknown" => NodeState::Unknown,
            "busy" => NodeState::Busy,
            "draining" => NodeState::Draining,
            "disabled" => NodeState::Disabled,
            "untrusted" => NodeState::Untrusted,
            _ => NodeState::Offline,
        },
        last_heartbeat_at: chrono::DateTime::parse_from_rfc3339(&last_heartbeat_raw)
            .map(|value| value.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now() - chrono::Duration::days(365)),
    })
}

fn message_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "api_version": API_VERSION,
        "kind": "AgentMessage",
        "metadata": {
            "id": row.get::<_, String>("id")?,
            "project_id": row.get::<_, String>("project_id")?,
            "from": row.get::<_, String>("from_agent_id")?,
            "to": json_text(row, "to_agents_json", json!([]))?,
            "created_at": row.get::<_, String>("created_at")?
        },
        "spec": {
            "type": row.get::<_, String>("message_type")?,
            "subject": row.get::<_, String>("subject")?,
            "summary": row.get::<_, String>("summary")?,
            "priority": row.get::<_, String>("priority")?,
            "requires_ack": row.get::<_, i64>("requires_ack")? == 1,
            "payload": json_text(row, "payload_json", json!({}))?
        }
    }))
}

fn audit_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "api_version": API_VERSION,
        "kind": "AuditEvent",
        "metadata": {
            "id": row.get::<_, String>("id")?,
            "project_id": row.get::<_, String>("project_id")?,
            "created_at": row.get::<_, String>("created_at")?
        },
        "spec": {
            "type": row.get::<_, String>("event_type")?,
            "actor": row.get::<_, String>("actor")?,
            "subject_id": row.get::<_, Option<String>>("subject_id")?,
            "summary": row.get::<_, String>("summary")?,
            "payload": json_text(row, "payload_json", json!({}))?
        }
    }))
}

fn provisioning_plan_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "api_version": API_VERSION,
        "kind": "NodeProvisioningPlan",
        "metadata": {
            "id": row.get::<_, String>("id")?,
            "project_id": row.get::<_, String>("project_id")?,
            "created_by": row.get::<_, String>("created_by")?,
            "created_at": row.get::<_, String>("created_at")?,
            "updated_at": row.get::<_, String>("updated_at")?
        },
        "spec": {
            "node_id": row.get::<_, String>("node_id")?,
            "node_name": row.get::<_, String>("node_name")?,
            "ssh_host": row.get::<_, String>("ssh_host")?,
            "ssh_user": row.get::<_, String>("ssh_user")?,
            "os": row.get::<_, String>("os")?,
            "arch": row.get::<_, String>("arch")?,
            "hub_url": row.get::<_, String>("hub_url")?,
            "steps": json_text(row, "steps_json", json!([]))?,
            "join_token_hint": row.get::<_, String>("join_token_hint")?,
            "bound_machine_fingerprint": row.get::<_, Option<String>>("bound_machine_fingerprint")?,
            "bound_at": row.get::<_, Option<String>>("bound_at")?,
            "notes": row.get::<_, String>("notes")?
        },
        "status": {
            "state": row.get::<_, String>("status")?
        }
    }))
}

fn workflow_template_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "api_version": API_VERSION,
        "kind": "WorkflowTemplate",
        "metadata": {
            "id": row.get::<_, String>("id")?,
            "project_id": row.get::<_, String>("project_id")?,
            "created_by": row.get::<_, String>("created_by")?,
            "created_at": row.get::<_, String>("created_at")?,
            "updated_at": row.get::<_, String>("updated_at")?
        },
        "spec": {
            "name": row.get::<_, String>("name")?,
            "summary": row.get::<_, String>("summary")?,
            "parameters": json_text(row, "parameters_json", json!([]))?,
            "nodes": json_text(row, "nodes_json", json!([]))?
        }
    }))
}

fn task_template_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "api_version": API_VERSION,
        "kind": "TaskTemplate",
        "metadata": {
            "id": row.get::<_, String>("id")?,
            "project_id": row.get::<_, String>("project_id")?,
            "created_by": row.get::<_, String>("created_by")?,
            "created_at": row.get::<_, String>("created_at")?,
            "updated_at": row.get::<_, String>("updated_at")?
        },
        "spec": {
            "name": row.get::<_, String>("name")?,
            "summary": row.get::<_, String>("summary")?,
            "category": row.get::<_, String>("category")?,
            "tool_id": row.get::<_, String>("tool_id")?,
            "payload": json_text(row, "payload_json", json!({}))?,
            "parameters": json_text(row, "parameters_json", json!([]))?,
            "verify": json_optional_text(row, "verify_json")?,
            "labels": json_text(row, "labels_json", json!([]))?
        }
    }))
}

fn job_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "api_version": "agentgrid.job/v1",
        "kind": "Job",
        "metadata": {
            "id": row.get::<_, String>("id")?,
            "project_id": row.get::<_, String>("project_id")?,
            "created_by": row.get::<_, String>("created_by")?,
            "created_at": row.get::<_, String>("created_at")?,
            "updated_at": row.get::<_, String>("updated_at")?
        },
        "spec": {
            "title": row.get::<_, String>("title")?,
            "summary": row.get::<_, String>("summary")?,
            "tool_id": row.get::<_, String>("tool_id")?,
            "payload": json_text(row, "payload_json", json!({}))?,
            "placement": json_text(row, "placement_json", json!({}))?,
            "strategy": json_text(row, "strategy_json", json!({"type": "single"}))?,
            "reduce": json_text(row, "reduce_json", json!({"type": "summary"}))?,
            "retry_policy": json_text(row, "retry_policy_json", json!({}))?,
            "checkpoint_policy": json_text(row, "checkpoint_policy_json", json!({}))?,
            "idempotency": json_text(row, "idempotency_json", json!({}))?,
            "idempotency_key": row.get::<_, Option<String>>("idempotency_key")?
        },
        "status": {
            "state": row.get::<_, String>("status")?,
            "max_attempts": row.get::<_, i64>("max_attempts")?,
            "idempotency_reused": false,
            "idempotency_key": row.get::<_, Option<String>>("idempotency_key")?,
            "latest_checkpoint_id": row.get::<_, Option<String>>("latest_checkpoint_id")?,
            "current_attempt_id": row.get::<_, Option<String>>("current_attempt_id")?,
            "current_task_id": row.get::<_, Option<String>>("current_task_id")?,
            "reducer_task_id": row.get::<_, Option<String>>("reducer_task_id")?,
            "completed_at": row.get::<_, Option<String>>("completed_at")?,
            "result": json_optional_text(row, "result_json")?,
            "error": json_optional_text(row, "error_json")?
        }
    }))
}

fn job_attempt_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "api_version": "agentgrid.job/v1",
        "kind": "JobAttempt",
        "metadata": {
            "id": row.get::<_, String>("id")?,
            "project_id": row.get::<_, String>("project_id")?,
            "job_id": row.get::<_, String>("job_id")?,
            "shard_id": row.get::<_, Option<String>>("shard_id")?,
            "task_id": row.get::<_, String>("task_id")?,
            "created_at": row.get::<_, String>("created_at")?,
            "updated_at": row.get::<_, String>("updated_at")?
        },
        "spec": {
            "attempt_number": row.get::<_, i64>("attempt_number")?,
            "reason": row.get::<_, String>("reason")?,
            "resume_checkpoint_id": row.get::<_, Option<String>>("resume_checkpoint_id")?
        },
        "status": {
            "state": row.get::<_, String>("status")?,
            "node_id": row.get::<_, Option<String>>("node_id")?,
            "started_at": row.get::<_, Option<String>>("started_at")?,
            "completed_at": row.get::<_, Option<String>>("completed_at")?,
            "result": json_optional_text(row, "result_json")?,
            "error": json_optional_text(row, "error_json")?
        }
    }))
}

fn job_shard_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "api_version": "agentgrid.job/v1",
        "kind": "JobShard",
        "metadata": {
            "id": row.get::<_, String>("id")?,
            "project_id": row.get::<_, String>("project_id")?,
            "job_id": row.get::<_, String>("job_id")?,
            "created_at": row.get::<_, String>("created_at")?,
            "updated_at": row.get::<_, String>("updated_at")?
        },
        "spec": {
            "shard_index": row.get::<_, i64>("shard_index")?,
            "shard_count": row.get::<_, i64>("shard_count")?,
            "payload": json_text(row, "payload_json", json!({}))?
        },
        "status": {
            "state": row.get::<_, String>("status")?,
            "current_attempt_id": row.get::<_, Option<String>>("current_attempt_id")?,
            "current_task_id": row.get::<_, Option<String>>("current_task_id")?,
            "node_id": row.get::<_, Option<String>>("node_id")?,
            "completed_at": row.get::<_, Option<String>>("completed_at")?,
            "result": json_optional_text(row, "result_json")?,
            "error": json_optional_text(row, "error_json")?
        }
    }))
}

fn job_checkpoint_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "api_version": "agentgrid.job/v1",
        "kind": "JobCheckpoint",
        "metadata": {
            "id": row.get::<_, String>("id")?,
            "project_id": row.get::<_, String>("project_id")?,
            "job_id": row.get::<_, String>("job_id")?,
            "attempt_id": row.get::<_, Option<String>>("attempt_id")?,
            "task_id": row.get::<_, Option<String>>("task_id")?,
            "node_id": row.get::<_, Option<String>>("node_id")?,
            "created_at": row.get::<_, String>("created_at")?
        },
        "status": {
            "sequence": row.get::<_, i64>("sequence")?,
            "progress": row.get::<_, i64>("progress")?,
            "resume_token": json_text(row, "resume_token_json", json!({}))?,
            "artifacts": json_text(row, "artifacts_json", json!([]))?
        }
    }))
}

fn ingress_event_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "api_version": "agentgrid.event/v1",
        "kind": "NodeEvent",
        "metadata": {
            "id": row.get::<_, String>("id")?,
            "project_id": row.get::<_, String>("project_id")?,
            "created_at": row.get::<_, String>("created_at")?,
            "updated_at": row.get::<_, String>("updated_at")?
        },
        "spec": {
            "source": row.get::<_, String>("source")?,
            "target": json_text(row, "target_json", json!({}))?,
            "type": row.get::<_, String>("event_type")?,
            "idempotency_key": row.get::<_, String>("idempotency_key")?,
            "ttl_seconds": row.get::<_, i64>("ttl_seconds")?,
            "payload": json_text(row, "payload_json", json!({}))?
        },
        "status": {
            "state": row.get::<_, String>("status")?
        }
    }))
}

fn webhook_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let secret = row.get::<_, Option<String>>("secret")?;
    Ok(json!({
        "api_version": API_VERSION,
        "kind": "WebhookSubscription",
        "metadata": {
            "id": row.get::<_, String>("id")?,
            "project_id": row.get::<_, String>("project_id")?,
            "created_by": row.get::<_, String>("created_by")?,
            "created_at": row.get::<_, String>("created_at")?,
            "updated_at": row.get::<_, String>("updated_at")?
        },
        "spec": {
            "name": row.get::<_, String>("name")?,
            "url": row.get::<_, String>("url")?,
            "events": json_text(row, "events_json", json!([]))?,
            "enabled": row.get::<_, i64>("enabled")? == 1,
            "has_secret": secret.as_deref().map(|value| !value.is_empty()).unwrap_or(false)
        }
    }))
}

fn webhook_delivery_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "api_version": API_VERSION,
        "kind": "WebhookDelivery",
        "metadata": {
            "id": row.get::<_, String>("id")?,
            "project_id": row.get::<_, String>("project_id")?,
            "created_at": row.get::<_, String>("created_at")?
        },
        "spec": {
            "webhook_id": row.get::<_, String>("webhook_id")?,
            "event_type": row.get::<_, String>("event_type")?,
            "subject_id": row.get::<_, Option<String>>("subject_id")?,
            "status": row.get::<_, String>("status")?,
            "status_code": row.get::<_, Option<i64>>("status_code")?,
            "error": row.get::<_, Option<String>>("error")?,
            "payload": json_text(row, "payload_json", json!({}))?
        }
    }))
}

fn task_log_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "api_version": API_VERSION,
        "kind": "TaskLog",
        "metadata": {
            "id": row.get::<_, String>("id")?,
            "project_id": row.get::<_, String>("project_id")?,
            "created_at": row.get::<_, String>("created_at")?
        },
        "spec": {
            "task_id": row.get::<_, String>("task_id")?,
            "node_id": row.get::<_, String>("node_id")?,
            "stream": row.get::<_, String>("stream")?,
            "line": row.get::<_, String>("line")?,
            "sequence": row.get::<_, i64>("sequence")?
        }
    }))
}

fn artifact_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let id: String = row.get("id")?;
    Ok(json!({
        "api_version": API_VERSION,
        "kind": "Artifact",
        "metadata": {
            "id": id,
            "project_id": row.get::<_, String>("project_id")?,
            "created_at": row.get::<_, String>("created_at")?
        },
        "spec": {
            "task_id": row.get::<_, String>("task_id")?,
            "node_id": row.get::<_, Option<String>>("node_id")?,
            "name": row.get::<_, String>("name")?,
            "type": row.get::<_, String>("artifact_type")?,
            "content_type": row.get::<_, String>("content_type")?,
            "content_base64": row.get::<_, Option<String>>("content_base64")?,
            "source_path": row.get::<_, Option<String>>("source_path")?,
            "size_bytes": row.get::<_, i64>("size_bytes")?,
            "metadata": json_text(row, "metadata_json", json!({}))?,
            "v2": artifact_v2_view(&json_text(row, "metadata_json", json!({}))?),
            "download_url": format!("/api/artifacts/{id}/download")
        }
    }))
}

fn tool_probe_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "api_version": API_VERSION,
        "kind": "ToolProbe",
        "metadata": {
            "id": row.get::<_, String>("id")?,
            "project_id": row.get::<_, String>("project_id")?,
            "tool_id": row.get::<_, String>("tool_id")?,
            "node_id": row.get::<_, String>("node_id")?,
            "task_id": row.get::<_, Option<String>>("task_id")?,
            "created_at": row.get::<_, String>("created_at")?,
            "updated_at": row.get::<_, String>("updated_at")?
        },
        "spec": {
            "support_basis": row.get::<_, String>("support_basis")?
        },
        "status": {
            "state": row.get::<_, String>("status")?,
            "started_at": row.get::<_, Option<String>>("started_at")?,
            "completed_at": row.get::<_, Option<String>>("completed_at")?,
            "expires_at": row.get::<_, Option<String>>("expires_at")?,
            "result": json_optional_text(row, "result_json")?,
            "error": json_optional_text(row, "error_json")?
        }
    }))
}

fn workflow_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "api_version": API_VERSION,
        "kind": "Workflow",
        "metadata": {
            "id": row.get::<_, String>("id")?,
            "project_id": row.get::<_, String>("project_id")?,
            "created_by": row.get::<_, String>("created_by")?,
            "created_at": row.get::<_, String>("created_at")?,
            "updated_at": row.get::<_, String>("updated_at")?
        },
        "spec": {
            "name": row.get::<_, String>("name")?,
            "summary": row.get::<_, String>("summary")?,
            "inputs": json_text(row, "inputs_json", json!({}))?,
            "nodes": json_text(row, "nodes_json", json!([]))?
        },
        "status": {
            "state": row.get::<_, String>("status")?,
            "started_at": row.get::<_, Option<String>>("started_at")?,
            "completed_at": row.get::<_, Option<String>>("completed_at")?,
            "result": json_optional_text(row, "result_json")?,
            "error": json_optional_text(row, "error_json")?
        }
    }))
}

fn workflow_run_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "api_version": API_VERSION,
        "kind": "WorkflowRun",
        "metadata": {
            "id": row.get::<_, String>("id")?,
            "project_id": row.get::<_, String>("project_id")?,
            "workflow_id": row.get::<_, String>("workflow_id")?,
            "workflow_node_id": row.get::<_, String>("workflow_node_id")?,
            "task_id": row.get::<_, Option<String>>("task_id")?,
            "created_at": row.get::<_, String>("created_at")?,
            "updated_at": row.get::<_, String>("updated_at")?
        },
        "spec": {
            "depends_on": json_text(row, "depends_on_json", json!([]))?
        },
        "status": {
            "state": row.get::<_, String>("status")?,
            "started_at": row.get::<_, Option<String>>("started_at")?,
            "completed_at": row.get::<_, Option<String>>("completed_at")?,
            "result": json_optional_text(row, "result_json")?,
            "error": json_optional_text(row, "error_json")?
        }
    }))
}

fn task_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    Ok(json!({
        "api_version": API_VERSION,
        "kind": "AgentTask",
            "metadata": {
                "id": row.get::<_, String>("id")?,
                "project_id": row.get::<_, String>("project_id")?,
                "created_by": row.get::<_, String>("created_by")?,
                "assigned_to": json_text(row, "assigned_to_json", json!([]))?,
                "created_at": row.get::<_, String>("created_at")?,
                "updated_at": row.get::<_, String>("updated_at")?,
                "correlation_id": row.get::<_, Option<String>>("correlation_id")?,
                "workflow_id": row.get::<_, Option<String>>("workflow_id")?,
                "workflow_node_id": row.get::<_, Option<String>>("workflow_node_id")?,
                "job_id": row.get::<_, Option<String>>("job_id")?,
                "job_attempt_id": row.get::<_, Option<String>>("job_attempt_id")?,
                "job_shard_id": row.get::<_, Option<String>>("job_shard_id")?
            },
        "spec": {
            "title": row.get::<_, String>("title")?,
            "summary": row.get::<_, String>("summary")?,
            "owner": row.get::<_, Option<String>>("owner_agent_id")?,
            "priority": row.get::<_, String>("priority")?,
            "inputs": json_text(row, "inputs_json", json!([]))?,
            "outputs": json_text(row, "outputs_json", json!([]))?,
            "acceptance_criteria": json_text(row, "acceptance_criteria_json", json!([]))?,
            "verify": json_optional_text(row, "verify_json")?,
            "labels": json_text(row, "labels_json", json!([]))?,
            "depends_on": json_text(row, "depends_on_json", json!([]))?,
            "due_at": row.get::<_, Option<String>>("due_at")?
        },
        "status": {
            "state": row.get::<_, String>("status")?,
            "progress": row.get::<_, i64>("progress")?,
            "started_at": row.get::<_, Option<String>>("started_at")?,
            "completed_at": row.get::<_, Option<String>>("completed_at")?,
            "blocked_reason": row.get::<_, Option<String>>("blocked_reason")?,
            "last_message_id": row.get::<_, Option<String>>("last_message_id")?,
            "leased_by_node_id": row.get::<_, Option<String>>("leased_by_node_id")?,
            "lease_expires_at": row.get::<_, Option<String>>("lease_expires_at")?,
            "attempts": row.get::<_, i64>("attempts")?,
            "result": json_optional_text(row, "result_json")?,
            "error": json_optional_text(row, "error_json")?,
            "control": json_optional_text(row, "control_json")?
        }
    }))
}

fn parse_workflow_nodes(value: &Value) -> anyhow::Result<Vec<WorkflowNode>> {
    let items = value
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("workflow nodes must be an array"))?;
    let mut nodes = Vec::with_capacity(items.len());
    for item in items {
        let id = item
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow::anyhow!("workflow node id is required"))?
            .to_string();
        let payload = item
            .get("payload")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("workflow node payload is required"))?;
        let mut labels = item
            .get("labels")
            .map(string_array_from_value)
            .unwrap_or_default();
        ensure_label(&mut labels, "compute");
        ensure_label(&mut labels, workflow_payload_label(&payload));
        nodes.push(WorkflowNode {
            id,
            title: item
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("工作流节点")
                .to_string(),
            summary: item
                .get("summary")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            payload,
            depends_on: item
                .get("depends_on")
                .map(string_array_from_value)
                .unwrap_or_default(),
            on_failure: item
                .get("on_failure")
                .and_then(Value::as_str)
                .unwrap_or("fail_workflow")
                .to_string(),
            optional: item
                .get("optional")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            labels,
            owner: item
                .get("owner")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            priority: item
                .get("priority")
                .and_then(Value::as_str)
                .unwrap_or("normal")
                .to_string(),
            acceptance_criteria: item
                .get("acceptance_criteria")
                .map(string_array_from_value)
                .unwrap_or_else(|| {
                    vec!["节点任务执行成功".to_string(), "结果写回 Hub".to_string()]
                }),
            outputs: item
                .get("outputs")
                .map(string_array_from_value)
                .unwrap_or_else(|| vec!["结构化结果".to_string(), "执行日志".to_string()]),
        });
    }
    Ok(nodes)
}

fn validate_workflow_nodes(nodes: &[WorkflowNode]) -> anyhow::Result<()> {
    if nodes.is_empty() {
        anyhow::bail!("workflow must contain at least one node");
    }
    let mut ids = HashSet::new();
    for node in nodes {
        if !ids.insert(node.id.clone()) {
            anyhow::bail!("duplicate workflow node id: {}", node.id);
        }
    }
    for node in nodes {
        for dependency in &node.depends_on {
            if !ids.contains(dependency) {
                anyhow::bail!(
                    "workflow node {} depends on unknown node {}",
                    node.id,
                    dependency
                );
            }
            if dependency == &node.id {
                anyhow::bail!("workflow node {} cannot depend on itself", node.id);
            }
        }
    }
    let graph = nodes
        .iter()
        .map(|node| (node.id.as_str(), node.depends_on.as_slice()))
        .collect::<HashMap<_, _>>();
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    for node in nodes {
        validate_workflow_node_acyclic(&node.id, &graph, &mut visiting, &mut visited)?;
    }
    Ok(())
}

fn validate_workflow_node_acyclic<'a>(
    node_id: &'a str,
    graph: &HashMap<&'a str, &'a [String]>,
    visiting: &mut HashSet<&'a str>,
    visited: &mut HashSet<&'a str>,
) -> anyhow::Result<()> {
    if visited.contains(node_id) {
        return Ok(());
    }
    if !visiting.insert(node_id) {
        anyhow::bail!("workflow contains a cycle at node {node_id}");
    }
    if let Some(dependencies) = graph.get(node_id) {
        for dependency in *dependencies {
            validate_workflow_node_acyclic(dependency, graph, visiting, visited)?;
        }
    }
    visiting.remove(node_id);
    visited.insert(node_id);
    Ok(())
}

fn workflow_node_to_json(node: &WorkflowNode) -> Value {
    json!({
        "id": node.id,
        "title": node.title,
        "summary": node.summary,
        "payload": node.payload,
        "depends_on": node.depends_on,
        "on_failure": node.on_failure,
        "optional": node.optional,
        "labels": node.labels,
        "owner": node.owner,
        "priority": node.priority,
        "acceptance_criteria": node.acceptance_criteria,
        "outputs": node.outputs
    })
}

fn workflow_node_task_payload(workflow_id: &str, node: &WorkflowNode) -> Value {
    let owner = node
        .owner
        .clone()
        .unwrap_or_else(|| "worker-agent".to_string());
    json!({
        "title": node.title,
        "summary": node.summary,
        "created_by": "workflow-engine",
        "owner": owner,
        "assigned_to": [owner],
        "labels": node.labels,
        "priority": node.priority,
        "inputs": [serde_json::to_string_pretty(&node.payload).unwrap_or_else(|_| node.payload.to_string())],
        "outputs": node.outputs,
        "acceptance_criteria": node.acceptance_criteria,
        "depends_on": node.depends_on,
        "correlation_id": workflow_id,
        "workflow_id": workflow_id,
        "workflow_node_id": node.id
    })
}

fn render_workflow_node(node: &WorkflowNode, context: &Value) -> anyhow::Result<WorkflowNode> {
    Ok(WorkflowNode {
        id: node.id.clone(),
        title: render_workflow_template_text(&node.title, context)?,
        summary: render_workflow_template_text(&node.summary, context)?,
        payload: render_workflow_template_value(&node.payload, context)?,
        depends_on: node.depends_on.clone(),
        on_failure: node.on_failure.clone(),
        optional: node.optional,
        labels: node
            .labels
            .iter()
            .map(|label| render_workflow_template_text(label, context))
            .collect::<anyhow::Result<Vec<_>>>()?,
        owner: node
            .owner
            .as_ref()
            .map(|owner| render_workflow_template_text(owner, context))
            .transpose()?,
        priority: node.priority.clone(),
        acceptance_criteria: node.acceptance_criteria.clone(),
        outputs: node.outputs.clone(),
    })
}

fn render_workflow_template_value(value: &Value, context: &Value) -> anyhow::Result<Value> {
    match value {
        Value::String(text) => Ok(Value::String(render_workflow_template_text(text, context)?)),
        Value::Array(items) => Ok(Value::Array(
            items
                .iter()
                .map(|item| render_workflow_template_value(item, context))
                .collect::<anyhow::Result<Vec<_>>>()?,
        )),
        Value::Object(map) => {
            let rendered = map
                .iter()
                .map(|(key, value)| {
                    Ok((key.clone(), render_workflow_template_value(value, context)?))
                })
                .collect::<anyhow::Result<serde_json::Map<_, _>>>()?;
            Ok(Value::Object(rendered))
        }
        other => Ok(other.clone()),
    }
}

fn render_workflow_template_text(text: &str, context: &Value) -> anyhow::Result<String> {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        output.push_str(&rest[..start]);
        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find('}') else {
            anyhow::bail!("unclosed workflow template expression in {text}");
        };
        let expression = after_start[..end].trim();
        if expression.is_empty() {
            anyhow::bail!("empty workflow template expression in {text}");
        }
        let value = resolve_workflow_context_path(context, expression)
            .ok_or_else(|| anyhow::anyhow!("workflow context value not found: {expression}"))?;
        write!(&mut output, "{}", workflow_context_value_to_string(value))?;
        rest = &after_start[end + 1..];
    }
    output.push_str(rest);
    Ok(output)
}

fn resolve_workflow_context_path<'a>(context: &'a Value, expression: &str) -> Option<&'a Value> {
    let mut current = context;
    for part in expression.split('.') {
        if part.is_empty() {
            return None;
        }
        current = match current {
            Value::Object(map) => map.get(part)?,
            Value::Array(items) => items.get(part.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(current)
}

fn workflow_context_value_to_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn workflow_context_keys(context: &Value) -> Vec<String> {
    context
        .get("steps")
        .and_then(Value::as_object)
        .map(|steps| steps.keys().cloned().collect())
        .unwrap_or_default()
}

fn workflow_payload_label(payload: &Value) -> &'static str {
    match payload.get("type").and_then(Value::as_str).unwrap_or("") {
        "http_request" => "http_request",
        "file" => "file",
        "git" => "git",
        "docker" => "docker",
        "browser" => "browser",
        "session" => "session",
        "agent_message" | "agentmessage" => "agentmessage",
        "command" => "command",
        _ => "command",
    }
}

fn ensure_label(labels: &mut Vec<String>, label: &str) {
    if !labels.iter().any(|item| item == label) {
        labels.push(label.to_string());
    }
}

fn normalize_job_strategy(value: Option<&Value>) -> Value {
    let mut strategy = value
        .cloned()
        .unwrap_or_else(|| json!({ "type": "single" }));
    let Some(map) = strategy.as_object_mut() else {
        return json!({ "type": "single" });
    };
    let strategy_type = map
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("single")
        .to_string();
    if strategy_type != "sharded" {
        return json!({ "type": "single" });
    }
    let shard_count = map
        .get("shard_count")
        .and_then(Value::as_i64)
        .unwrap_or(1)
        .clamp(1, 1024);
    let max_parallelism = map
        .get("max_parallelism")
        .and_then(Value::as_i64)
        .unwrap_or(shard_count)
        .clamp(1, shard_count);
    json!({
        "type": "sharded",
        "shard_count": shard_count,
        "max_parallelism": max_parallelism,
        "payload_mode": map.get("payload_mode").and_then(Value::as_str).unwrap_or("inject_shard")
    })
}

fn attach_partition_to_strategy(mut strategy: Value, partition: &Value) -> Value {
    if let Some(map) = strategy.as_object_mut() {
        map.insert("partition".to_string(), partition.clone());
    }
    strategy
}

fn normalize_job_partition(value: Option<&Value>) -> anyhow::Result<Value> {
    let Some(value) = value else {
        return Ok(json!({ "type": "none" }));
    };
    let partition_type = value.get("type").and_then(Value::as_str).unwrap_or("none");
    match partition_type {
        "items" => {
            let items = value
                .get("items")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            Ok(json!({
                "type": "items",
                "mode": value.get("mode").and_then(Value::as_str).unwrap_or("balanced"),
                "items": items
            }))
        }
        "range" => {
            let start = value.get("start").and_then(Value::as_i64).unwrap_or(0);
            let end = value.get("end").and_then(Value::as_i64).unwrap_or(start);
            if end < start {
                anyhow::bail!("partition range end must be greater than or equal to start");
            }
            Ok(json!({
                "type": "range",
                "start": start,
                "end": end,
                "step": value.get("step").and_then(Value::as_i64).unwrap_or(1).max(1)
            }))
        }
        _ => Ok(json!({ "type": "none" })),
    }
}

fn normalize_job_reduce(value: Option<&Value>) -> Value {
    let reduce = value
        .cloned()
        .unwrap_or_else(|| json!({ "type": "summary" }));
    let Some(map) = reduce.as_object() else {
        return json!({ "type": "summary" });
    };
    let reduce_type = map.get("type").and_then(Value::as_str).unwrap_or("summary");
    match reduce_type {
        "stdout_concat" => json!({
            "type": "stdout_concat",
            "order_by": map.get("order_by").and_then(Value::as_str).unwrap_or("shard_index")
        }),
        "json_array" => json!({
            "type": "json_array",
            "order_by": map.get("order_by").and_then(Value::as_str).unwrap_or("shard_index")
        }),
        _ => json!({ "type": "summary" }),
    }
}

fn inject_job_shard_payload(
    mut payload: Value,
    index: i64,
    count: i64,
    partition: Value,
) -> anyhow::Result<Value> {
    let shard = json!({
        "index": index,
        "count": count,
        "first": index == 0,
        "last": index + 1 == count
    });
    inject_payload_field(&mut payload, "shard", shard.clone())?;
    inject_payload_field(&mut payload, "partition", partition.clone())?;
    let context = json!({
        "shard": shard,
        "partition": partition
    });
    render_job_payload_template_value(&payload, &context)
}

fn partition_for_shard(strategy: &Value, index: i64, count: i64) -> anyhow::Result<Value> {
    let partition = strategy
        .get("partition")
        .cloned()
        .unwrap_or_else(|| json!({ "type": "none" }));
    match partition
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("none")
    {
        "items" => {
            let items = partition
                .get("items")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let selected = balanced_partition_slice(&items, index, count);
            Ok(json!({
                "type": "items",
                "items": selected,
                "item_count": selected.len(),
                "total_items": items.len()
            }))
        }
        "range" => {
            let start = partition.get("start").and_then(Value::as_i64).unwrap_or(0);
            let end = partition
                .get("end")
                .and_then(Value::as_i64)
                .unwrap_or(start);
            let step = partition
                .get("step")
                .and_then(Value::as_i64)
                .unwrap_or(1)
                .max(1);
            let total = ((end - start) + step - 1) / step;
            let shard_start_offset = balanced_range_start(total, index, count);
            let shard_end_offset = balanced_range_start(total, index + 1, count);
            Ok(json!({
                "type": "range",
                "start": start + shard_start_offset * step,
                "end": start + shard_end_offset * step,
                "step": step,
                "total_units": total
            }))
        }
        _ => Ok(json!({ "type": "none" })),
    }
}

fn render_job_payload_template_value(value: &Value, context: &Value) -> anyhow::Result<Value> {
    match value {
        Value::String(text) => Ok(Value::String(render_job_payload_template_text(
            text, context,
        )?)),
        Value::Array(items) => Ok(Value::Array(
            items
                .iter()
                .map(|item| render_job_payload_template_value(item, context))
                .collect::<anyhow::Result<Vec<_>>>()?,
        )),
        Value::Object(map) => {
            let rendered = map
                .iter()
                .map(|(key, value)| {
                    Ok((
                        key.clone(),
                        render_job_payload_template_value(value, context)?,
                    ))
                })
                .collect::<anyhow::Result<serde_json::Map<_, _>>>()?;
            Ok(Value::Object(rendered))
        }
        other => Ok(other.clone()),
    }
}

fn render_job_payload_template_text(text: &str, context: &Value) -> anyhow::Result<String> {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        output.push_str(&rest[..start]);
        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find('}') else {
            anyhow::bail!("unclosed job payload template expression in {text}");
        };
        let expression = after_start[..end].trim();
        if expression.is_empty() {
            anyhow::bail!("empty job payload template expression in {text}");
        }
        let value = resolve_job_template_path(context, expression)
            .ok_or_else(|| anyhow::anyhow!("job payload template value not found: {expression}"))?;
        write!(&mut output, "{}", workflow_context_value_to_string(value))?;
        rest = &after_start[end + 1..];
    }
    output.push_str(rest);
    Ok(output)
}

fn resolve_job_template_path<'a>(context: &'a Value, expression: &str) -> Option<&'a Value> {
    let mut current = context;
    for part in expression.split('.') {
        if part.is_empty() {
            return None;
        }
        current = resolve_job_template_part(current, part)?;
    }
    Some(current)
}

fn resolve_job_template_part<'a>(value: &'a Value, part: &str) -> Option<&'a Value> {
    let mut field = part;
    let mut current = value;
    loop {
        let Some(bracket_start) = field.find('[') else {
            return match current {
                Value::Object(map) => map.get(field),
                Value::Array(items) => items.get(field.parse::<usize>().ok()?),
                _ => None,
            };
        };
        let key = &field[..bracket_start];
        if !key.is_empty() {
            current = match current {
                Value::Object(map) => map.get(key)?,
                _ => return None,
            };
        }
        let after_start = &field[bracket_start + 1..];
        let bracket_end = after_start.find(']')?;
        let index = after_start[..bracket_end].trim().parse::<usize>().ok()?;
        current = current.as_array()?.get(index)?;
        field = &after_start[bracket_end + 1..];
        if field.is_empty() {
            return Some(current);
        }
    }
}

fn balanced_partition_slice(items: &[Value], index: i64, count: i64) -> Vec<Value> {
    let len = items.len() as i64;
    let start = balanced_range_start(len, index, count) as usize;
    let end = balanced_range_start(len, index + 1, count) as usize;
    items[start.min(items.len())..end.min(items.len())].to_vec()
}

fn balanced_range_start(total: i64, index: i64, count: i64) -> i64 {
    if total <= 0 || count <= 0 {
        return 0;
    }
    ((total * index.clamp(0, count)) / count).clamp(0, total)
}

fn inject_payload_field(payload: &mut Value, key: &str, value: Value) -> anyhow::Result<()> {
    let map = payload
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("job payload must be a JSON object"))?;
    map.insert(key.to_string(), value);
    Ok(())
}

fn collect_reduce_artifacts(shards: &[Value]) -> Value {
    let mut artifacts = Vec::new();
    for shard in shards {
        if let Some(items) = shard
            .pointer("/status/result/artifacts")
            .and_then(Value::as_array)
        {
            for item in items {
                artifacts.push(item.clone());
            }
        }
    }
    Value::Array(artifacts)
}

fn render_template_value(value: &Value, parameters: &Value) -> Value {
    match value {
        Value::String(text) => Value::String(render_template_text(text, parameters)),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| render_template_value(item, parameters))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| (key.clone(), render_template_value(value, parameters)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn render_template_text(text: &str, parameters: &Value) -> String {
    let Some(map) = parameters.as_object() else {
        return text.to_string();
    };
    let mut output = text.to_string();
    for (key, value) in map {
        let replacement = value
            .as_str()
            .map(ToString::to_string)
            .unwrap_or_else(|| value.to_string());
        output = output.replace(&format!("${{{key}}}"), &replacement);
    }
    output
}

fn node_provisioning_steps(
    node_id: &str,
    node_name: &str,
    ssh_host: &str,
    ssh_user: &str,
    hub_url: &str,
) -> Value {
    let install_command = format!(
        "curl -fsSL {hub_url}/worker/download/linux-x86_64 -o /opt/agentgrid-worker/agentgrid-worker && chmod +x /opt/agentgrid-worker/agentgrid-worker"
    );
    let service = format!(
        "[Unit]\nDescription=AgentGrid Worker\nAfter=network-online.target\nWants=network-online.target\n\n[Service]\nType=simple\nExecStart=/opt/agentgrid-worker/agentgrid-worker --hub {hub_url} --id {node_id} --name \"{node_name}\" --max-concurrent-jobs 4\nRestart=always\nRestartSec=3\n\n[Install]\nWantedBy=multi-user.target\n"
    );
    json!([
        {
            "name": "连接服务器",
            "command": format!("ssh {ssh_user}@{ssh_host}"),
            "description": "从运维机器进入目标主机。"
        },
        {
            "name": "创建运行目录",
            "command": "sudo mkdir -p /opt/agentgrid-worker && sudo chown -R $(id -u):$(id -g) /opt/agentgrid-worker",
            "description": "为 Worker 二进制和运行文件准备目录。"
        },
        {
            "name": "安装 Worker",
            "command": install_command,
            "description": "从中心服务器下载对应平台 Worker。老 glibc 主机需要使用兼容目标包。"
        },
        {
            "name": "写入 systemd 服务",
            "content": service,
            "description": "保存为 /etc/systemd/system/agentgrid-worker.service。"
        },
        {
            "name": "启动服务",
            "command": "sudo systemctl daemon-reload && sudo systemctl enable --now agentgrid-worker && sudo systemctl status agentgrid-worker --no-pager",
            "description": "启动后节点会主动向 Hub 心跳，不需要开放子节点入口。"
        }
    ])
}

fn default_workflow_templates() -> Vec<Value> {
    vec![
        json!({
            "id": "node-healthcheck",
            "name": "节点健康巡检",
            "summary": "按目标系统巡检 hostname、磁盘和 uptime。",
            "parameters": [
                { "name": "target_os", "label": "目标系统", "default": "linux" }
            ],
            "nodes": [
                {
                    "id": "hostname",
                    "title": "获取主机名",
                    "payload": { "type": "command", "program": "hostname", "args": [], "timeout_seconds": 30 },
                    "labels": ["compute", "command", "os:${target_os}"]
                },
                {
                    "id": "disk",
                    "title": "检查磁盘空间",
                    "depends_on": ["hostname"],
                    "payload": { "type": "command", "program": "df", "args": ["-h"], "timeout_seconds": 30 },
                    "labels": ["compute", "command", "os:${target_os}"]
                },
                {
                    "id": "uptime",
                    "title": "检查运行时间",
                    "depends_on": ["disk"],
                    "payload": { "type": "command", "program": "uptime", "args": [], "timeout_seconds": 30 },
                    "labels": ["compute", "command", "os:${target_os}"]
                }
            ]
        }),
        json!({
            "id": "http-probe",
            "name": "HTTP 探测流水线",
            "summary": "探测 URL 后发送 AgentMessage 协作消息。",
            "parameters": [
                { "name": "url", "label": "探测地址", "default": "https://httpbin.org/get" },
                { "name": "reviewer", "label": "接收员工", "default": "review-agent" }
            ],
            "nodes": [
                {
                    "id": "fetch",
                    "title": "请求 ${url}",
                    "payload": { "type": "http_request", "method": "GET", "url": "${url}", "headers": [], "body": null, "timeout_seconds": 30, "max_response_bytes": 65536 },
                    "labels": ["compute", "http_request"]
                },
                {
                    "id": "notify",
                    "title": "发送探测结果通知",
                    "depends_on": ["fetch"],
                    "payload": { "type": "agent_message", "from": "workflow-engine", "to": ["${reviewer}"], "message_type": "workflow.probe.completed", "subject": "HTTP 探测完成", "summary": "${url} 已完成探测。", "payload": {} },
                    "labels": ["compute", "agentmessage"]
                }
            ]
        }),
        json!({
            "id": "repo-ci-skeleton",
            "name": "仓库 CI 骨架",
            "summary": "对节点上的仓库执行 Git 状态检查和测试命令。",
            "parameters": [
                { "name": "repo_dir", "label": "仓库目录", "default": "/tmp/repo" },
                { "name": "test_command", "label": "测试命令", "default": "cargo test" }
            ],
            "nodes": [
                {
                    "id": "git_status",
                    "title": "检查 Git 状态",
                    "payload": { "type": "git", "operation": "status", "repo_dir": "${repo_dir}" },
                    "labels": ["compute", "git"]
                },
                {
                    "id": "run_tests",
                    "title": "运行测试命令",
                    "depends_on": ["git_status"],
                    "payload": { "type": "command", "program": "sh", "args": ["-lc", "${test_command}"], "working_dir": "${repo_dir}", "timeout_seconds": 600 },
                    "labels": ["compute", "command"]
                }
            ]
        }),
    ]
}

fn workflow_progress(runs: &[Value]) -> i64 {
    if runs.is_empty() {
        return 0;
    }
    let done = count_runs(runs, "done") + count_runs(runs, "skipped");
    ((done as f64 / runs.len() as f64) * 100.0).round() as i64
}

fn count_runs(runs: &[Value], state: &str) -> usize {
    runs.iter()
        .filter(|run| run.pointer("/status/state").and_then(Value::as_str) == Some(state))
        .count()
}

fn json_text(row: &rusqlite::Row<'_>, column: &str, fallback: Value) -> rusqlite::Result<Value> {
    let raw: String = row.get(column)?;
    Ok(serde_json::from_str(&raw).unwrap_or(fallback))
}

fn parse_json_column<T>(row: &rusqlite::Row<'_>, column: &str, fallback: T) -> rusqlite::Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let raw: String = row.get(column)?;
    Ok(serde_json::from_str(&raw).unwrap_or(fallback))
}

fn json_optional_text(row: &rusqlite::Row<'_>, column: &str) -> rusqlite::Result<Option<Value>> {
    let raw: Option<String> = row.get(column)?;
    Ok(raw.and_then(|value| serde_json::from_str(&value).ok()))
}

fn effective_node_state(stored_state: &str, last_heartbeat_at: &str) -> String {
    if !matches!(stored_state, "online" | "busy") {
        return stored_state.to_string();
    }
    let Ok(last_seen) = chrono::DateTime::parse_from_rfc3339(last_heartbeat_at) else {
        return "offline".to_string();
    };
    let age = Utc::now()
        .signed_duration_since(last_seen.with_timezone(&Utc))
        .num_seconds();
    if age > HEARTBEAT_OFFLINE_AFTER_SECONDS {
        "offline".to_string()
    } else if age > HEARTBEAT_UNKNOWN_AFTER_SECONDS {
        "unknown".to_string()
    } else {
        stored_state.to_string()
    }
}

fn node_success_rate(success_count: i64, failure_count: i64) -> f64 {
    let total = success_count + failure_count;
    if total <= 0 {
        100.0
    } else {
        (success_count as f64 / total as f64 * 100.0).clamp(0.0, 100.0)
    }
}

fn required_string(data: &Value, key: &str) -> anyhow::Result<String> {
    optional_string(data, key)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("{key} is required"))
}

fn optional_string(data: &Value, key: &str) -> Option<String> {
    data.get(key)
        .and_then(Value::as_str)
        .map(|value| value.trim().to_string())
}

fn optional_json_value_string(data: &Value, key: &str) -> anyhow::Result<Option<String>> {
    data.get(key)
        .filter(|value| !value.is_null())
        .map(serde_json::to_string)
        .transpose()
        .map_err(Into::into)
}

fn string_or(data: &Value, key: &str, default: &str) -> String {
    optional_string(data, key).unwrap_or_else(|| default.to_string())
}

fn array_field(data: &Value, key: &str) -> Vec<String> {
    match data.get(key) {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect(),
        Some(Value::String(value)) => value
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(ToString::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

fn normalize_agent_node_scope(value: Option<&Value>) -> Value {
    let mut scope = value.cloned().unwrap_or_else(|| {
        json!({
            "mode": "none",
            "nodes": [],
            "groups": [],
            "os": []
        })
    });
    let Some(map) = scope.as_object_mut() else {
        return json!({
            "mode": "none",
            "nodes": [],
            "groups": [],
            "os": []
        });
    };
    let mode = map
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("none")
        .to_ascii_lowercase();
    let mode = match mode.as_str() {
        "all" | "nodes" | "group" | "groups" | "os" | "none" => mode,
        _ => "none".to_string(),
    };
    map.insert("mode".to_string(), json!(mode));
    ensure_json_array_field(map, "nodes");
    ensure_json_array_field(map, "groups");
    ensure_json_array_field(map, "os");
    scope
}

fn normalize_agent_tool_scope(value: Option<&Value>) -> Value {
    let mut scope = value.cloned().unwrap_or_else(|| {
        json!({
            "mode": "declared",
            "tools": []
        })
    });
    let Some(map) = scope.as_object_mut() else {
        return json!({
            "mode": "declared",
            "tools": []
        });
    };
    let mode = map
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("declared")
        .to_ascii_lowercase();
    let mode = match mode.as_str() {
        "all" | "tools" | "declared" | "none" => mode,
        _ => "declared".to_string(),
    };
    map.insert("mode".to_string(), json!(mode));
    ensure_json_array_field(map, "tools");
    scope
}

fn ensure_json_array_field(map: &mut serde_json::Map<String, Value>, key: &str) {
    if !map.get(key).is_some_and(Value::is_array) {
        map.insert(key.to_string(), json!([]));
    }
}

fn ensure_object(value: &mut Value) {
    if !value.is_object() {
        *value = json!({});
    }
}

fn string_array_from_value(value: &Value) -> Vec<String> {
    match value {
        Value::Array(items) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(ToString::to_string)
            .collect(),
        Value::String(value) => value
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(ToString::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

fn merge_json_object(target: &mut Value, patch: Value) {
    let (Some(target), Some(patch)) = (target.as_object_mut(), patch.as_object()) else {
        return;
    };
    for (key, value) in patch {
        if value.is_object() {
            match target.get_mut(key) {
                Some(existing) if existing.is_object() => {
                    merge_json_object(existing, value.clone())
                }
                _ => {
                    target.insert(key.clone(), value.clone());
                }
            }
        } else {
            target.insert(key.clone(), value.clone());
        }
    }
}

fn merge_json_defaults(target: &mut Value, defaults: Value) {
    let (Some(target), Some(defaults)) = (target.as_object_mut(), defaults.as_object()) else {
        return;
    };
    for (key, value) in defaults {
        match target.get_mut(key) {
            Some(existing) if existing.is_object() && value.is_object() => {
                merge_json_defaults(existing, value.clone())
            }
            Some(existing) if existing.is_array() && value.is_array() => {
                let Some(existing_items) = existing.as_array_mut() else {
                    continue;
                };
                for item in value.as_array().unwrap_or(&Vec::new()) {
                    if !existing_items.iter().any(|existing| existing == item) {
                        existing_items.push(item.clone());
                    }
                }
            }
            Some(_) => {}
            None => {
                target.insert(key.clone(), value.clone());
            }
        }
    }
}

fn number_or(data: &Value, key: &str, default: i64) -> i64 {
    data.get(key).and_then(Value::as_i64).unwrap_or(default)
}

fn float_or(data: &Value, key: &str, default: f64) -> f64 {
    data.get(key).and_then(Value::as_f64).unwrap_or(default)
}

fn bool_or(data: &Value, key: &str, default: bool) -> bool {
    data.get(key).and_then(Value::as_bool).unwrap_or(default)
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    fn not_found(message: &str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: message.to_string(),
        }
    }

    fn bad_request(message: &str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "bad_request",
            message: message.to_string(),
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        let message = error.to_string();
        let status = if message.contains("not found") {
            StatusCode::NOT_FOUND
        } else if message.contains("required") || message.contains("unknown task action") {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        };
        Self {
            status,
            code: if status == StatusCode::INTERNAL_SERVER_ERROR {
                "internal_error"
            } else if status == StatusCode::NOT_FOUND {
                "not_found"
            } else {
                "bad_request"
            },
            message,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({
                "ok": false,
                "error": {
                    "code": self.code,
                    "message": self.message
                }
            })),
        )
            .into_response()
    }
}
