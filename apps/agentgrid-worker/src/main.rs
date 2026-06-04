use std::{
    collections::{HashMap, HashSet},
    env, fs,
    io::{Read, Write},
    net::{IpAddr, UdpSocket},
    path::{Path, PathBuf},
    process::{Command as StdCommand, Stdio},
    sync::{
        mpsc::{self as std_mpsc, RecvTimeoutError, Sender},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use agentgrid_executor::execute_with_cancel_and_logs;
use agentgrid_protocol::{
    AgentMessagePayload, BrowserPayload, CommandPayload, DesktopPayload, DockerPayload,
    FilePayload, GitPayload, HttpRequestPayload, JobPayload, JobResult, PluginPayload,
    SessionPayload,
};
use anyhow::{Context, Result};
use base64::Engine as _;
use clap::Parser;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use futures_util::{SinkExt, StreamExt};
use portable_pty::{native_pty_system, Child as PtyChild, CommandBuilder, MasterPty, PtySize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sysinfo::{Disks, System};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{mpsc, Mutex as AsyncMutex},
};
use tokio_tungstenite::{connect_async, tungstenite::Message};

const WORKER_VERSION: &str = env!("CARGO_PKG_VERSION");
const TASK_LEASE_SECONDS: u64 = 120;
const TASK_LEASE_RENEW_INTERVAL_SECONDS: u64 = 45;
const CONTROL_WS_PING_INTERVAL_SECONDS: u64 = 15;
const CONTROL_WS_STALE_AFTER_SECONDS: u64 = 45;
const CONTROL_WS_SEND_TIMEOUT_SECONDS: u64 = 10;
const CONTROL_WS_RECONNECT_MIN_SECONDS: u64 = 2;
const CONTROL_WS_RECONNECT_MAX_SECONDS: u64 = 30;
const CONTROL_WS_STABLE_RESET_SECONDS: u64 = 60;

#[derive(Debug, Clone)]
struct SecurityPolicy {
    http: HttpPolicy,
    command: CommandPolicy,
}

#[derive(Debug, Clone)]
struct HttpPolicy {
    allowed_domains: Vec<String>,
    blocked_ips: Vec<String>,
    allow_private_network: bool,
    max_response_bytes: u64,
}

#[derive(Debug, Clone)]
struct CommandPolicy {
    enabled: bool,
    command_allowlist: Vec<String>,
}

#[derive(Debug, Parser)]
#[command(name = "agentgrid-worker")]
#[command(about = "AgentGrid Rust worker node")]
struct Cli {
    #[arg(long, default_value = "http://127.0.0.1:20181")]
    hub: String,
    #[arg(long)]
    id: Option<String>,
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    join_token: Option<String>,
    #[arg(long)]
    machine_fingerprint: Option<String>,
    #[arg(long, default_value_t = 10)]
    interval_seconds: u64,
    #[arg(long, default_value_t = 4)]
    max_concurrent_jobs: usize,
    #[arg(long = "tag")]
    tags: Vec<String>,
    #[arg(long = "capability")]
    capabilities: Vec<String>,
    #[arg(long, env = "AGENTGRID_CHANNEL_ROLE")]
    channel_role: Option<String>,
    #[arg(long, default_value_t = false)]
    once: bool,
    #[arg(long, default_value_t = 300)]
    update_interval_seconds: u64,
    #[arg(long, default_value_t = true)]
    auto_update: bool,
    #[arg(long, default_value_t = false)]
    no_auto_update: bool,
    #[arg(long, default_value = "stable")]
    update_channel: String,
    #[arg(long)]
    update_public_key: Option<String>,
    #[arg(long, default_value_t = false)]
    require_update_signature: bool,
    #[arg(long)]
    journal_path: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    no_journal: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let node_id = cli.id.unwrap_or_else(default_node_id);
    let node_name = cli.name.unwrap_or_else(|| node_id.clone());
    let join_token = cli
        .join_token
        .or_else(|| env::var("AGENTGRID_JOIN_TOKEN").ok())
        .or_else(|| env::var("AG_JOIN_TOKEN").ok())
        .filter(|value| !value.trim().is_empty());
    let machine_fingerprint = cli
        .machine_fingerprint
        .unwrap_or_else(default_machine_fingerprint);
    let tags = if cli.tags.is_empty() {
        vec![std::env::consts::OS.to_string()]
    } else {
        cli.tags
    };
    let channel_role = normalize_node_channel_role(cli.channel_role.as_deref(), &node_id);
    let capabilities = if cli.capabilities.is_empty() {
        default_capabilities_for_channel(&channel_role)
    } else {
        cli.capabilities
    };
    let client = reqwest::blocking::Client::new();
    let running = Arc::new(Mutex::new(HashSet::<String>::new()));
    let base = cli.hub.trim_end_matches('/').to_string();
    let journal = if cli.no_journal {
        None
    } else {
        Some(Arc::new(ExecutionJournal::new(
            cli.journal_path
                .clone()
                .unwrap_or_else(|| default_journal_path(&node_id)),
            node_id.clone(),
        )?))
    };
    if let Some(journal) = journal.as_ref() {
        if let Err(error) = reconcile_journal(&client, &base, &node_id, journal) {
            eprintln!("worker journal reconcile failed: {error:#}");
        }
    }
    start_terminal_agent(base.clone(), node_id.clone());
    start_bridge_agent(base.clone(), node_id.clone());
    if cli.auto_update && !cli.no_auto_update && !cli.once {
        start_auto_update_agent(
            base.clone(),
            node_id.clone(),
            cli.update_interval_seconds.max(30),
            cli.update_channel.clone(),
            update_public_key_from_config(cli.update_public_key.as_deref()),
            cli.require_update_signature || env_truthy("AGENTGRID_REQUIRE_UPDATE_SIGNATURE"),
        );
    }
    let mut policy = fetch_policy(&client, &base).unwrap_or_else(|error| {
        eprintln!("policy fetch failed, using locked down defaults: {error:#}");
        locked_down_policy()
    });

    loop {
        let running_count = running.lock().expect("running lock").len();
        let report = collect_report(
            &node_id,
            &node_name,
            &tags,
            &capabilities,
            running_count,
            cli.max_concurrent_jobs,
            cli.auto_update && !cli.no_auto_update,
            &cli.update_channel,
            &machine_fingerprint,
            &channel_role,
            join_token.as_deref(),
        );
        if let Err(error) = client
            .post(format!("{base}/api/nodes"))
            .json(&report)
            .send()
            .and_then(|response| response.error_for_status())
        {
            eprintln!("heartbeat failed: {error}");
            if cli.once {
                anyhow::bail!("heartbeat failed: {error}");
            }
            thread::sleep(Duration::from_secs(cli.interval_seconds.max(2)));
            continue;
        }
        if let Ok(next_policy) = fetch_policy(&client, &base) {
            policy = next_policy;
        }

        let capacity = cli.max_concurrent_jobs.saturating_sub(running_count);
        if capacity > 0 {
            let lease_response = client
                .post(format!("{base}/api/worker/lease"))
                .json(&json!({
                    "node_id": node_id,
                    "max_tasks": capacity,
                    "lease_seconds": TASK_LEASE_SECONDS,
                    "capabilities": capabilities,
                    "machine_fingerprint": machine_fingerprint,
                    "join_token": join_token
                }))
                .send()
                .and_then(|response| response.error_for_status());
            let lease = match lease_response {
                Ok(response) => response.json::<Value>()?,
                Err(error) => {
                    eprintln!("lease request failed: {error}");
                    if cli.once {
                        anyhow::bail!("lease request failed: {error}");
                    }
                    thread::sleep(Duration::from_secs(cli.interval_seconds.max(2)));
                    continue;
                }
            };
            for task in lease
                .get("tasks")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
            {
                let task_id = task
                    .pointer("/metadata/id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if task_id.is_empty() {
                    continue;
                }
                if let Some(journal) = journal.as_ref() {
                    journal.record_task_event("leased", &task_id, Some(&task), None);
                }
                running
                    .lock()
                    .expect("running lock")
                    .insert(task_id.clone());
                let base = base.clone();
                let node_id = node_id.clone();
                let policy = policy.clone();
                let running = Arc::clone(&running);
                let journal = journal.clone();
                thread::spawn(move || {
                    if let Some(journal) = journal.as_ref() {
                        journal.record_task_event("started", &task_id, Some(&task), None);
                    }
                    let lease_renew_stop = start_lease_renewal(
                        base.clone(),
                        node_id.clone(),
                        task_id.clone(),
                        TASK_LEASE_SECONDS,
                        TASK_LEASE_RENEW_INTERVAL_SECONDS,
                        journal.clone(),
                    );
                    let _ = report_checkpoint(&base, &node_id, &task, 1, 1, "started", None);
                    let result = run_task(&task, &policy, &base, &task_id)
                        .and_then(|result| {
                            let _ = report_checkpoint(
                                &base,
                                &node_id,
                                &task,
                                100,
                                100,
                                "completed",
                                Some(&result),
                            );
                            complete_task(&base, &task_id, &node_id, result)
                        })
                        .or_else(|error| fail_task(&base, &task_id, &node_id, error));
                    let _ = lease_renew_stop.send(());
                    match result {
                        Ok(()) => {
                            if let Some(journal) = journal.as_ref() {
                                journal.record_task_event("reported", &task_id, None, None);
                            }
                        }
                        Err(error) => {
                            if let Some(journal) = journal.as_ref() {
                                journal.record_task_event(
                                    "report_failed",
                                    &task_id,
                                    None,
                                    Some(json!({ "message": error.to_string() })),
                                );
                            }
                            eprintln!("task {task_id} report failed: {error:#}");
                        }
                    }
                    running.lock().expect("running lock").remove(&task_id);
                });
            }
        }

        if cli.once {
            while !running.lock().expect("running lock").is_empty() {
                thread::sleep(Duration::from_millis(100));
            }
            break;
        }
        thread::sleep(Duration::from_secs(cli.interval_seconds.max(2)));
    }

    Ok(())
}

struct ExecutionJournal {
    path: PathBuf,
    node_id: String,
    lock: Mutex<()>,
}

impl ExecutionJournal {
    fn new(path: PathBuf, node_id: String) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create journal dir {}", parent.display()))?;
        }
        Ok(Self {
            path,
            node_id,
            lock: Mutex::new(()),
        })
    }

    fn record_task_event(
        &self,
        event: &str,
        task_id: &str,
        task: Option<&Value>,
        detail: Option<Value>,
    ) {
        if let Err(error) = self.try_record_task_event(event, task_id, task, detail) {
            eprintln!("execution journal write failed: {error:#}");
        }
    }

    fn try_record_task_event(
        &self,
        event: &str,
        task_id: &str,
        task: Option<&Value>,
        detail: Option<Value>,
    ) -> Result<()> {
        let _guard = self.lock.lock().expect("journal lock");
        let record = json!({
            "api_version": "agentgrid.worker-journal/v1",
            "kind": "WorkerExecutionJournalRecord",
            "time": now_rfc3339(),
            "node_id": self.node_id,
            "event": event,
            "task_id": task_id,
            "job_id": task.and_then(|item| item.pointer("/metadata/job_id")).cloned().unwrap_or(Value::Null),
            "job_attempt_id": task.and_then(|item| item.pointer("/metadata/job_attempt_id")).cloned().unwrap_or(Value::Null),
            "job_shard_id": task.and_then(|item| item.pointer("/metadata/job_shard_id")).cloned().unwrap_or(Value::Null),
            "lease_expires_at": task.and_then(|item| item.pointer("/status/lease_expires_at")).cloned().unwrap_or(Value::Null),
            "detail": detail.unwrap_or_else(|| json!({}))
        });
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("failed to open journal {}", self.path.display()))?;
        serde_json::to_writer(&mut file, &record)?;
        file.write_all(b"\n")?;
        Ok(())
    }

    fn recent_records(&self, limit: usize) -> Result<Vec<Value>> {
        let _guard = self.lock.lock().expect("journal lock");
        let Ok(content) = fs::read_to_string(&self.path) else {
            return Ok(Vec::new());
        };
        let mut records = content
            .lines()
            .rev()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .take(limit)
            .collect::<Vec<_>>();
        records.reverse();
        Ok(records)
    }
}

fn reconcile_journal(
    client: &reqwest::blocking::Client,
    base: &str,
    node_id: &str,
    journal: &ExecutionJournal,
) -> Result<()> {
    let records = journal.recent_records(200)?;
    if records.is_empty() {
        return Ok(());
    }
    let response = client
        .post(format!("{base}/api/worker/reconcile"))
        .json(&json!({
            "node_id": node_id,
            "records": records
        }))
        .send()?
        .error_for_status()?
        .json::<Value>()?;
    let needs_attention = response
        .get("needs_attention")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    if needs_attention > 0 {
        eprintln!("worker journal reconcile found {needs_attention} records needing attention");
    }
    Ok(())
}

fn start_lease_renewal(
    base: String,
    node_id: String,
    task_id: String,
    lease_seconds: u64,
    interval_seconds: u64,
    journal: Option<Arc<ExecutionJournal>>,
) -> Sender<()> {
    let (stop_tx, stop_rx) = std_mpsc::channel::<()>();
    thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        let interval = Duration::from_secs(interval_seconds.clamp(5, lease_seconds.max(10)));
        loop {
            match stop_rx.recv_timeout(interval) {
                Ok(_) | Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {
                    match renew_task_lease(&client, &base, &task_id, &node_id, lease_seconds) {
                        Ok(lease_expires_at) => {
                            if let Some(journal) = journal.as_ref() {
                                journal.record_task_event(
                                    "lease_renewed",
                                    &task_id,
                                    None,
                                    Some(json!({
                                        "lease_seconds": lease_seconds,
                                        "lease_expires_at": lease_expires_at
                                    })),
                                );
                            }
                        }
                        Err(error) => {
                            if let Some(journal) = journal.as_ref() {
                                journal.record_task_event(
                                    "lease_renew_failed",
                                    &task_id,
                                    None,
                                    Some(json!({ "message": error.to_string() })),
                                );
                            }
                            eprintln!("task {task_id} lease renew failed: {error:#}");
                        }
                    }
                }
            }
        }
    });
    stop_tx
}

fn renew_task_lease(
    client: &reqwest::blocking::Client,
    base: &str,
    task_id: &str,
    node_id: &str,
    lease_seconds: u64,
) -> Result<String> {
    let response = client
        .post(format!("{base}/api/worker/tasks/{task_id}/renew"))
        .json(&json!({
            "node_id": node_id,
            "lease_seconds": lease_seconds
        }))
        .send()?
        .error_for_status()?
        .json::<Value>()?;
    response
        .get("lease_expires_at")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| anyhow::anyhow!("Hub renew response missing lease_expires_at"))
}

fn run_task(task: &Value, policy: &SecurityPolicy, base: &str, task_id: &str) -> Result<Value> {
    if task
        .pointer("/status/control/action")
        .and_then(Value::as_str)
        == Some("stop")
    {
        return Ok(json!({
            "type": "error",
            "code": "task_stopped",
            "message": "task stopped before execution",
            "retryable": false
        }));
    }
    let payload = parse_task_payload(task)?;
    enforce_policy(&payload, policy)?;
    let client = reqwest::blocking::Client::new();
    let log_client = reqwest::blocking::Client::new();
    let control_base = base.to_string();
    let control_task_id = task_id.to_string();
    let log_base = base.to_string();
    let log_task_id = task_id.to_string();
    let log_node_id = task
        .pointer("/status/leased_by_node_id")
        .and_then(Value::as_str)
        .unwrap_or("worker")
        .to_string();
    let log = Arc::new(move |stream: &str, line: &str| {
        let _ = report_task_log(
            &log_client,
            &log_base,
            &log_task_id,
            &log_node_id,
            stream,
            line,
        );
    });
    let result = match payload {
        JobPayload::AgentMessage(message) => deliver_agent_message(task, message)?,
        other => execute_with_cancel_and_logs(
            &other,
            move || task_should_stop(&client, &control_base, &control_task_id).unwrap_or(false),
            log,
        )?,
    };
    Ok(serde_json::to_value(result)?)
}

fn task_should_stop(client: &reqwest::blocking::Client, base: &str, task_id: &str) -> Result<bool> {
    let response = client
        .get(format!("{base}/api/worker/tasks/{task_id}/control"))
        .send()?
        .error_for_status()?
        .json::<Value>()?;
    let action = response
        .pointer("/control/action")
        .and_then(Value::as_str)
        .unwrap_or("");
    let state = response.get("state").and_then(Value::as_str).unwrap_or("");
    Ok(action == "stop" || matches!(state, "stopping" | "cancelled"))
}

fn report_task_log(
    client: &reqwest::blocking::Client,
    base: &str,
    task_id: &str,
    node_id: &str,
    stream: &str,
    line: &str,
) -> Result<()> {
    client
        .post(format!("{base}/api/worker/tasks/{task_id}/logs"))
        .json(&json!({
            "node_id": node_id,
            "stream": stream,
            "line": line
        }))
        .send()?
        .error_for_status()?;
    Ok(())
}

fn complete_task(base: &str, task_id: &str, node_id: &str, result: Value) -> Result<()> {
    if result.get("type").and_then(Value::as_str) == Some("error") {
        return fail_task_with_error(base, task_id, node_id, result);
    }
    if is_non_zero_exit_result(&result) {
        return fail_task_with_error(
            base,
            task_id,
            node_id,
            json!({
                "code": "process_exit_non_zero",
                "message": format!(
                    "{} exited with code {}",
                    result.get("type").and_then(Value::as_str).unwrap_or("process"),
                    result.get("exit_code").and_then(Value::as_i64).unwrap_or(-1)
                ),
                "retryable": false,
                "result": result
            }),
        );
    }
    reqwest::blocking::Client::new()
        .post(format!("{base}/api/worker/tasks/{task_id}/complete"))
        .json(&json!({ "node_id": node_id, "result": result }))
        .send()?
        .error_for_status()?;
    Ok(())
}

fn report_checkpoint(
    base: &str,
    node_id: &str,
    task: &Value,
    sequence: i64,
    progress: i64,
    stage: &str,
    result: Option<&Value>,
) -> Result<()> {
    let Some(job_id) = task.pointer("/metadata/job_id").and_then(Value::as_str) else {
        return Ok(());
    };
    let task_id = task
        .pointer("/metadata/id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let attempt_id = task
        .pointer("/metadata/job_attempt_id")
        .and_then(Value::as_str);
    let shard_id = task
        .pointer("/metadata/job_shard_id")
        .and_then(Value::as_str);
    reqwest::blocking::Client::new()
        .post(format!("{base}/api/jobs/{job_id}/checkpoints"))
        .json(&json!({
            "attempt_id": attempt_id,
            "task_id": task_id,
            "node_id": node_id,
            "sequence": sequence,
            "progress": progress,
            "resume_token": {
                "stage": stage,
                "task_id": task_id,
                "attempt_id": attempt_id,
                "shard_id": shard_id,
                "result_type": result.and_then(|item| item.get("type")).and_then(Value::as_str)
            },
            "artifacts": result.and_then(|item| item.get("artifacts")).cloned().unwrap_or_else(|| json!([]))
        }))
        .send()?
        .error_for_status()?;
    Ok(())
}

fn is_non_zero_exit_result(result: &Value) -> bool {
    matches!(
        result.get("type").and_then(Value::as_str),
        Some("command_result" | "git_result" | "docker_result" | "session_result")
    ) && result
        .get("exit_code")
        .and_then(Value::as_i64)
        .map(|code| code != 0)
        .unwrap_or(false)
}

fn fail_task(base: &str, task_id: &str, node_id: &str, error: anyhow::Error) -> Result<()> {
    fail_task_with_error(
        base,
        task_id,
        node_id,
        json!({
            "code": "worker_error",
            "message": error.to_string()
        }),
    )
}

fn fail_task_with_error(base: &str, task_id: &str, node_id: &str, error: Value) -> Result<()> {
    reqwest::blocking::Client::new()
        .post(format!("{base}/api/worker/tasks/{task_id}/fail"))
        .json(&json!({
            "node_id": node_id,
            "message": error.get("message").and_then(Value::as_str).unwrap_or("worker task failed"),
            "error": error
        }))
        .send()?
        .error_for_status()?;
    Ok(())
}

fn default_journal_path(node_id: &str) -> PathBuf {
    let base_dir = env::var_os("AGENTGRID_WORKER_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("XDG_DATA_HOME").map(|path| PathBuf::from(path).join("agentgrid")))
        .unwrap_or_else(|| {
            env::var_os("HOME")
                .map(|home| Path::new(&home).join(".agentgrid"))
                .unwrap_or_else(|| PathBuf::from(".agentgrid"))
        });
    base_dir
        .join("worker")
        .join("journal")
        .join(format!("{node_id}.jsonl"))
}

fn now_rfc3339() -> String {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => format!("{}.{:09}Z", duration.as_secs(), duration.subsec_nanos()),
        Err(_) => "0.000000000Z".to_string(),
    }
}

fn reconnect_delay(attempt: u32) -> Duration {
    let capped_attempt = attempt.min(5);
    let seconds = (CONTROL_WS_RECONNECT_MIN_SECONDS * 2_u64.pow(capped_attempt))
        .min(CONTROL_WS_RECONNECT_MAX_SECONDS);
    Duration::from_secs(seconds)
}

fn start_auto_update_agent(
    base: String,
    node_id: String,
    interval_seconds: u64,
    update_channel: String,
    update_public_key: Option<String>,
    require_signature: bool,
) {
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(10));
        loop {
            match check_and_apply_worker_update(
                &base,
                &node_id,
                &update_channel,
                update_public_key.as_deref(),
                require_signature,
            ) {
                Ok(false) => {}
                Ok(true) => return,
                Err(error) => eprintln!("auto update check failed: {error:#}"),
            }
            thread::sleep(Duration::from_secs(interval_seconds));
        }
    });
}

fn check_and_apply_worker_update(
    base: &str,
    node_id: &str,
    update_channel: &str,
    configured_public_key: Option<&str>,
    require_signature: bool,
) -> Result<bool> {
    let current_exe = env::current_exe().context("current executable path unavailable")?;
    let current_bytes = fs::read(&current_exe).context("read current worker binary")?;
    let current_sha256 = sha256_hex(&current_bytes);
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let glibc = glibc_version();
    let target = worker_target();
    let client = reqwest::blocking::Client::new();
    let manifest = client
        .get(format!("{base}/api/worker/update-manifest"))
        .query(&json!({
            "os": os,
            "arch": arch,
            "current_sha256": current_sha256,
            "glibc_version": glibc,
            "worker_target": target,
            "node_id": node_id,
            "channel": update_channel
        }))
        .send()?
        .error_for_status()?
        .json::<Value>()?;
    if manifest.get("update_available").and_then(Value::as_bool) != Some(true) {
        eprintln!(
            "worker update check: no update node={} target={} compatible={}",
            node_id,
            target,
            manifest
                .get("compatible")
                .and_then(Value::as_bool)
                .unwrap_or(true)
        );
        return Ok(false);
    }
    let sha256 = manifest
        .get("sha256")
        .and_then(Value::as_str)
        .context("update manifest sha256 missing")?;
    let download_url = manifest
        .get("download_url")
        .and_then(Value::as_str)
        .context("update manifest download_url missing")?;
    let download = if download_url.starts_with("http://") || download_url.starts_with("https://") {
        download_url.to_string()
    } else {
        format!("{base}{download_url}")
    };
    eprintln!(
        "worker update available: current={} target={} version={}",
        &current_sha256[..12.min(current_sha256.len())],
        &sha256[..12.min(sha256.len())],
        manifest
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or(WORKER_VERSION)
    );
    let bytes = client
        .get(download)
        .send()?
        .error_for_status()?
        .bytes()?
        .to_vec();
    let downloaded_sha256 = sha256_hex(&bytes);
    if downloaded_sha256 != sha256 {
        anyhow::bail!("downloaded worker sha256 mismatch");
    }
    verify_worker_update_signature_from_manifest(
        &manifest,
        &bytes,
        configured_public_key,
        require_signature,
    )?;
    install_worker_update(&current_exe, &bytes)?;
    restart_current_process(&current_exe)?;
    Ok(true)
}

fn update_public_key_from_config(cli_value: Option<&str>) -> Option<String> {
    cli_value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            env::var("AGENTGRID_WORKER_UPDATE_PUBLIC_KEY")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
}

fn env_truthy(name: &str) -> bool {
    env::var(name)
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
}

fn verify_worker_update_signature_from_manifest(
    manifest: &Value,
    bytes: &[u8],
    configured_public_key: Option<&str>,
    require_signature: bool,
) -> Result<()> {
    let manifest_requires_signature = manifest
        .get("signature_required")
        .or_else(|| manifest.pointer("/signing/required"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let must_verify = require_signature || manifest_requires_signature;
    let signature = manifest
        .get("signature")
        .or_else(|| manifest.pointer("/signing/signature"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let public_key = configured_public_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            if must_verify {
                return None;
            }
            manifest
                .get("signing_public_key")
                .or_else(|| manifest.pointer("/signing/public_key"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
        });

    match (signature, public_key) {
        (Some(signature), Some(public_key)) => {
            verify_worker_update_signature(public_key, bytes, signature)?;
            eprintln!("worker update signature verified with ed25519");
            Ok(())
        }
        (Some(_), None) if must_verify => {
            anyhow::bail!("worker update signature present but no public key configured")
        }
        (None, _) if must_verify => anyhow::bail!("worker update signature required but missing"),
        (Some(_), None) => {
            eprintln!("worker update signature skipped: no public key configured");
            Ok(())
        }
        (None, _) => {
            eprintln!("worker update signature not provided; sha256-only compatibility mode");
            Ok(())
        }
    }
}

fn verify_worker_update_signature(
    public_key_b64: &str,
    bytes: &[u8],
    signature_b64: &str,
) -> Result<()> {
    let public_key_bytes = base64::engine::general_purpose::STANDARD
        .decode(public_key_b64.trim())
        .context("decode ed25519 public key")?;
    let key_bytes: [u8; 32] = public_key_bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("ed25519 public key must be 32 bytes"))?;
    let signature_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature_b64.trim())
        .context("decode ed25519 signature")?;
    let signature_array: [u8; 64] = signature_bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("ed25519 signature must be 64 bytes"))?;
    let verifying_key = VerifyingKey::from_bytes(&key_bytes)?;
    let signature = Signature::from_bytes(&signature_array);
    verifying_key
        .verify(bytes, &signature)
        .context("verify ed25519 worker update signature")
}

fn install_worker_update(current_exe: &PathBuf, bytes: &[u8]) -> Result<()> {
    let parent = current_exe
        .parent()
        .context("current executable parent missing")?;
    let file_name = current_exe
        .file_name()
        .and_then(|value| value.to_str())
        .context("current executable filename missing")?;
    let update_path = parent.join(format!("{file_name}.update"));
    let backup_path = parent.join(format!("{file_name}.old"));
    {
        let mut file = fs::File::create(&update_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&update_path, fs::Permissions::from_mode(0o755))?;
    }
    let _ = fs::remove_file(&backup_path);
    let _ = fs::rename(current_exe, &backup_path);
    fs::rename(&update_path, current_exe)?;
    Ok(())
}

fn restart_current_process(current_exe: &PathBuf) -> Result<()> {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    StdCommand::new(current_exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("spawn updated worker")?;
    std::process::exit(0);
}

fn start_bridge_agent(base: String, node_id: String) {
    thread::spawn(move || {
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime,
            Err(error) => {
                eprintln!("bridge runtime start failed: {error}");
                return;
            }
        };
        runtime.block_on(async move {
            let mut reconnect_attempt = 0_u32;
            loop {
                let started_at = std::time::Instant::now();
                if let Err(error) = bridge_agent_loop(&base, &node_id).await {
                    eprintln!("bridge websocket disconnected: {error:#}");
                }
                if started_at.elapsed() >= Duration::from_secs(CONTROL_WS_STABLE_RESET_SECONDS) {
                    reconnect_attempt = 0;
                } else {
                    reconnect_attempt = reconnect_attempt.saturating_add(1);
                }
                tokio::time::sleep(reconnect_delay(reconnect_attempt)).await;
            }
        });
    });
}

async fn bridge_agent_loop(base: &str, node_id: &str) -> Result<()> {
    let url = bridge_ws_url(base, node_id);
    let (socket, _) = connect_async(&url).await?;
    let (mut sink, mut stream) = socket.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Value>();
    let sessions = Arc::new(AsyncMutex::new(HashMap::<
        String,
        mpsc::UnboundedSender<BridgePayload>,
    >::new()));
    let mut ping_interval =
        tokio::time::interval(Duration::from_secs(CONTROL_WS_PING_INTERVAL_SECONDS));
    let mut stale_timeout = Box::pin(tokio::time::sleep(Duration::from_secs(
        CONTROL_WS_STALE_AFTER_SECONDS,
    )));

    let send_task = tokio::spawn(async move {
        while let Some(value) = out_rx.recv().await {
            let send_result = tokio::time::timeout(
                Duration::from_secs(CONTROL_WS_SEND_TIMEOUT_SECONDS),
                sink.send(Message::Text(value.to_string())),
            )
            .await;
            if !matches!(send_result, Ok(Ok(()))) {
                break;
            }
        }
    });

    loop {
        tokio::select! {
            _ = ping_interval.tick() => {
                let _ = out_tx.send(json!({
                    "type": "bridge.worker_ping",
                    "node_id": node_id,
                    "ts": now_rfc3339()
                }));
            }
            _ = &mut stale_timeout => {
                sessions.lock().await.clear();
                send_task.abort();
                anyhow::bail!("bridge websocket stale: no hub response for {CONTROL_WS_STALE_AFTER_SECONDS}s");
            }
            message = stream.next() => {
                let Some(message) = message else {
                    break;
                };
                let message = message?;
                stale_timeout.as_mut().reset(tokio::time::Instant::now() + Duration::from_secs(CONTROL_WS_STALE_AFTER_SECONDS));
                match message {
                    Message::Text(text) => {
                        let value: Value = serde_json::from_str(&text)?;
                        if value.get("type").and_then(Value::as_str) == Some("bridge.worker_pong") {
                            continue;
                        }
                        handle_bridge_command(value, Arc::clone(&sessions), out_tx.clone()).await;
                    }
                    Message::Ping(payload) => {
                        let _ = out_tx.send(json!({
                            "type": "bridge.worker_pong",
                            "node_id": node_id,
                            "payload_base64": base64::engine::general_purpose::STANDARD.encode(payload),
                            "ts": now_rfc3339()
                        }));
                    }
                    Message::Pong(_) => {}
                    Message::Close(frame) => {
                        sessions.lock().await.clear();
                        send_task.abort();
                        anyhow::bail!("bridge websocket closed by hub: {frame:?}");
                    }
                    _ => {}
                }
            }
        }

        if send_task.is_finished() {
            sessions.lock().await.clear();
            send_task.abort();
            anyhow::bail!("bridge websocket send loop stopped");
        }
    }
    sessions.lock().await.clear();
    send_task.abort();
    Ok(())
}

async fn handle_bridge_command(
    value: Value,
    sessions: Arc<AsyncMutex<HashMap<String, mpsc::UnboundedSender<BridgePayload>>>>,
    out_tx: mpsc::UnboundedSender<Value>,
) {
    let session_id = value
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    match value.get("type").and_then(Value::as_str).unwrap_or("") {
        "bridge.open" => {
            let service_id = value
                .get("service_id")
                .and_then(Value::as_str)
                .unwrap_or("");
            if service_id != "codex.local" {
                let _ = out_tx.send(json!({
                    "type": "bridge.error",
                    "session_id": session_id,
                    "message": "unsupported local service"
                }));
                return;
            }
            let _ = out_tx.send(json!({
            "type": "bridge.ready",
            "session_id": session_id,
            "service_id": service_id
            }));
        }
        "bridge.request" => {
            let service_id = value
                .get("service_id")
                .and_then(Value::as_str)
                .unwrap_or("");
            if service_id != "codex.local" {
                let _ = out_tx.send(json!({
                    "type": "bridge.error",
                    "session_id": session_id,
                    "message": "unsupported local service"
                }));
                return;
            }
            let response = match forward_codex_local_request(&value).await {
                Ok(response) => response,
                Err(error) => json!({
                "type": "bridge.error",
                "session_id": session_id,
                "service_id": service_id,
                "message": error.to_string()
                }),
            };
            let _ = out_tx.send(response);
        }
        "bridge.websocket.open" => {
            let service_id = value
                .get("service_id")
                .and_then(Value::as_str)
                .unwrap_or("");
            if service_id != "codex.local" {
                let _ = out_tx.send(json!({
                    "type": "bridge.error",
                    "session_id": session_id,
                    "message": "unsupported local service"
                }));
                return;
            }
            if let Err(error) =
                open_codex_websocket_session(&session_id, &value, sessions, out_tx.clone()).await
            {
                let _ = out_tx.send(json!({
                    "type": "bridge.error",
                    "session_id": session_id,
                    "service_id": service_id,
                    "message": error.to_string()
                }));
            }
        }
        "bridge.websocket.message" => {
            let payload = bridge_websocket_payload(&value);
            let writer = {
                let sessions = sessions.lock().await;
                sessions.get(&session_id).cloned()
            };
            if let Some(writer) = writer {
                let _ = writer.send(BridgePayload::Text(payload));
            } else {
                let _ = out_tx.send(json!({
                    "type": "bridge.error",
                    "session_id": session_id,
                    "service_id": "codex.local",
                    "message": "codex websocket session is not open"
                }));
            }
        }
        "bridge.websocket.close" | "bridge.close" => {
            sessions.lock().await.remove(&session_id);
            let _ = out_tx.send(json!({
            "type": "bridge.closed",
            "session_id": session_id,
            "service_id": "codex.local"
            }));
        }
        "port_bridge.start_source" => {
            start_port_bridge_source(value, Arc::clone(&sessions), out_tx.clone()).await;
        }
        "port_bridge.prepare_target" => {
            prepare_port_bridge_target(value, out_tx.clone()).await;
        }
        "port_bridge.open_target" => {
            open_port_bridge_target(value, Arc::clone(&sessions), out_tx.clone()).await;
        }
        "port_bridge.data" => {
            let bridge_id = value
                .get("bridge_id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let connection_id = value
                .get("connection_id")
                .and_then(Value::as_str)
                .unwrap_or("");
            let key = port_bridge_connection_key(&bridge_id, connection_id);
            let writer = {
                let sessions = sessions.lock().await;
                sessions.get(&key).cloned()
            };
            if let Some(writer) = writer {
                if let Some(bytes) = value
                    .get("data_base64")
                    .and_then(Value::as_str)
                    .and_then(|raw| base64::engine::general_purpose::STANDARD.decode(raw).ok())
                {
                    let _ = writer.send(BridgePayload::Bytes(bytes));
                }
            }
        }
        "port_bridge.close_connection" => {
            let bridge_id = value
                .get("bridge_id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let connection_id = value
                .get("connection_id")
                .and_then(Value::as_str)
                .unwrap_or("");
            sessions
                .lock()
                .await
                .remove(&port_bridge_connection_key(&bridge_id, connection_id));
        }
        "port_bridge.close" => {
            let bridge_id = value
                .get("bridge_id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            close_port_bridge_sessions(&bridge_id, Arc::clone(&sessions)).await;
            let _ = out_tx.send(json!({
                "type": "port_bridge.closed",
                "bridge_id": bridge_id
            }));
        }
        other => {
            let _ = out_tx.send(json!({
            "type": "bridge.error",
            "session_id": session_id,
            "message": format!("unsupported bridge message: {other}")
            }));
        }
    }
}

async fn open_codex_websocket_session(
    session_id: &str,
    value: &Value,
    sessions: Arc<AsyncMutex<HashMap<String, mpsc::UnboundedSender<BridgePayload>>>>,
    out_tx: mpsc::UnboundedSender<Value>,
) -> Result<()> {
    let path =
        sanitize_local_service_path(value.get("path").and_then(Value::as_str).unwrap_or("/"))?;
    let url = format!("ws://127.0.0.1:8390{path}");
    let (socket, _) = connect_async(&url).await?;
    let (mut local_sink, mut local_stream) = socket.split();
    let (local_tx, mut local_rx) = mpsc::unbounded_channel::<BridgePayload>();
    sessions
        .lock()
        .await
        .insert(session_id.to_string(), local_tx);

    let session_id_string = session_id.to_string();
    let out_tx_for_task = out_tx.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(payload) = local_rx.recv() => {
                    let sent = match payload {
                        BridgePayload::Text(text) => local_sink.send(Message::Text(text)).await,
                        BridgePayload::Bytes(bytes) => local_sink.send(Message::Binary(bytes)).await,
                    };
                    if sent.is_err() { break; }
                }
                message = local_stream.next() => {
                    match message {
                        Some(Ok(Message::Text(text))) => {
                            let _ = out_tx_for_task.send(json!({
                                "type": "bridge.websocket.message",
                                "session_id": session_id_string,
                                "service_id": "codex.local",
                                "body": text.to_string()
                            }));
                        }
                        Some(Ok(Message::Binary(bytes))) => {
                            let _ = out_tx_for_task.send(json!({
                                "type": "bridge.websocket.message",
                                "session_id": session_id_string,
                                "service_id": "codex.local",
                                "body_base64": base64::engine::general_purpose::STANDARD.encode(bytes)
                            }));
                        }
                        Some(Ok(Message::Close(_))) | None => break,
                        Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) | Some(Ok(Message::Frame(_))) => {}
                        Some(Err(error)) => {
                            let _ = out_tx_for_task.send(json!({
                                "type": "bridge.error",
                                "session_id": session_id_string,
                                "service_id": "codex.local",
                                "message": error.to_string()
                            }));
                            break;
                        }
                    }
                }
            }
        }
        let _ = out_tx_for_task.send(json!({
            "type": "bridge.websocket.closed",
            "session_id": session_id_string,
            "service_id": "codex.local"
        }));
    });

    let _ = out_tx.send(json!({
        "type": "bridge.websocket.ready",
        "session_id": session_id,
        "service_id": "codex.local"
    }));
    Ok(())
}

#[derive(Debug)]
enum BridgePayload {
    Text(String),
    Bytes(Vec<u8>),
}

async fn start_port_bridge_source(
    value: Value,
    sessions: Arc<AsyncMutex<HashMap<String, mpsc::UnboundedSender<BridgePayload>>>>,
    out_tx: mpsc::UnboundedSender<Value>,
) {
    let bridge_id = value
        .get("bridge_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let bind_host = value
        .get("source_bind_host")
        .and_then(Value::as_str)
        .unwrap_or("127.0.0.1");
    let bind_port = value
        .get("source_bind_port")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if bridge_id.is_empty() || bind_host != "127.0.0.1" || bind_port > u16::MAX as u64 {
        let _ = out_tx.send(json!({
            "type": "port_bridge.error",
            "bridge_id": bridge_id,
            "message": "invalid source port bridge request"
        }));
        return;
    }
    let address = format!("{bind_host}:{}", bind_port as u16);
    let listener = match TcpListener::bind(&address).await {
        Ok(listener) => listener,
        Err(error) => {
            let _ = out_tx.send(json!({
                "type": "port_bridge.error",
                "bridge_id": bridge_id,
                "message": format!("source bind failed: {error}")
            }));
            return;
        }
    };
    let actual_port = listener.local_addr().map(|addr| addr.port()).unwrap_or(0);
    let _ = out_tx.send(json!({
        "type": "port_bridge.source_ready",
        "bridge_id": bridge_id,
        "source_bind_host": bind_host,
        "source_bind_port": actual_port
    }));
    let sessions_for_task = Arc::clone(&sessions);
    let out_tx_for_task = out_tx.clone();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                break;
            };
            let connection_id = uuid_like_id();
            let key = port_bridge_connection_key(&bridge_id, &connection_id);
            let (writer_tx, writer_rx) = mpsc::unbounded_channel::<BridgePayload>();
            sessions_for_task
                .lock()
                .await
                .insert(key.clone(), writer_tx);
            let _ = out_tx_for_task.send(json!({
                "type": "port_bridge.open_target",
                "bridge_id": bridge_id,
                "connection_id": connection_id
            }));
            spawn_port_bridge_stream(
                "source",
                bridge_id.clone(),
                connection_id,
                stream,
                writer_rx,
                Arc::clone(&sessions_for_task),
                out_tx_for_task.clone(),
            );
        }
    });
}

async fn prepare_port_bridge_target(value: Value, out_tx: mpsc::UnboundedSender<Value>) {
    let bridge_id = value.get("bridge_id").and_then(Value::as_str).unwrap_or("");
    let _ = out_tx.send(json!({
        "type": "port_bridge.target_ready",
        "bridge_id": bridge_id
    }));
}

async fn open_port_bridge_target(
    value: Value,
    sessions: Arc<AsyncMutex<HashMap<String, mpsc::UnboundedSender<BridgePayload>>>>,
    out_tx: mpsc::UnboundedSender<Value>,
) {
    let bridge_id = value
        .get("bridge_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let connection_id = value
        .get("connection_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let target_host = value
        .get("target_host")
        .and_then(Value::as_str)
        .unwrap_or("127.0.0.1");
    let target_port = value
        .get("target_port")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if bridge_id.is_empty()
        || connection_id.is_empty()
        || !is_allowed_port_bridge_target_host_worker(target_host)
        || target_port == 0
        || target_port > u16::MAX as u64
    {
        let _ = out_tx.send(json!({
            "type": "port_bridge.error",
            "bridge_id": bridge_id,
            "connection_id": connection_id,
            "message": "invalid target port bridge request"
        }));
        return;
    }
    let address = format!("{target_host}:{}", target_port as u16);
    let stream = match TcpStream::connect(&address).await {
        Ok(stream) => stream,
        Err(error) => {
            let _ = out_tx.send(json!({
                "type": "port_bridge.error",
                "bridge_id": bridge_id,
                "connection_id": connection_id,
                "message": format!("target connect failed: {error}")
            }));
            return;
        }
    };
    let key = port_bridge_connection_key(&bridge_id, &connection_id);
    let (writer_tx, writer_rx) = mpsc::unbounded_channel::<BridgePayload>();
    sessions.lock().await.insert(key.clone(), writer_tx);
    spawn_port_bridge_stream(
        "target",
        bridge_id,
        connection_id,
        stream,
        writer_rx,
        sessions,
        out_tx,
    );
}

fn spawn_port_bridge_stream(
    side: &'static str,
    bridge_id: String,
    connection_id: String,
    stream: TcpStream,
    mut writer_rx: mpsc::UnboundedReceiver<BridgePayload>,
    sessions: Arc<AsyncMutex<HashMap<String, mpsc::UnboundedSender<BridgePayload>>>>,
    out_tx: mpsc::UnboundedSender<Value>,
) {
    tokio::spawn(async move {
        let (mut reader, mut writer) = stream.into_split();
        let bridge_id_for_read = bridge_id.clone();
        let connection_id_for_read = connection_id.clone();
        let out_tx_for_read = out_tx.clone();
        let read_task = tokio::spawn(async move {
            let mut buffer = vec![0u8; 16 * 1024];
            loop {
                match reader.read(&mut buffer).await {
                    Ok(0) => break,
                    Ok(size) => {
                        let data =
                            base64::engine::general_purpose::STANDARD.encode(&buffer[..size]);
                        let _ = out_tx_for_read.send(json!({
                            "type": "port_bridge.data",
                            "bridge_id": bridge_id_for_read,
                            "connection_id": connection_id_for_read,
                            "from": side,
                            "data_base64": data
                        }));
                    }
                    Err(error) => {
                        let _ = out_tx_for_read.send(json!({
                            "type": "port_bridge.error",
                            "bridge_id": bridge_id_for_read,
                            "connection_id": connection_id_for_read,
                            "message": format!("{side} read failed: {error}")
                        }));
                        break;
                    }
                }
            }
        });
        while let Some(payload) = writer_rx.recv().await {
            let bytes = match payload {
                BridgePayload::Text(text) => text.into_bytes(),
                BridgePayload::Bytes(bytes) => bytes,
            };
            if writer.write_all(&bytes).await.is_err() {
                break;
            }
        }
        read_task.abort();
        sessions
            .lock()
            .await
            .remove(&port_bridge_connection_key(&bridge_id, &connection_id));
        let _ = out_tx.send(json!({
            "type": "port_bridge.close_connection",
            "bridge_id": bridge_id,
            "connection_id": connection_id,
            "from": side
        }));
    });
}

async fn close_port_bridge_sessions(
    bridge_id: &str,
    sessions: Arc<AsyncMutex<HashMap<String, mpsc::UnboundedSender<BridgePayload>>>>,
) {
    let prefix = format!("pbridge:{bridge_id}:");
    sessions
        .lock()
        .await
        .retain(|key, _| !key.starts_with(&prefix));
}

fn port_bridge_connection_key(bridge_id: &str, connection_id: &str) -> String {
    format!("pbridge:{bridge_id}:{connection_id}")
}

fn uuid_like_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("conn_{nanos:x}")
}

fn is_allowed_port_bridge_target_host_worker(host: &str) -> bool {
    if matches!(host, "127.0.0.1" | "localhost" | "::1") {
        return true;
    }
    host.parse::<IpAddr>().ok().is_some_and(|ip| match ip {
        IpAddr::V4(ip) => ip.is_private() || ip.is_loopback() || ip.is_link_local(),
        IpAddr::V6(ip) => ip.is_loopback() || ip.is_unique_local(),
    })
}

fn bridge_websocket_payload(value: &Value) -> String {
    if let Some(body) = value.get("body") {
        if let Some(text) = body.as_str() {
            return text.to_string();
        }
        return body.to_string();
    }
    value
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

async fn forward_codex_local_request(value: &Value) -> Result<Value> {
    let session_id = value
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let method = value
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("POST")
        .to_ascii_uppercase();
    let path =
        sanitize_local_service_path(value.get("path").and_then(Value::as_str).unwrap_or("/"))?;
    let url = format!("http://127.0.0.1:8390{path}");
    let client = reqwest::Client::new();
    let mut request = match method.as_str() {
        "GET" => client.get(url),
        "POST" => client.post(url),
        "PUT" => client.put(url),
        "PATCH" => client.patch(url),
        "DELETE" => client.delete(url),
        _ => anyhow::bail!("unsupported bridge HTTP method: {method}"),
    };
    if let Some(headers) = value.get("headers").and_then(Value::as_object) {
        for (name, header_value) in headers {
            if !is_forwardable_header(name) {
                continue;
            }
            if let Some(header_value) = header_value.as_str() {
                request = request.header(name, header_value);
            }
        }
    }
    if let Some(body) = value.get("body").filter(|body| !body.is_null()) {
        if let Some(text) = body.as_str() {
            request = request.body(text.to_string());
        } else {
            request = request.json(body);
        }
    }
    let response = request.send().await?;
    let status = response.status().as_u16();
    let mut headers = serde_json::Map::new();
    for (name, value) in response.headers() {
        if let Ok(value) = value.to_str() {
            headers.insert(name.as_str().to_string(), json!(value));
        }
    }
    let body = response.text().await?;
    Ok(json!({
        "type": "bridge.response",
        "session_id": session_id,
        "service_id": "codex.local",
        "status": status,
        "headers": headers,
        "body": body
    }))
}

fn sanitize_local_service_path(path: &str) -> Result<String> {
    if !path.starts_with('/') || path.contains("://") || path.contains('\\') {
        anyhow::bail!("invalid local service path");
    }
    Ok(path.to_string())
}

fn is_forwardable_header(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    !matches!(
        name.as_str(),
        "host" | "connection" | "upgrade" | "proxy-authorization" | "proxy-authenticate"
    )
}

fn start_terminal_agent(base: String, node_id: String) {
    thread::spawn(move || {
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime,
            Err(error) => {
                eprintln!("terminal runtime start failed: {error}");
                return;
            }
        };
        runtime.block_on(async move {
            let mut reconnect_attempt = 0_u32;
            loop {
                let started_at = std::time::Instant::now();
                if let Err(error) = terminal_agent_loop(&base, &node_id).await {
                    eprintln!("terminal websocket disconnected: {error:#}");
                }
                if started_at.elapsed() >= Duration::from_secs(CONTROL_WS_STABLE_RESET_SECONDS) {
                    reconnect_attempt = 0;
                } else {
                    reconnect_attempt = reconnect_attempt.saturating_add(1);
                }
                tokio::time::sleep(reconnect_delay(reconnect_attempt)).await;
            }
        });
    });
}

async fn terminal_agent_loop(base: &str, node_id: &str) -> Result<()> {
    let url = terminal_ws_url(base, node_id);
    let (socket, _) = connect_async(&url).await?;
    let (mut sink, mut stream) = socket.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Value>();
    let sessions = Arc::new(AsyncMutex::new(HashMap::<String, TerminalSession>::new()));
    let mut ping_interval =
        tokio::time::interval(Duration::from_secs(CONTROL_WS_PING_INTERVAL_SECONDS));
    let mut stale_timeout = Box::pin(tokio::time::sleep(Duration::from_secs(
        CONTROL_WS_STALE_AFTER_SECONDS,
    )));

    let send_task = tokio::spawn(async move {
        while let Some(value) = out_rx.recv().await {
            let send_result = tokio::time::timeout(
                Duration::from_secs(CONTROL_WS_SEND_TIMEOUT_SECONDS),
                sink.send(Message::Text(value.to_string())),
            )
            .await;
            if !matches!(send_result, Ok(Ok(()))) {
                break;
            }
        }
    });

    loop {
        tokio::select! {
            _ = ping_interval.tick() => {
                let _ = out_tx.send(json!({
                    "type": "terminal.worker_ping",
                    "node_id": node_id,
                    "ts": now_rfc3339()
                }));
            }
            _ = &mut stale_timeout => {
                sessions.lock().await.clear();
                send_task.abort();
                anyhow::bail!("terminal websocket stale: no hub response for {CONTROL_WS_STALE_AFTER_SECONDS}s");
            }
            message = stream.next() => {
                let Some(message) = message else {
                    break;
                };
                let message = message?;
                stale_timeout.as_mut().reset(tokio::time::Instant::now() + Duration::from_secs(CONTROL_WS_STALE_AFTER_SECONDS));
                match message {
                    Message::Text(text) => {
                        let value: Value = serde_json::from_str(&text)?;
                        if value.get("type").and_then(Value::as_str) == Some("terminal.worker_pong") {
                            continue;
                        }
                        handle_terminal_command(value, Arc::clone(&sessions), out_tx.clone()).await;
                    }
                    Message::Ping(payload) => {
                        let _ = out_tx.send(json!({
                            "type": "terminal.worker_pong",
                            "node_id": node_id,
                            "payload_base64": base64::engine::general_purpose::STANDARD.encode(payload),
                            "ts": now_rfc3339()
                        }));
                    }
                    Message::Pong(_) => {}
                    Message::Close(frame) => {
                        sessions.lock().await.clear();
                        send_task.abort();
                        anyhow::bail!("terminal websocket closed by hub: {frame:?}");
                    }
                    _ => {}
                }
            }
        }

        if send_task.is_finished() {
            sessions.lock().await.clear();
            send_task.abort();
            anyhow::bail!("terminal websocket send loop stopped");
        }
    }

    sessions.lock().await.clear();
    send_task.abort();
    Ok(())
}

async fn handle_terminal_command(
    value: Value,
    sessions: Arc<AsyncMutex<HashMap<String, TerminalSession>>>,
    out_tx: mpsc::UnboundedSender<Value>,
) {
    let message_type = value.get("type").and_then(Value::as_str).unwrap_or("");
    let session_id = value
        .get("session_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if session_id.is_empty() {
        return;
    }
    match message_type {
        "terminal.open" => {
            if let Err(error) =
                open_terminal_session(&session_id, Arc::clone(&sessions), out_tx.clone(), &value)
                    .await
            {
                let _ = out_tx.send(json!({
                    "type": "terminal.error",
                    "session_id": session_id,
                    "message": error.to_string()
                }));
            }
        }
        "terminal.input" => {
            let data = value.get("data").and_then(Value::as_str).unwrap_or("");
            let writer = {
                let sessions = sessions.lock().await;
                sessions
                    .get(&session_id)
                    .map(|session| Arc::clone(&session.writer))
            };
            if let Some(writer) = writer {
                let data = data.as_bytes().to_vec();
                let _ = tokio::task::spawn_blocking(move || {
                    let mut writer = writer.lock().expect("terminal writer lock");
                    writer.write_all(&data)?;
                    writer.flush()
                })
                .await;
            }
        }
        "terminal.close" => {
            let session = sessions.lock().await.remove(&session_id);
            if let Some(mut session) = session {
                let _ = session.child.kill();
            }
        }
        _ => {}
    }
}

async fn open_terminal_session(
    session_id: &str,
    sessions: Arc<AsyncMutex<HashMap<String, TerminalSession>>>,
    out_tx: mpsc::UnboundedSender<Value>,
    value: &Value,
) -> Result<()> {
    let rows = value.get("rows").and_then(Value::as_u64).unwrap_or(32) as u16;
    let cols = value.get("cols").and_then(Value::as_u64).unwrap_or(120) as u16;
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    })?;
    let shell = terminal_shell();
    let command = CommandBuilder::new(shell);
    let child = pair.slave.spawn_command(command)?;
    let mut reader = pair.master.try_clone_reader()?;
    let writer = Arc::new(Mutex::new(pair.master.take_writer()?));
    let session_id_output = session_id.to_string();
    let out_tx_output = out_tx.clone();

    thread::spawn(move || {
        let mut buffer = [0_u8; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(size) => {
                    let text = String::from_utf8_lossy(&buffer[..size]).to_string();
                    let _ = out_tx_output.send(json!({
                        "type": "terminal.output",
                        "session_id": session_id_output,
                        "stream": "pty",
                        "data": text
                    }));
                }
                Err(error) => {
                    let _ = out_tx_output.send(json!({
                        "type": "terminal.error",
                        "session_id": session_id_output,
                        "message": error.to_string()
                    }));
                    break;
                }
            }
        }
    });

    sessions.lock().await.insert(
        session_id.to_string(),
        TerminalSession {
            writer,
            child,
            _master: pair.master,
        },
    );
    let _ = out_tx.send(json!({
        "type": "terminal.ready",
        "session_id": session_id,
        "message": "远程终端已连接"
    }));
    Ok(())
}

struct TerminalSession {
    writer: Arc<Mutex<Box<dyn std::io::Write + Send>>>,
    child: Box<dyn PtyChild + Send + Sync>,
    _master: Box<dyn MasterPty + Send>,
}

fn terminal_shell() -> String {
    if cfg!(windows) {
        std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
    }
}

fn terminal_ws_url(base: &str, node_id: &str) -> String {
    let mut url = base.trim_end_matches('/').to_string();
    if let Some(rest) = url.strip_prefix("https://") {
        url = format!("wss://{rest}");
    } else if let Some(rest) = url.strip_prefix("http://") {
        url = format!("ws://{rest}");
    }
    format!("{url}/api/worker/terminal/ws?node_id={node_id}")
}

fn bridge_ws_url(base: &str, node_id: &str) -> String {
    let mut url = base.trim_end_matches('/').to_string();
    if let Some(rest) = url.strip_prefix("https://") {
        url = format!("wss://{rest}");
    } else if let Some(rest) = url.strip_prefix("http://") {
        url = format!("ws://{rest}");
    }
    format!("{url}/api/worker/bridge/ws?node_id={node_id}")
}

fn parse_task_payload(task: &Value) -> Result<JobPayload> {
    let raw = task
        .pointer("/spec/inputs/0")
        .and_then(Value::as_str)
        .context("task input payload missing")?;
    let value: Value = serde_json::from_str(raw).context("task input payload is not json")?;
    let task_type = value.get("type").and_then(Value::as_str).unwrap_or("");
    match task_type {
        "http_request" => Ok(JobPayload::HttpRequest(HttpRequestPayload {
            method: value
                .get("method")
                .and_then(Value::as_str)
                .unwrap_or("GET")
                .to_string(),
            url: value
                .get("url")
                .and_then(Value::as_str)
                .context("http url missing")?
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
        "command" => Ok(JobPayload::Command(CommandPayload {
            program: value
                .get("program")
                .and_then(Value::as_str)
                .context("command program missing")?
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
        "plugin" => Ok(JobPayload::Plugin(PluginPayload {
            plugin_id: value
                .get("plugin_id")
                .and_then(Value::as_str)
                .context("plugin_id missing")?
                .to_string(),
            action: value
                .get("action")
                .and_then(Value::as_str)
                .unwrap_or("run")
                .to_string(),
            input: value.get("input").cloned().unwrap_or_else(|| json!({})),
            timeout_seconds: value
                .get("timeout_seconds")
                .and_then(Value::as_u64)
                .unwrap_or(60),
        })),
        other => {
            let executor = value.get("executor").and_then(Value::as_str).unwrap_or("");
            if let Some(plugin_id) = executor.strip_prefix("plugin:") {
                return Ok(JobPayload::Plugin(PluginPayload {
                    plugin_id: plugin_id.to_string(),
                    action: value
                        .get("action")
                        .and_then(Value::as_str)
                        .unwrap_or("run")
                        .to_string(),
                    input: value.get("input").cloned().unwrap_or_else(|| {
                        value
                            .as_object()
                            .map(|map| {
                                let mut input = map.clone();
                                input.remove("type");
                                input.remove("tool_id");
                                input.remove("executor");
                                input.remove("action");
                                input.remove("timeout_seconds");
                                Value::Object(input)
                            })
                            .unwrap_or_else(|| json!({}))
                    }),
                    timeout_seconds: value
                        .get("timeout_seconds")
                        .and_then(Value::as_u64)
                        .unwrap_or(60),
                }));
            }
            anyhow::bail!("unsupported task payload type: {other}")
        }
    }
}

fn enforce_policy(payload: &JobPayload, policy: &SecurityPolicy) -> Result<()> {
    match payload {
        JobPayload::HttpRequest(request) => enforce_http_policy(request, policy),
        JobPayload::Command(command) => enforce_command_policy(command, policy),
        JobPayload::Git(_) => enforce_command_name("git", policy),
        JobPayload::Docker(_) => enforce_command_name("docker", policy),
        JobPayload::File(_) => Ok(()),
        JobPayload::Browser(_) => Ok(()),
        JobPayload::Desktop(_) => Ok(()),
        JobPayload::Session(session) => enforce_session_policy(session, policy),
        JobPayload::AgentMessage(_) => Ok(()),
        JobPayload::Plugin(_) => Ok(()),
        JobPayload::Custom { .. } => anyhow::bail!("custom execution is disabled"),
    }
}

fn enforce_http_policy(request: &HttpRequestPayload, policy: &SecurityPolicy) -> Result<()> {
    let url = reqwest::Url::parse(&request.url)?;
    match url.scheme() {
        "http" | "https" => {}
        scheme => anyhow::bail!("policy denied: unsupported URL scheme {scheme}"),
    }
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if !policy.http.allowed_domains.is_empty()
        && !policy.http.allowed_domains.iter().any(|domain| {
            host == domain.to_ascii_lowercase()
                || host.ends_with(&format!(".{}", domain.to_ascii_lowercase()))
        })
    {
        anyhow::bail!("policy denied: domain {host} is not in allowed_domains");
    }
    if matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1")
        || policy.http.blocked_ips.iter().any(|ip| ip == &host)
    {
        anyhow::bail!("policy denied: blocked host {host}");
    }
    if !policy.http.allow_private_network
        && (host.starts_with("10.")
            || host.starts_with("192.168.")
            || host.starts_with("172.16.")
            || host.starts_with("172.17.")
            || host.starts_with("172.18.")
            || host.starts_with("172.19.")
            || host.starts_with("172.2")
            || host.starts_with("172.30.")
            || host.starts_with("172.31."))
    {
        anyhow::bail!("policy denied: private network targets are disabled");
    }
    if request.max_response_bytes > policy.http.max_response_bytes {
        anyhow::bail!("policy denied: max_response_bytes exceeds hub policy");
    }
    Ok(())
}

fn enforce_command_policy(command: &CommandPayload, policy: &SecurityPolicy) -> Result<()> {
    if !policy.command.enabled {
        anyhow::bail!("policy denied: command execution is disabled");
    }
    let program_name = command
        .program
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(command.program.as_str());
    if !policy
        .command
        .command_allowlist
        .iter()
        .any(|allowed| allowed == &command.program || allowed == program_name)
    {
        anyhow::bail!(
            "policy denied: command {} is not allowlisted",
            command.program
        );
    }
    Ok(())
}

fn enforce_command_name(program: &str, policy: &SecurityPolicy) -> Result<()> {
    if !policy.command.enabled {
        anyhow::bail!("policy denied: command execution is disabled");
    }
    if !policy
        .command
        .command_allowlist
        .iter()
        .any(|allowed| allowed == program)
    {
        anyhow::bail!("policy denied: command {program} is not allowlisted");
    }
    Ok(())
}

fn enforce_session_policy(session: &SessionPayload, policy: &SecurityPolicy) -> Result<()> {
    match session {
        SessionPayload::Run {
            program,
            args,
            working_dir,
            timeout_seconds,
            ..
        } => enforce_command_policy(
            &CommandPayload {
                program: program.clone(),
                args: args.clone(),
                working_dir: working_dir.clone(),
                timeout_seconds: *timeout_seconds,
            },
            policy,
        ),
    }
}

fn parse_file_payload(value: &Value) -> Result<FilePayload> {
    match value.get("operation").and_then(Value::as_str).unwrap_or("") {
        "read" => Ok(FilePayload::Read {
            path: required_value_string(value, "path")?,
            max_bytes: value.get("max_bytes").and_then(Value::as_u64),
        }),
        "write" => Ok(FilePayload::Write {
            path: required_value_string(value, "path")?,
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
            path: required_value_string(value, "path")?,
            recursive: value
                .get("recursive")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            max_entries: value.get("max_entries").and_then(Value::as_u64),
        }),
        "upload" => Ok(FilePayload::Upload {
            path: required_value_string(value, "path")?,
            content_base64: required_value_string(value, "content_base64")?,
            create_dirs: value
                .get("create_dirs")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        }),
        "download" => Ok(FilePayload::Download {
            path: required_value_string(value, "path")?,
            max_bytes: value.get("max_bytes").and_then(Value::as_u64),
        }),
        other => anyhow::bail!("unsupported file operation: {other}"),
    }
}

fn parse_git_payload(value: &Value) -> Result<GitPayload> {
    match value.get("operation").and_then(Value::as_str).unwrap_or("") {
        "clone" => Ok(GitPayload::Clone {
            repo: required_value_string(value, "repo")?,
            dest: required_value_string(value, "dest")?,
            branch: optional_value_string(value, "branch"),
            depth: value
                .get("depth")
                .and_then(Value::as_u64)
                .map(|depth| depth as u32),
        }),
        "pull" => Ok(GitPayload::Pull {
            repo_dir: required_value_string(value, "repo_dir")?,
        }),
        "status" => Ok(GitPayload::Status {
            repo_dir: required_value_string(value, "repo_dir")?,
        }),
        "checkout" => Ok(GitPayload::Checkout {
            repo_dir: required_value_string(value, "repo_dir")?,
            reference: required_value_string(value, "reference")?,
        }),
        other => anyhow::bail!("unsupported git operation: {other}"),
    }
}

fn parse_docker_payload(value: &Value) -> Result<DockerPayload> {
    match value.get("operation").and_then(Value::as_str).unwrap_or("") {
        "ps" => Ok(DockerPayload::Ps),
        "images" => Ok(DockerPayload::Images),
        "run" => Ok(DockerPayload::Run {
            image: required_value_string(value, "image")?,
            args: string_array(value.get("args")),
            timeout_seconds: value
                .get("timeout_seconds")
                .and_then(Value::as_u64)
                .unwrap_or(60),
        }),
        other => anyhow::bail!("unsupported docker operation: {other}"),
    }
}

fn parse_browser_payload(value: &Value) -> Result<BrowserPayload> {
    match value
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("fetch")
    {
        "fetch" => Ok(BrowserPayload::Fetch {
            url: required_value_string(value, "url")?,
            selector: optional_value_string(value, "selector"),
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
            url: required_value_string(value, "url")?,
            actions: value
                .get("actions")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
            screenshot_path: optional_value_string(value, "screenshot_path"),
            timeout_seconds: value
                .get("timeout_seconds")
                .and_then(Value::as_u64)
                .unwrap_or(60),
        }),
        other => anyhow::bail!("unsupported browser operation: {other}"),
    }
}

fn parse_desktop_payload(value: &Value) -> Result<DesktopPayload> {
    match value
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("screenshot")
    {
        "screenshot" => Ok(DesktopPayload::Screenshot {
            path: optional_value_string(value, "path"),
            timeout_seconds: value
                .get("timeout_seconds")
                .and_then(Value::as_u64)
                .unwrap_or(30),
        }),
        "click" => Ok(DesktopPayload::Click {
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
        "type_text" => Ok(DesktopPayload::TypeText {
            text: required_value_string(value, "text")?,
            interval_ms: value.get("interval_ms").and_then(Value::as_u64),
            timeout_seconds: value
                .get("timeout_seconds")
                .and_then(Value::as_u64)
                .unwrap_or(30),
        }),
        "key" => Ok(DesktopPayload::Key {
            key: required_value_string(value, "key")?,
            modifiers: string_array(value.get("modifiers")),
            timeout_seconds: value
                .get("timeout_seconds")
                .and_then(Value::as_u64)
                .unwrap_or(10),
        }),
        other => anyhow::bail!("unsupported desktop operation: {other}"),
    }
}

fn parse_session_payload(value: &Value) -> Result<SessionPayload> {
    match value
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("run")
    {
        "run" => Ok(SessionPayload::Run {
            session_id: optional_value_string(value, "session_id"),
            program: required_value_string(value, "program")?,
            args: string_array(value.get("args")),
            working_dir: optional_value_string(value, "working_dir"),
            timeout_seconds: value
                .get("timeout_seconds")
                .and_then(Value::as_u64)
                .unwrap_or(300),
        }),
        other => anyhow::bail!("unsupported session operation: {other}"),
    }
}

fn deliver_agent_message(task: &Value, message: AgentMessagePayload) -> Result<JobResult> {
    let started = std::time::Instant::now();
    let task_id = task
        .pointer("/metadata/id")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    Ok(JobResult::AgentMessageResult {
        delivered: true,
        message_id: None,
        summary: format!(
            "{} -> {}: {} ({task_id})",
            message.from,
            message.to.join(", "),
            message.subject
        ),
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

fn required_value_string(value: &Value, key: &str) -> Result<String> {
    optional_value_string(value, key)
        .filter(|item| !item.is_empty())
        .context(format!("{key} missing"))
}

fn optional_value_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn default_node_id() -> String {
    let host = System::host_name().unwrap_or_else(|| "unknown-host".to_string());
    format!("{}-{}", host, std::env::consts::OS)
        .to_lowercase()
        .replace(' ', "-")
}

fn default_machine_fingerprint() -> String {
    let host = System::host_name().unwrap_or_else(|| "unknown-host".to_string());
    let os = System::name().unwrap_or_else(|| std::env::consts::OS.to_string());
    let long_os = System::long_os_version().unwrap_or_default();
    let kernel = System::kernel_version().unwrap_or_default();
    let machine_id = read_machine_id().unwrap_or_default();
    let raw = format!(
        "agentgrid-machine-v1|{}|{}|{}|{}|{}|{}|{}",
        host,
        os,
        long_os,
        kernel,
        std::env::consts::OS,
        std::env::consts::ARCH,
        machine_id
    );
    format!("sha256:{}", sha256_hex(raw.as_bytes()))
}

fn read_machine_id() -> Option<String> {
    let candidates = [
        "/etc/machine-id",
        "/var/lib/dbus/machine-id",
        "/var/db/db.uuid",
    ];
    for path in candidates {
        if let Ok(value) = fs::read_to_string(path) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    if std::env::consts::OS == "windows" {
        if let Ok(value) = env::var("COMPUTERNAME") {
            if !value.trim().is_empty() {
                return Some(value);
            }
        }
    }
    None
}

fn collect_report(
    id: &str,
    name: &str,
    tags: &[String],
    capabilities: &[String],
    running_jobs: usize,
    max_concurrent_jobs: usize,
    auto_update_enabled: bool,
    update_channel: &str,
    machine_fingerprint: &str,
    channel_role: &str,
    join_token: Option<&str>,
) -> serde_json::Value {
    let mut system = System::new_all();
    system.refresh_all();
    let disks = Disks::new_with_refreshed_list();

    let total_memory_mb = bytes_to_mb(system.total_memory());
    let used_memory_mb = bytes_to_mb(system.used_memory());
    let disk_total_mb = disks
        .iter()
        .map(|disk| bytes_to_mb(disk.total_space()))
        .sum::<u64>();
    let disk_free_mb = disks
        .iter()
        .map(|disk| bytes_to_mb(disk.available_space()))
        .sum::<u64>();
    let cpu_usage = if system.cpus().is_empty() {
        0.0
    } else {
        system
            .cpus()
            .iter()
            .map(|cpu| cpu.cpu_usage() as f64)
            .sum::<f64>()
            / system.cpus().len() as f64
    };

    json!({
        "id": id,
        "name": name,
        "os": System::name().unwrap_or_else(|| std::env::consts::OS.to_string()),
        "arch": std::env::consts::ARCH,
        "address": node_address(),
        "worker_version": WORKER_VERSION,
        "worker_target": worker_target(),
        "glibc_version": glibc_version(),
        "machine_fingerprint": machine_fingerprint,
        "physical_host_id": machine_fingerprint,
        "channel_role": channel_role,
        "join_token": join_token,
        "auto_update_enabled": auto_update_enabled,
        "update_channel": update_channel,
        "tags": tags,
        "capabilities": capabilities,
        "local_services": discover_local_services(),
        "groups": node_groups(tags),
        "cpu_cores": system.cpus().len() as i64,
        "memory_mb": total_memory_mb,
        "cpu_usage_percent": cpu_usage,
        "memory_used_mb": used_memory_mb,
        "disk_total_mb": disk_total_mb,
        "disk_free_mb": disk_free_mb,
        "running_jobs": running_jobs as i64,
        "max_concurrent_jobs": max_concurrent_jobs as i64,
        "status": "online"
    })
}

fn normalize_node_channel_role(explicit: Option<&str>, id: &str) -> String {
    let explicit = explicit.unwrap_or("").trim().to_ascii_lowercase();
    if matches!(
        explicit.as_str(),
        "worker" | "desktop" | "service" | "bridge" | "device"
    ) {
        return explicit;
    }
    if id.ends_with("-desktop") {
        "desktop".to_string()
    } else {
        "worker".to_string()
    }
}

fn default_capabilities_for_channel(channel_role: &str) -> Vec<String> {
    let items = match channel_role {
        "desktop" => vec!["desktop"],
        "service" => vec!["service_bridge"],
        "bridge" => vec!["port_bridge"],
        "device" => vec!["device"],
        _ => vec![
            "http",
            "command",
            "file",
            "git",
            "docker",
            "browser",
            "session",
            "agentmessage",
            "plugin",
            "port_bridge",
        ],
    };
    items.into_iter().map(ToString::to_string).collect()
}

fn discover_local_services() -> Vec<Value> {
    vec![codex_local_service()]
}

fn codex_local_service() -> Value {
    let available = std::net::TcpStream::connect_timeout(
        &"127.0.0.1:8390"
            .parse()
            .expect("codex bridge socket address"),
        Duration::from_millis(150),
    )
    .is_ok();
    json!({
        "id": "codex.local",
        "name": "Codex Local Bridge",
        "capability": "codex.local_bridge",
        "protocol": "http",
        "host": "127.0.0.1",
        "port": 8390,
        "status": if available { "available" } else { "unavailable" },
        "exposure": "hub_authenticated",
        "allowed_transports": ["websocket"],
        "allowed_methods": ["GET", "POST", "PUT", "PATCH", "DELETE"],
        "health_checked_at": chrono_like_now()
    })
}

fn chrono_like_now() -> String {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => format!("{}", duration.as_secs()),
        Err(_) => "0".to_string(),
    }
}

fn worker_target() -> String {
    let os = normalize_os(std::env::consts::OS);
    let arch = normalize_arch(std::env::consts::ARCH);
    if os == "linux" {
        if let Some(version) = glibc_version() {
            if compare_versions(&version, "2.34") < 0 {
                return format!("linux-glibc-2.32-{arch}");
            }
        }
    }
    format!("{os}-{arch}")
}

fn normalize_os(value: &str) -> String {
    match value.to_ascii_lowercase().as_str() {
        "macos" | "darwin" => "darwin".to_string(),
        "windows" | "win32" => "windows".to_string(),
        "linux" => "linux".to_string(),
        other => other.replace(' ', "-"),
    }
}

fn normalize_arch(value: &str) -> String {
    match value.to_ascii_lowercase().as_str() {
        "amd64" | "x86_64" => "x86_64".to_string(),
        "arm64" | "aarch64" => "aarch64".to_string(),
        other => other.replace(' ', "-"),
    }
}

fn glibc_version() -> Option<String> {
    if std::env::consts::OS != "linux" {
        return None;
    }
    for candidate in ["/lib64/libc.so.6", "/lib/x86_64-linux-gnu/libc.so.6"] {
        if let Ok(output) = StdCommand::new(candidate).output() {
            let text = String::from_utf8_lossy(&output.stdout);
            if let Some(version) = parse_glibc_version(&text) {
                return Some(version);
            }
        }
    }
    if let Ok(mut file) = fs::File::open("/lib64/libc.so.6") {
        let mut bytes = Vec::new();
        let _ = std::io::Read::by_ref(&mut file)
            .take(8192)
            .read_to_end(&mut bytes);
        let text = String::from_utf8_lossy(&bytes);
        return parse_glibc_version(&text);
    }
    None
}

fn parse_glibc_version(text: &str) -> Option<String> {
    for line in text.lines() {
        if let Some(index) = line.find("release version ") {
            let rest = &line[index + "release version ".len()..];
            return rest
                .split([',', ' ', '\n'])
                .find(|item| item.chars().next().is_some_and(|ch| ch.is_ascii_digit()))
                .map(clean_version_token);
        }
        if let Some(index) = line.find("GNU C Library") {
            let rest = &line[index..];
            if let Some(version) = rest
                .split_whitespace()
                .find(|item| item.chars().next().is_some_and(|ch| ch.is_ascii_digit()))
            {
                return Some(clean_version_token(version));
            }
        }
    }
    None
}

fn clean_version_token(value: &str) -> String {
    value
        .trim_matches([')', '(', ',', ';'])
        .trim_end_matches(|ch: char| !ch.is_ascii_digit())
        .to_string()
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

fn bytes_to_mb(value: u64) -> u64 {
    value / 1024 / 1024
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn node_groups(tags: &[String]) -> Vec<String> {
    let mut groups = vec!["default".to_string()];
    for tag in tags {
        if matches!(
            tag.as_str(),
            "linux" | "macos" | "windows" | "worker" | "center"
        ) {
            groups.push(tag.clone());
        }
    }
    groups.sort();
    groups.dedup();
    groups
}

fn fetch_policy(client: &reqwest::blocking::Client, base: &str) -> Result<SecurityPolicy> {
    let value = client
        .get(format!("{base}/api/policy"))
        .send()?
        .error_for_status()?
        .json::<Value>()?;
    Ok(policy_from_value(
        value.get("policy").unwrap_or(&Value::Null),
    ))
}

fn policy_from_value(value: &Value) -> SecurityPolicy {
    SecurityPolicy {
        http: HttpPolicy {
            allowed_domains: string_array(value.pointer("/http/allowed_domains")),
            blocked_ips: string_array(value.pointer("/http/blocked_ips")),
            allow_private_network: value
                .pointer("/http/allow_private_network")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            max_response_bytes: value
                .pointer("/http/max_response_bytes")
                .and_then(Value::as_u64)
                .unwrap_or(65_536),
        },
        command: CommandPolicy {
            enabled: value
                .pointer("/command/enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            command_allowlist: string_array(value.pointer("/command/command_allowlist")),
        },
    }
}

fn locked_down_policy() -> SecurityPolicy {
    SecurityPolicy {
        http: HttpPolicy {
            allowed_domains: Vec::new(),
            blocked_ips: vec![
                "127.0.0.1".to_string(),
                "::1".to_string(),
                "0.0.0.0".to_string(),
            ],
            allow_private_network: false,
            max_response_bytes: 65_536,
        },
        command: CommandPolicy {
            enabled: false,
            command_allowlist: Vec::new(),
        },
    }
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn node_address() -> String {
    let host = System::host_name().unwrap_or_default();
    let ip = local_ip_address().unwrap_or_default();
    match (host.is_empty(), ip.is_empty()) {
        (false, false) => format!("{host} / {ip}"),
        (false, true) => host,
        (true, false) => ip,
        (true, true) => String::new(),
    }
}

fn local_ip_address() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_loopback() => Some(ip.to_string()),
        IpAddr::V6(ip) if !ip.is_loopback() => Some(ip.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use rand_core::OsRng;

    #[test]
    fn worker_update_signature_verifies_and_rejects_tampering() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let public_key = base64::engine::general_purpose::STANDARD
            .encode(signing_key.verifying_key().to_bytes());
        let bytes = b"agentgrid worker update bytes";
        let signature =
            base64::engine::general_purpose::STANDARD.encode(signing_key.sign(bytes).to_bytes());
        let manifest = json!({
            "signature": signature,
            "signature_required": true
        });

        verify_worker_update_signature_from_manifest(&manifest, bytes, Some(&public_key), true)
            .expect("valid signature");

        let error = verify_worker_update_signature_from_manifest(
            &manifest,
            b"tampered update bytes",
            Some(&public_key),
            true,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("verify ed25519"));
    }

    #[test]
    fn required_update_signature_does_not_trust_manifest_public_key() {
        let signing_key = SigningKey::generate(&mut OsRng);
        let bytes = b"agentgrid worker update bytes";
        let signature =
            base64::engine::general_purpose::STANDARD.encode(signing_key.sign(bytes).to_bytes());
        let manifest_public_key = base64::engine::general_purpose::STANDARD
            .encode(signing_key.verifying_key().to_bytes());
        let manifest = json!({
            "signature": signature,
            "signing_public_key": manifest_public_key,
            "signature_required": true
        });

        let error =
            verify_worker_update_signature_from_manifest(&manifest, bytes, None, true).unwrap_err();
        assert!(error.to_string().contains("no public key configured"));
    }
}
