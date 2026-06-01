use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Priority;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub api_version: String,
    pub kind: String,
    pub metadata: JobMetadata,
    pub spec: JobSpec,
    pub status: JobStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobMetadata {
    pub id: String,
    pub project_id: String,
    pub client_id: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSpec {
    pub name: String,
    pub priority: Priority,
    pub requirements: JobRequirements,
    pub payload: JobPayload,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JobRequirements {
    pub tags: Vec<String>,
    pub capabilities: Vec<String>,
    pub os: Vec<String>,
    pub groups: Vec<String>,
    pub preferred_node_ids: Vec<String>,
    pub avoid_node_ids: Vec<String>,
    pub exclusive_key: Option<String>,
    pub cpu_cores: Option<u16>,
    pub memory_mb: Option<u64>,
    pub disk_free_mb: Option<u64>,
    pub node_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JobPayload {
    HttpRequest(HttpRequestPayload),
    Command(CommandPayload),
    File(FilePayload),
    Git(GitPayload),
    Docker(DockerPayload),
    Browser(BrowserPayload),
    Desktop(DesktopPayload),
    Session(SessionPayload),
    AgentMessage(AgentMessagePayload),
    Plugin(PluginPayload),
    Custom { name: String, value: Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRequestPayload {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Value>,
    pub timeout_seconds: u64,
    pub max_response_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandPayload {
    pub program: String,
    pub args: Vec<String>,
    pub working_dir: Option<String>,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum FilePayload {
    Read {
        path: String,
        max_bytes: Option<u64>,
    },
    Write {
        path: String,
        content: String,
        append: bool,
        create_dirs: bool,
    },
    List {
        path: String,
        recursive: bool,
        max_entries: Option<u64>,
    },
    Upload {
        path: String,
        content_base64: String,
        create_dirs: bool,
    },
    Download {
        path: String,
        max_bytes: Option<u64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum GitPayload {
    Clone {
        repo: String,
        dest: String,
        branch: Option<String>,
        depth: Option<u32>,
    },
    Pull {
        repo_dir: String,
    },
    Status {
        repo_dir: String,
    },
    Checkout {
        repo_dir: String,
        reference: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum DockerPayload {
    Ps,
    Images,
    Run {
        image: String,
        args: Vec<String>,
        timeout_seconds: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum BrowserPayload {
    Fetch {
        url: String,
        selector: Option<String>,
        timeout_seconds: u64,
        max_response_bytes: u64,
    },
    Automate {
        url: String,
        actions: Vec<Value>,
        screenshot_path: Option<String>,
        timeout_seconds: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum DesktopPayload {
    Screenshot {
        path: Option<String>,
        timeout_seconds: u64,
    },
    Click {
        x: i32,
        y: i32,
        button: String,
        timeout_seconds: u64,
    },
    TypeText {
        text: String,
        interval_ms: Option<u64>,
        timeout_seconds: u64,
    },
    Key {
        key: String,
        modifiers: Vec<String>,
        timeout_seconds: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum SessionPayload {
    Run {
        session_id: Option<String>,
        program: String,
        args: Vec<String>,
        working_dir: Option<String>,
        timeout_seconds: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessagePayload {
    pub from: String,
    pub to: Vec<String>,
    #[serde(rename = "type")]
    pub message_type: String,
    pub subject: String,
    pub summary: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginPayload {
    pub plugin_id: String,
    pub action: String,
    pub input: Value,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobStatus {
    pub state: JobState,
    pub assigned_node_id: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub result: Option<JobResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Scheduled,
    Dispatching,
    Running,
    Succeeded,
    Failed,
    Retrying,
    Cancelled,
    Lost,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JobResult {
    HttpResponse {
        status_code: u16,
        headers: Vec<(String, String)>,
        body: Value,
        duration_ms: u64,
    },
    CommandResult {
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
        duration_ms: u64,
    },
    FileResult {
        operation: String,
        path: String,
        content: Option<String>,
        entries: Vec<Value>,
        bytes: u64,
        duration_ms: u64,
    },
    GitResult {
        operation: String,
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
        duration_ms: u64,
    },
    DockerResult {
        operation: String,
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
        duration_ms: u64,
    },
    BrowserResult {
        url: String,
        status_code: u16,
        title: Option<String>,
        text: String,
        screenshot_path: Option<String>,
        duration_ms: u64,
    },
    DesktopResult {
        operation: String,
        path: Option<String>,
        content_base64: Option<String>,
        width: Option<u64>,
        height: Option<u64>,
        bytes: u64,
        message: String,
        duration_ms: u64,
    },
    SessionResult {
        session_id: String,
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
        duration_ms: u64,
    },
    AgentMessageResult {
        delivered: bool,
        message_id: Option<String>,
        summary: String,
        duration_ms: u64,
    },
    PluginResult {
        plugin_id: String,
        action: String,
        output: Value,
        duration_ms: u64,
    },
    Error {
        code: String,
        message: String,
        retryable: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub name: String,
    pub os: String,
    pub arch: String,
    pub tags: Vec<String>,
    pub capabilities: Vec<String>,
    pub cpu_cores: u16,
    pub memory_mb: u64,
    pub cpu_usage_percent: f32,
    pub memory_used_mb: u64,
    pub disk_total_mb: u64,
    pub disk_free_mb: u64,
    pub running_jobs: u16,
    pub max_concurrent_jobs: u16,
    pub weight: f64,
    pub groups: Vec<String>,
    pub success_rate: f64,
    pub status: NodeState,
    pub last_heartbeat_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeState {
    Online,
    Unknown,
    Offline,
    Busy,
    Draining,
    Disabled,
    Untrusted,
}
