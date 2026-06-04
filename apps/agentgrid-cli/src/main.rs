use std::{
    fs,
    io::{self, Read},
    thread,
    time::Duration,
};

use anyhow::{Context, Result};
use base64::Engine;
use clap::{Parser, Subcommand};
use serde_json::Value;

const DEFAULT_HUB_URL: &str = "http://chenqi.tminos.com:20080/agentgrid";

#[derive(Debug, Parser)]
#[command(name = "agentgrid")]
#[command(about = "AgentGrid CLI for AI agent collaboration and compute runtime")]
struct Cli {
    #[arg(long, default_value = DEFAULT_HUB_URL, env = "AGENTGRID_HUB")]
    hub: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Health,
    Agents,
    Messages {
        #[arg(long, default_value_t = 20)]
        limit: u16,
    },
    Nodes {
        #[command(subcommand)]
        command: Option<NodeCommands>,
    },
    Workbench {
        #[command(subcommand)]
        command: Option<WorkbenchCommands>,
    },
    NodeTools {
        #[command(subcommand)]
        command: Option<NodeToolCommands>,
    },
    Policy,
    Capabilities,
    Standard {
        #[command(subcommand)]
        command: Option<StandardCommands>,
    },
    Tools {
        #[command(subcommand)]
        command: Option<ToolCommands>,
    },
    Tasks {
        #[command(subcommand)]
        command: Option<TaskCommands>,
    },
    Jobs {
        #[command(subcommand)]
        command: Option<JobCommands>,
    },
    Workflows {
        #[command(subcommand)]
        command: Option<WorkflowCommands>,
    },
    Runtime {
        #[command(subcommand)]
        command: Option<RuntimeCommands>,
    },
    Records {
        #[command(subcommand)]
        command: RecordCommands,
    },
    TaskTemplates {
        #[command(subcommand)]
        command: Option<TaskTemplateCommands>,
    },
    Webhooks {
        #[command(subcommand)]
        command: Option<WebhookCommands>,
    },
    PortBridges {
        #[command(subcommand)]
        command: Option<PortBridgeCommands>,
    },
    BridgePort {
        #[arg(long = "source-node")]
        source_node: String,
        #[arg(long = "target-node")]
        target_node: String,
        #[arg(long = "target-port")]
        target_port: u16,
        #[arg(long = "source-port", default_value_t = 0)]
        source_port: u16,
        #[arg(long = "target-host", default_value = "127.0.0.1")]
        target_host: String,
        #[arg(long = "bind-host", default_value = "127.0.0.1")]
        bind_host: String,
        #[arg(long, default_value_t = 1800)]
        ttl_seconds: u64,
        #[arg(long, default_value = "tcp")]
        protocol: String,
        #[arg(long)]
        purpose: Option<String>,
        #[arg(long, default_value = "agentgrid-cli")]
        created_by: String,
        #[arg(long, default_value = "text")]
        output: String,
    },
    SubmitHttp {
        #[arg(long)]
        url: String,
        #[arg(long, default_value = "GET")]
        method: String,
        #[arg(long = "header")]
        headers: Vec<String>,
        #[arg(long)]
        body: Option<String>,
        #[arg(long, default_value_t = 30)]
        timeout_seconds: u64,
        #[arg(long, default_value_t = 65536)]
        max_response_bytes: u64,
        #[arg(long = "node")]
        node_id: Option<String>,
        #[arg(long = "workbench")]
        workbench_id: Option<String>,
        #[arg(long = "os")]
        os: Option<String>,
        #[arg(long, default_value = "architect-agent")]
        created_by: String,
        #[arg(long, default_value = "worker-agent")]
        owner: String,
        #[arg(long, default_value = "normal")]
        priority: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, default_value_t = false)]
        wait: bool,
        #[arg(long, default_value_t = 300)]
        wait_timeout_seconds: u64,
        #[arg(long, default_value = "json")]
        output: String,
        #[arg(long)]
        verify_json: Option<String>,
        #[arg(long, default_value_t = false)]
        expect_status_2xx: bool,
        #[arg(long)]
        expect_status: Option<u16>,
        #[arg(long)]
        expect_body_contains: Option<String>,
    },
    SubmitCommand {
        #[arg(long)]
        program: String,
        #[arg(long = "arg")]
        args: Vec<String>,
        #[arg(long)]
        working_dir: Option<String>,
        #[arg(long, default_value_t = 30)]
        timeout_seconds: u64,
        #[arg(long = "node")]
        node_id: Option<String>,
        #[arg(long = "workbench")]
        workbench_id: Option<String>,
        #[arg(long = "os")]
        os: Option<String>,
        #[arg(long, default_value = "architect-agent")]
        created_by: String,
        #[arg(long, default_value = "worker-agent")]
        owner: String,
        #[arg(long, default_value = "normal")]
        priority: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, default_value_t = false)]
        wait: bool,
        #[arg(long, default_value_t = 300)]
        wait_timeout_seconds: u64,
        #[arg(long, default_value = "text")]
        output: String,
        #[arg(long)]
        verify_json: Option<String>,
        #[arg(long)]
        expect_exit_code: Option<i64>,
        #[arg(long)]
        expect_stdout_contains: Option<String>,
        #[arg(long)]
        expect_stderr_contains: Option<String>,
    },
    SubmitSoftwareInstall {
        #[arg(long = "node")]
        node_id: String,
        #[arg(long, default_value = "windows")]
        os: String,
        #[arg(long)]
        name: String,
        #[arg(long = "source-url")]
        source_url: String,
        #[arg(long, default_value = "exe")]
        installer: String,
        #[arg(long = "installer-arg")]
        installer_args: Vec<String>,
        #[arg(long)]
        sha256: Option<String>,
        #[arg(long, default_value_t = 900)]
        timeout_seconds: u64,
        #[arg(long, default_value = "architect-agent")]
        created_by: String,
        #[arg(long, default_value = "worker-agent")]
        owner: String,
        #[arg(long, default_value = "normal")]
        priority: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, default_value_t = false)]
        wait: bool,
        #[arg(long, default_value_t = 1200)]
        wait_timeout_seconds: u64,
        #[arg(long, default_value = "text")]
        output: String,
    },
    SubmitFile {
        #[arg(long)]
        operation: String,
        #[arg(long)]
        path: String,
        #[arg(long)]
        content: Option<String>,
        #[arg(long, default_value_t = false)]
        append: bool,
        #[arg(long, default_value_t = true)]
        create_dirs: bool,
        #[arg(long, default_value_t = false)]
        recursive: bool,
        #[arg(long)]
        max_bytes: Option<u64>,
        #[arg(long)]
        max_entries: Option<u64>,
        #[arg(long = "node")]
        node_id: Option<String>,
        #[arg(long = "workbench")]
        workbench_id: Option<String>,
        #[arg(long = "os")]
        os: Option<String>,
        #[arg(long, default_value = "architect-agent")]
        created_by: String,
        #[arg(long, default_value = "worker-agent")]
        owner: String,
        #[arg(long, default_value = "normal")]
        priority: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, default_value_t = false)]
        wait: bool,
        #[arg(long, default_value_t = 300)]
        wait_timeout_seconds: u64,
        #[arg(long, default_value = "text")]
        output: String,
    },
    SubmitGit {
        #[arg(long)]
        operation: String,
        #[arg(long)]
        repo: Option<String>,
        #[arg(long)]
        dest: Option<String>,
        #[arg(long)]
        repo_dir: Option<String>,
        #[arg(long)]
        branch: Option<String>,
        #[arg(long)]
        reference: Option<String>,
        #[arg(long)]
        depth: Option<u32>,
        #[arg(long = "node")]
        node_id: Option<String>,
        #[arg(long = "workbench")]
        workbench_id: Option<String>,
        #[arg(long = "os")]
        os: Option<String>,
        #[arg(long, default_value = "architect-agent")]
        created_by: String,
        #[arg(long, default_value = "worker-agent")]
        owner: String,
        #[arg(long, default_value = "normal")]
        priority: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, default_value_t = false)]
        wait: bool,
        #[arg(long, default_value_t = 300)]
        wait_timeout_seconds: u64,
        #[arg(long, default_value = "text")]
        output: String,
    },
    SubmitDocker {
        #[arg(long)]
        operation: String,
        #[arg(long)]
        image: Option<String>,
        #[arg(long = "arg")]
        args: Vec<String>,
        #[arg(long, default_value_t = 60)]
        timeout_seconds: u64,
        #[arg(long = "node")]
        node_id: Option<String>,
        #[arg(long = "workbench")]
        workbench_id: Option<String>,
        #[arg(long = "os")]
        os: Option<String>,
        #[arg(long, default_value = "architect-agent")]
        created_by: String,
        #[arg(long, default_value = "worker-agent")]
        owner: String,
        #[arg(long, default_value = "normal")]
        priority: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, default_value_t = false)]
        wait: bool,
        #[arg(long, default_value_t = 300)]
        wait_timeout_seconds: u64,
        #[arg(long, default_value = "text")]
        output: String,
    },
    SubmitBrowser {
        #[arg(long)]
        url: String,
        #[arg(long)]
        selector: Option<String>,
        #[arg(long, default_value_t = 30)]
        timeout_seconds: u64,
        #[arg(long, default_value_t = 65536)]
        max_response_bytes: u64,
        #[arg(long = "node")]
        node_id: Option<String>,
        #[arg(long = "workbench")]
        workbench_id: Option<String>,
        #[arg(long = "os")]
        os: Option<String>,
        #[arg(long, default_value = "architect-agent")]
        created_by: String,
        #[arg(long, default_value = "worker-agent")]
        owner: String,
        #[arg(long, default_value = "normal")]
        priority: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, default_value_t = false)]
        wait: bool,
        #[arg(long, default_value_t = 300)]
        wait_timeout_seconds: u64,
        #[arg(long, default_value = "text")]
        output: String,
    },
    SubmitDesktopScreenshot {
        #[arg(long = "node")]
        node_id: Option<String>,
        #[arg(long = "workbench")]
        workbench_id: Option<String>,
        #[arg(long)]
        path: Option<String>,
        #[arg(long, default_value_t = 30)]
        timeout_seconds: u64,
        #[arg(long, default_value = "architect-agent")]
        created_by: String,
        #[arg(long, default_value = "worker-agent")]
        owner: String,
        #[arg(long, default_value = "normal")]
        priority: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, default_value_t = false)]
        wait: bool,
        #[arg(long, default_value_t = 300)]
        wait_timeout_seconds: u64,
        #[arg(long, default_value = "text")]
        output: String,
    },
    SubmitDesktopClick {
        #[arg(long = "node")]
        node_id: Option<String>,
        #[arg(long = "workbench")]
        workbench_id: Option<String>,
        #[arg(long)]
        x: i32,
        #[arg(long)]
        y: i32,
        #[arg(long, default_value = "left")]
        button: String,
        #[arg(long, default_value_t = 10)]
        timeout_seconds: u64,
        #[arg(long, default_value = "architect-agent")]
        created_by: String,
        #[arg(long, default_value = "worker-agent")]
        owner: String,
        #[arg(long, default_value = "normal")]
        priority: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, default_value_t = false)]
        wait: bool,
        #[arg(long, default_value_t = 300)]
        wait_timeout_seconds: u64,
        #[arg(long, default_value = "text")]
        output: String,
    },
    SubmitDesktopTypeText {
        #[arg(long = "node")]
        node_id: Option<String>,
        #[arg(long = "workbench")]
        workbench_id: Option<String>,
        #[arg(long)]
        text: String,
        #[arg(long)]
        interval_ms: Option<u64>,
        #[arg(long, default_value_t = 30)]
        timeout_seconds: u64,
        #[arg(long, default_value = "architect-agent")]
        created_by: String,
        #[arg(long, default_value = "worker-agent")]
        owner: String,
        #[arg(long, default_value = "normal")]
        priority: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, default_value_t = false)]
        wait: bool,
        #[arg(long, default_value_t = 300)]
        wait_timeout_seconds: u64,
        #[arg(long, default_value = "text")]
        output: String,
    },
    SubmitDesktopKey {
        #[arg(long = "node")]
        node_id: Option<String>,
        #[arg(long = "workbench")]
        workbench_id: Option<String>,
        #[arg(long)]
        key: String,
        #[arg(long = "modifier")]
        modifiers: Vec<String>,
        #[arg(long, default_value_t = 10)]
        timeout_seconds: u64,
        #[arg(long, default_value = "architect-agent")]
        created_by: String,
        #[arg(long, default_value = "worker-agent")]
        owner: String,
        #[arg(long, default_value = "normal")]
        priority: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, default_value_t = false)]
        wait: bool,
        #[arg(long, default_value_t = 300)]
        wait_timeout_seconds: u64,
        #[arg(long, default_value = "text")]
        output: String,
    },
    SubmitAgentMessage {
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: Vec<String>,
        #[arg(long = "type", default_value = "broadcast.notice")]
        message_type: String,
        #[arg(long)]
        subject: String,
        #[arg(long)]
        summary: String,
        #[arg(long)]
        payload: Option<String>,
        #[arg(long = "node")]
        node_id: Option<String>,
        #[arg(long = "workbench")]
        workbench_id: Option<String>,
        #[arg(long = "os")]
        os: Option<String>,
        #[arg(long, default_value = "architect-agent")]
        created_by: String,
        #[arg(long, default_value = "worker-agent")]
        owner: String,
        #[arg(long, default_value = "normal")]
        priority: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, default_value_t = false)]
        wait: bool,
        #[arg(long, default_value_t = 300)]
        wait_timeout_seconds: u64,
        #[arg(long, default_value = "text")]
        output: String,
    },
    SubmitPlugin {
        #[arg(long)]
        plugin_id: String,
        #[arg(long, default_value = "run")]
        action: String,
        #[arg(long, conflicts_with_all = ["input_file", "input_stdin", "input_base64"])]
        input: Option<String>,
        #[arg(long = "input-file", conflicts_with_all = ["input", "input_stdin", "input_base64"])]
        input_file: Option<String>,
        #[arg(long = "input-stdin", default_value_t = false, conflicts_with_all = ["input", "input_file", "input_base64"])]
        input_stdin: bool,
        #[arg(long = "input-base64", conflicts_with_all = ["input", "input_file", "input_stdin"])]
        input_base64: Option<String>,
        #[arg(long, default_value_t = 60)]
        timeout_seconds: u64,
        #[arg(long = "node")]
        node_id: Option<String>,
        #[arg(long = "workbench")]
        workbench_id: Option<String>,
        #[arg(long = "os")]
        os: Option<String>,
        #[arg(long, default_value = "architect-agent")]
        created_by: String,
        #[arg(long, default_value = "worker-agent")]
        owner: String,
        #[arg(long, default_value = "normal")]
        priority: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, default_value_t = false)]
        wait: bool,
        #[arg(long, default_value_t = 300)]
        wait_timeout_seconds: u64,
        #[arg(long, default_value = "text")]
        output: String,
    },
    Send {
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: Vec<String>,
        #[arg(long = "type")]
        message_type: String,
        #[arg(long)]
        subject: String,
        #[arg(long)]
        summary: String,
    },
}

#[derive(Debug, Subcommand)]
enum TaskCommands {
    List,
    Get {
        #[arg(long)]
        id: String,
    },
    Logs {
        #[arg(long)]
        id: String,
    },
    Watch {
        #[arg(long)]
        id: String,
        #[arg(long, default_value_t = 300)]
        timeout_seconds: u64,
    },
    Cancel {
        #[arg(long)]
        id: String,
        #[arg(long, default_value = "任务已取消")]
        reason: String,
    },
    Explain {
        #[arg(long)]
        id: String,
    },
}

#[derive(Debug, Subcommand)]
enum JobCommands {
    List,
    Get {
        #[arg(long)]
        id: String,
    },
    Execution {
        #[arg(long)]
        id: String,
    },
    RecoveryScan,
    Plan {
        #[arg(long)]
        tool: String,
        #[arg(long, conflicts_with_all = ["payload_file", "payload_stdin", "payload_base64"])]
        payload: Option<String>,
        #[arg(long = "payload-file", conflicts_with_all = ["payload", "payload_stdin", "payload_base64"])]
        payload_file: Option<String>,
        #[arg(long = "payload-stdin", default_value_t = false, conflicts_with_all = ["payload", "payload_file", "payload_base64"])]
        payload_stdin: bool,
        #[arg(long = "payload-base64", conflicts_with_all = ["payload", "payload_file", "payload_stdin"])]
        payload_base64: Option<String>,
        #[arg(long, default_value = "Job dry-run")]
        title: String,
        #[arg(long)]
        node: Option<String>,
        #[arg(long)]
        workbench: Option<String>,
        #[arg(long)]
        os: Option<String>,
        #[arg(long, default_value_t = 3)]
        max_attempts: i64,
        #[arg(long, default_value_t = 1)]
        shards: i64,
        #[arg(long)]
        max_parallelism: Option<i64>,
        #[arg(long, default_value = "summary")]
        reduce: String,
        #[arg(long)]
        partition_items: Option<String>,
        #[arg(long)]
        partition_range: Option<String>,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Submit {
        #[arg(long)]
        tool: String,
        #[arg(long, conflicts_with_all = ["payload_file", "payload_stdin", "payload_base64"])]
        payload: Option<String>,
        #[arg(long = "payload-file", conflicts_with_all = ["payload", "payload_stdin", "payload_base64"])]
        payload_file: Option<String>,
        #[arg(long = "payload-stdin", default_value_t = false, conflicts_with_all = ["payload", "payload_file", "payload_base64"])]
        payload_stdin: bool,
        #[arg(long = "payload-base64", conflicts_with_all = ["payload", "payload_file", "payload_stdin"])]
        payload_base64: Option<String>,
        #[arg(long)]
        title: String,
        #[arg(long)]
        node: Option<String>,
        #[arg(long)]
        workbench: Option<String>,
        #[arg(long)]
        os: Option<String>,
        #[arg(long, default_value_t = 3)]
        max_attempts: i64,
        #[arg(long, default_value_t = 1)]
        shards: i64,
        #[arg(long)]
        max_parallelism: Option<i64>,
        #[arg(long, default_value = "summary")]
        reduce: String,
        #[arg(long)]
        partition_items: Option<String>,
        #[arg(long)]
        partition_range: Option<String>,
        #[arg(long)]
        idempotency_key: Option<String>,
        #[arg(long, default_value_t = false)]
        wait: bool,
        #[arg(long, default_value_t = 300)]
        wait_timeout_seconds: u64,
    },
    Checkpoint {
        #[arg(long)]
        id: String,
        #[arg(long)]
        attempt: Option<String>,
        #[arg(long)]
        task: Option<String>,
        #[arg(long)]
        node: Option<String>,
        #[arg(long, default_value_t = 0)]
        sequence: i64,
        #[arg(long, default_value_t = 0)]
        progress: i64,
        #[arg(long, default_value = "{}")]
        resume_token: String,
    },
    Event {
        #[arg(long)]
        source: String,
        #[arg(long = "type")]
        event_type: String,
        #[arg(long)]
        job: Option<String>,
        #[arg(long)]
        node: Option<String>,
        #[arg(long, default_value = "{}")]
        payload: String,
    },
}

#[derive(Debug, Subcommand)]
enum NodeCommands {
    List,
    Get {
        #[arg(long)]
        id: String,
    },
}

#[derive(Debug, Subcommand)]
enum WorkbenchCommands {
    List,
    Get {
        #[arg(long)]
        id: String,
    },
    Timeline {
        #[arg(long)]
        id: String,
    },
    Command {
        #[arg(long)]
        id: String,
        #[arg(long)]
        program: String,
        #[arg(long = "arg")]
        args: Vec<String>,
        #[arg(long)]
        working_dir: Option<String>,
        #[arg(long, default_value_t = 30)]
        timeout_seconds: u64,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, default_value_t = false)]
        wait: bool,
        #[arg(long, default_value_t = 300)]
        wait_timeout_seconds: u64,
        #[arg(long, default_value = "text")]
        output: String,
    },
    Screenshot {
        #[arg(long)]
        id: String,
        #[arg(long)]
        path: Option<String>,
        #[arg(long, default_value_t = 30)]
        timeout_seconds: u64,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, default_value_t = false)]
        wait: bool,
        #[arg(long, default_value_t = 300)]
        wait_timeout_seconds: u64,
        #[arg(long, default_value = "text")]
        output: String,
    },
    File {
        #[arg(long)]
        id: String,
        #[arg(long)]
        operation: String,
        #[arg(long)]
        path: String,
        #[arg(long)]
        content: Option<String>,
        #[arg(long, default_value_t = false)]
        append: bool,
        #[arg(long, default_value_t = true)]
        create_dirs: bool,
        #[arg(long, default_value_t = false)]
        recursive: bool,
        #[arg(long)]
        max_bytes: Option<u64>,
        #[arg(long)]
        max_entries: Option<u64>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, default_value_t = false)]
        wait: bool,
        #[arg(long, default_value_t = 300)]
        wait_timeout_seconds: u64,
        #[arg(long, default_value = "text")]
        output: String,
    },
    Runtime {
        #[arg(long)]
        id: String,
        #[arg(long)]
        tool: String,
        #[arg(long, conflicts_with_all = ["payload_file", "payload_stdin", "payload_base64"])]
        payload: Option<String>,
        #[arg(long = "payload-file", conflicts_with_all = ["payload", "payload_stdin", "payload_base64"])]
        payload_file: Option<String>,
        #[arg(long = "payload-stdin", default_value_t = false, conflicts_with_all = ["payload", "payload_file", "payload_base64"])]
        payload_stdin: bool,
        #[arg(long = "payload-base64", conflicts_with_all = ["payload", "payload_file", "payload_stdin"])]
        payload_base64: Option<String>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, default_value_t = false)]
        wait: bool,
        #[arg(long, default_value_t = 300)]
        wait_timeout_seconds: u64,
        #[arg(long, default_value = "text")]
        output: String,
    },
    Action {
        #[arg(long)]
        id: String,
        #[arg(long)]
        action: String,
        #[arg(long, conflicts_with_all = ["payload_file", "payload_stdin", "payload_base64"])]
        payload: Option<String>,
        #[arg(long = "payload-file", conflicts_with_all = ["payload", "payload_stdin", "payload_base64"])]
        payload_file: Option<String>,
        #[arg(long = "payload-stdin", default_value_t = false, conflicts_with_all = ["payload", "payload_file", "payload_base64"])]
        payload_stdin: bool,
        #[arg(long = "payload-base64", conflicts_with_all = ["payload", "payload_file", "payload_stdin"])]
        payload_base64: Option<String>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, default_value_t = false)]
        wait: bool,
        #[arg(long, default_value_t = 300)]
        wait_timeout_seconds: u64,
        #[arg(long, default_value = "text")]
        output: String,
    },
}

#[derive(Debug, Subcommand)]
enum NodeToolCommands {
    List,
    Get {
        #[arg(long)]
        id: String,
    },
    Node {
        #[arg(long)]
        node: String,
    },
    Register {
        #[arg(long)]
        node: String,
        #[arg(long)]
        file: String,
    },
    Probe {
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        node: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum WorkflowCommands {
    List {
        #[arg(long, default_value_t = 100)]
        limit: u16,
    },
    Get {
        #[arg(long)]
        id: String,
    },
    Submit {
        #[arg(long)]
        file: String,
        #[arg(long, default_value_t = true)]
        start: bool,
        #[arg(long, default_value_t = false)]
        wait: bool,
        #[arg(long, default_value_t = 900)]
        timeout_seconds: u64,
        #[arg(long, default_value = "text")]
        output: String,
    },
    Start {
        #[arg(long)]
        id: String,
        #[arg(long, default_value_t = false)]
        wait: bool,
        #[arg(long, default_value_t = 900)]
        timeout_seconds: u64,
        #[arg(long, default_value = "text")]
        output: String,
    },
    Watch {
        #[arg(long)]
        id: String,
        #[arg(long, default_value_t = 900)]
        timeout_seconds: u64,
        #[arg(long, default_value = "text")]
        output: String,
    },
    Cancel {
        #[arg(long)]
        id: String,
        #[arg(long, default_value = "命令行取消工作流")]
        reason: String,
    },
}

#[derive(Debug, Subcommand)]
enum ToolCommands {
    List,
    ProbeCenter,
    RemediationCenter,
    Get {
        #[arg(long)]
        id: String,
    },
    Nodes {
        #[arg(long)]
        id: String,
    },
    Probes,
    Probe {
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        node: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum StandardCommands {
    All,
    ToolContracts,
    Capabilities,
    StateMachine,
    WorkflowTemplate,
    ResultReport,
    Workbench,
    Devices,
    Evidence,
    Runbook,
    MobileSdk,
    PluginRuntime,
    CapabilityGraph,
    ExecutionContract,
    EvidencePipeline,
    ProbeEngine,
    PlacementEngine,
    TaskIntent,
    ArtifactStore,
    EventTimeline,
}

#[derive(Debug, Subcommand)]
enum RuntimeCommands {
    Manifest,
    Submit {
        #[arg(long)]
        tool: String,
        #[arg(long, conflicts_with_all = ["payload_file", "payload_stdin", "payload_base64"])]
        payload: Option<String>,
        #[arg(long = "payload-file", conflicts_with_all = ["payload", "payload_stdin", "payload_base64"])]
        payload_file: Option<String>,
        #[arg(long = "payload-stdin", default_value_t = false, conflicts_with_all = ["payload", "payload_file", "payload_base64"])]
        payload_stdin: bool,
        #[arg(long = "payload-base64", conflicts_with_all = ["payload", "payload_file", "payload_stdin"])]
        payload_base64: Option<String>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        node: Option<String>,
        #[arg(long)]
        workbench: Option<String>,
        #[arg(long)]
        os: Option<String>,
        #[arg(long)]
        verify_json: Option<String>,
        #[arg(long, default_value_t = false)]
        wait: bool,
        #[arg(long, default_value_t = 300)]
        wait_timeout_seconds: u64,
        #[arg(long, default_value = "text")]
        output: String,
    },
    Get {
        #[arg(long)]
        id: String,
    },
}

#[derive(Debug, Subcommand)]
enum RecordCommands {
    Task {
        #[arg(long)]
        id: String,
    },
    Workflow {
        #[arg(long)]
        id: String,
    },
}

#[derive(Debug, Subcommand)]
enum TaskTemplateCommands {
    List,
    Get {
        #[arg(long)]
        id: String,
    },
    Start {
        #[arg(long)]
        id: String,
        #[arg(long = "param")]
        params: Vec<String>,
        #[arg(long)]
        parameters_json: Option<String>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long = "node")]
        node_id: Option<String>,
        #[arg(long = "os")]
        os: Option<String>,
        #[arg(long, default_value = "template-store-cli")]
        created_by: String,
        #[arg(long, default_value_t = false)]
        wait: bool,
        #[arg(long, default_value_t = 300)]
        wait_timeout_seconds: u64,
        #[arg(long, default_value = "text")]
        output: String,
    },
}

#[derive(Debug, Subcommand)]
enum WebhookCommands {
    List,
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        url: String,
        #[arg(long = "event")]
        events: Vec<String>,
        #[arg(long)]
        secret: Option<String>,
        #[arg(long, default_value = "architect-agent")]
        created_by: String,
        #[arg(long, default_value_t = true)]
        enabled: bool,
    },
    Delete {
        #[arg(long)]
        id: String,
    },
    Deliveries {
        #[arg(long, default_value_t = 200)]
        limit: u16,
    },
}

#[derive(Debug, Subcommand)]
enum PortBridgeCommands {
    List,
    Get {
        #[arg(long)]
        id: String,
    },
    Create {
        #[arg(long = "source-node")]
        source_node: String,
        #[arg(long = "target-node")]
        target_node: String,
        #[arg(long = "target-port")]
        target_port: u16,
        #[arg(long = "source-port", default_value_t = 0)]
        source_port: u16,
        #[arg(long = "target-host", default_value = "127.0.0.1")]
        target_host: String,
        #[arg(long = "bind-host", default_value = "127.0.0.1")]
        bind_host: String,
        #[arg(long, default_value_t = 1800)]
        ttl_seconds: u64,
        #[arg(long, default_value = "tcp")]
        protocol: String,
        #[arg(long)]
        purpose: Option<String>,
        #[arg(long, default_value = "agentgrid-cli")]
        created_by: String,
        #[arg(long, default_value = "text")]
        output: String,
    },
    Close {
        #[arg(long)]
        id: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let client = reqwest::blocking::Client::new();
    let base = cli.hub.trim_end_matches('/');

    match cli.command {
        Commands::Health => print_json(client.get(format!("{base}/api/health")).send()?.text()?),
        Commands::Agents => print_json(client.get(format!("{base}/api/agents")).send()?.text()?),
        Commands::Messages { limit } => print_json(
            client
                .get(format!("{base}/api/messages?limit={limit}"))
                .send()?
                .text()?,
        ),
        Commands::Nodes { command } => match command.unwrap_or(NodeCommands::List) {
            NodeCommands::List => {
                print_json(client.get(format!("{base}/api/nodes")).send()?.text()?)
            }
            NodeCommands::Get { id } => {
                let nodes = fetch_json(&client, &format!("{base}/api/nodes"))?;
                let node = nodes
                    .get("items")
                    .and_then(Value::as_array)
                    .and_then(|items| {
                        items.iter().find(|item| {
                            item.pointer("/metadata/id").and_then(Value::as_str)
                                == Some(id.as_str())
                        })
                    })
                    .cloned()
                    .context("node not found")?;
                println!("{}", serde_json::to_string_pretty(&node)?);
            }
        },
        Commands::Workbench { command } => match command.unwrap_or(WorkbenchCommands::List) {
            WorkbenchCommands::List => print_json(
                client
                    .get(format!("{base}/api/workbenches"))
                    .send()?
                    .text()?,
            ),
            WorkbenchCommands::Get { id } => print_json(
                client
                    .get(format!("{base}/api/workbenches/{id}"))
                    .send()?
                    .text()?,
            ),
            WorkbenchCommands::Timeline { id } => print_json(
                client
                    .get(format!("{base}/api/workbenches/{id}/timeline"))
                    .send()?
                    .text()?,
            ),
            WorkbenchCommands::Command {
                id,
                program,
                args,
                working_dir,
                timeout_seconds,
                title,
                wait,
                wait_timeout_seconds,
                output,
            } => {
                let payload = serde_json::json!({
                    "program": program,
                    "args": args,
                    "working_dir": working_dir,
                    "timeout_seconds": timeout_seconds
                });
                let task_title = title.unwrap_or_else(|| {
                    format!("{} 执行 {}", id, payload["program"].as_str().unwrap_or(""))
                });
                submit_workbench_action(
                    &client,
                    base,
                    id,
                    "command.run".to_string(),
                    payload,
                    task_title,
                    wait,
                    wait_timeout_seconds,
                    &output,
                )?;
            }
            WorkbenchCommands::Screenshot {
                id,
                path,
                timeout_seconds,
                title,
                wait,
                wait_timeout_seconds,
                output,
            } => {
                let payload = serde_json::json!({
                    "operation": "screenshot",
                    "path": path,
                    "timeout_seconds": timeout_seconds
                });
                let task_title = title.unwrap_or_else(|| format!("{id} 截取桌面"));
                submit_workbench_action(
                    &client,
                    base,
                    id,
                    "desktop.screenshot".to_string(),
                    payload,
                    task_title,
                    wait,
                    wait_timeout_seconds,
                    &output,
                )?;
            }
            WorkbenchCommands::File {
                id,
                operation,
                path,
                content,
                append,
                create_dirs,
                recursive,
                max_bytes,
                max_entries,
                title,
                wait,
                wait_timeout_seconds,
                output,
            } => {
                let payload = file_task_payload(
                    &operation,
                    path,
                    content,
                    append,
                    create_dirs,
                    recursive,
                    max_bytes,
                    max_entries,
                )?;
                let task_title = title.unwrap_or_else(|| {
                    format!(
                        "{} 文件任务 {} {}",
                        id,
                        operation,
                        payload["path"].as_str().unwrap_or("")
                    )
                });
                submit_workbench_action(
                    &client,
                    base,
                    id,
                    format!("file.{operation}"),
                    payload,
                    task_title,
                    wait,
                    wait_timeout_seconds,
                    &output,
                )?;
            }
            WorkbenchCommands::Runtime {
                id,
                tool,
                payload,
                payload_file,
                payload_stdin,
                payload_base64,
                title,
                wait,
                wait_timeout_seconds,
                output,
            } => {
                let payload = read_json_payload(
                    "--payload",
                    payload,
                    payload_file,
                    payload_stdin,
                    payload_base64,
                    Some(serde_json::json!({})),
                )?;
                let body = serde_json::json!({
                    "action": "runtime.submit",
                    "payload": {
                        "tool_id": tool,
                        "payload": payload
                    },
                    "title": title,
                    "created_by": "workbench-cli"
                });
                handle_workbench_action_response(
                    &client,
                    base,
                    client
                        .post(format!("{base}/api/workbenches/{id}/actions"))
                        .json(&body)
                        .send()?
                        .text()?,
                    wait,
                    wait_timeout_seconds,
                    &output,
                )?;
            }
            WorkbenchCommands::Action {
                id,
                action,
                payload,
                payload_file,
                payload_stdin,
                payload_base64,
                title,
                wait,
                wait_timeout_seconds,
                output,
            } => {
                let payload = read_json_payload(
                    "--payload",
                    payload,
                    payload_file,
                    payload_stdin,
                    payload_base64,
                    Some(serde_json::json!({})),
                )?;
                submit_workbench_action(
                    &client,
                    base,
                    id,
                    action,
                    payload,
                    title.unwrap_or_else(|| "Workbench Action".to_string()),
                    wait,
                    wait_timeout_seconds,
                    &output,
                )?;
            }
        },
        Commands::NodeTools { command } => match command.unwrap_or(NodeToolCommands::List) {
            NodeToolCommands::List => print_json(
                client
                    .get(format!("{base}/api/node-tools"))
                    .send()?
                    .text()?,
            ),
            NodeToolCommands::Get { id } => print_json(
                client
                    .get(format!("{base}/api/node-tools/{id}"))
                    .send()?
                    .text()?,
            ),
            NodeToolCommands::Node { node } => print_json(
                client
                    .get(format!("{base}/api/nodes/{node}/tools"))
                    .send()?
                    .text()?,
            ),
            NodeToolCommands::Register { node, file } => {
                let raw = fs::read_to_string(&file)
                    .with_context(|| format!("failed to read node tool file: {file}"))?;
                let body: Value = serde_json::from_str(&raw)
                    .with_context(|| format!("node tool file is not valid JSON: {file}"))?;
                print_json(
                    client
                        .post(format!("{base}/api/nodes/{node}/tools"))
                        .json(&body)
                        .send()?
                        .text()?,
                )
            }
            NodeToolCommands::Probe { id, node } => {
                let url = match (id, node) {
                    (Some(id), Some(node)) => {
                        format!("{base}/api/node-tools/{id}/nodes/{node}/probe")
                    }
                    (Some(id), None) => format!("{base}/api/node-tools/{id}/probe"),
                    (None, None) => format!("{base}/api/node-tools/probe"),
                    (None, Some(_)) => anyhow::bail!("--node requires --id"),
                };
                print_json(client.post(url).send()?.text()?)
            }
        },
        Commands::Policy => print_json(client.get(format!("{base}/api/policy")).send()?.text()?),
        Commands::Capabilities => print_json(
            client
                .get(format!("{base}/api/capabilities/manifest"))
                .send()?
                .text()?,
        ),
        Commands::Standard { command } => {
            let path = match command.unwrap_or(StandardCommands::All) {
                StandardCommands::All => "/api/runtime-standard",
                StandardCommands::ToolContracts => "/api/runtime-standard/tool-contracts",
                StandardCommands::Capabilities => "/api/runtime-standard/capabilities",
                StandardCommands::StateMachine => "/api/runtime-standard/state-machine",
                StandardCommands::WorkflowTemplate => "/api/runtime-standard/workflow-template",
                StandardCommands::ResultReport => "/api/runtime-standard/result-report",
                StandardCommands::Workbench => "/api/runtime-standard/workbench",
                StandardCommands::Devices => "/api/runtime-standard/devices",
                StandardCommands::Evidence => "/api/runtime-standard/evidence",
                StandardCommands::Runbook => "/api/runtime-standard/runbook",
                StandardCommands::MobileSdk => "/api/runtime-standard/mobile-sdk",
                StandardCommands::PluginRuntime => "/api/runtime-standard/plugin-runtime",
                StandardCommands::CapabilityGraph => "/api/runtime-standard/capability-graph",
                StandardCommands::ExecutionContract => "/api/runtime-standard/execution-contract",
                StandardCommands::EvidencePipeline => "/api/runtime-standard/evidence-pipeline",
                StandardCommands::ProbeEngine => "/api/runtime-standard/probe-engine",
                StandardCommands::PlacementEngine => "/api/runtime-standard/placement-engine",
                StandardCommands::TaskIntent => "/api/runtime-standard/task-intent",
                StandardCommands::ArtifactStore => "/api/runtime-standard/artifact-store",
                StandardCommands::EventTimeline => "/api/runtime-standard/event-timeline",
            };
            print_json(client.get(format!("{base}{path}")).send()?.text()?)
        }
        Commands::Tools { command } => match command.unwrap_or(ToolCommands::List) {
            ToolCommands::List => {
                print_json(client.get(format!("{base}/api/tools")).send()?.text()?)
            }
            ToolCommands::ProbeCenter => print_json(
                client
                    .get(format!("{base}/api/tools/probe-center"))
                    .send()?
                    .text()?,
            ),
            ToolCommands::RemediationCenter => print_json(
                client
                    .get(format!("{base}/api/tools/remediation-center"))
                    .send()?
                    .text()?,
            ),
            ToolCommands::Get { id } => print_json(
                client
                    .get(format!("{base}/api/tools/{id}"))
                    .send()?
                    .text()?,
            ),
            ToolCommands::Nodes { id } => print_json(
                client
                    .get(format!("{base}/api/tools/{id}/nodes"))
                    .send()?
                    .text()?,
            ),
            ToolCommands::Probes => print_json(
                client
                    .get(format!("{base}/api/tools/probes"))
                    .send()?
                    .text()?,
            ),
            ToolCommands::Probe { id, node } => {
                let url = match (id, node) {
                    (Some(id), Some(node)) => format!("{base}/api/tools/{id}/nodes/{node}/probe"),
                    (Some(id), None) => format!("{base}/api/tools/{id}/probe"),
                    (None, None) => format!("{base}/api/tools/probe"),
                    (None, Some(_)) => anyhow::bail!("--node requires --id"),
                };
                print_json(client.post(url).send()?.text()?)
            }
        },
        Commands::Tasks { command } => match command.unwrap_or(TaskCommands::List) {
            TaskCommands::List => {
                print_json(client.get(format!("{base}/api/tasks")).send()?.text()?)
            }
            TaskCommands::Get { id } => print_json(
                client
                    .get(format!("{base}/api/tasks/{id}"))
                    .send()?
                    .text()?,
            ),
            TaskCommands::Logs { id } => {
                let snapshot = fetch_json(&client, &format!("{base}/api/tasks/{id}/snapshot"))?;
                print_task_logs(&snapshot);
            }
            TaskCommands::Watch {
                id,
                timeout_seconds,
            } => {
                let snapshot = watch_task(&client, base, &id, timeout_seconds)?;
                print_task_summary(&snapshot, "text");
            }
            TaskCommands::Cancel { id, reason } => print_json(
                client
                    .post(format!("{base}/api/tasks/{id}/control"))
                    .json(&serde_json::json!({
                        "action": "cancel",
                        "actor": "agentgrid-cli",
                        "reason": reason
                    }))
                    .send()?
                    .text()?,
            ),
            TaskCommands::Explain { id } => print_json(
                client
                    .get(format!("{base}/api/tasks/{id}/schedule-preview"))
                    .send()?
                    .text()?,
            ),
        },
        Commands::Jobs { command } => match command.unwrap_or(JobCommands::List) {
            JobCommands::List => print_json(client.get(format!("{base}/api/jobs")).send()?.text()?),
            JobCommands::Get { id } => {
                print_json(client.get(format!("{base}/api/jobs/{id}")).send()?.text()?)
            }
            JobCommands::Execution { id } => print_json(
                client
                    .get(format!("{base}/api/jobs/{id}/execution"))
                    .send()?
                    .text()?,
            ),
            JobCommands::RecoveryScan => print_json(
                client
                    .post(format!("{base}/api/jobs/recovery/scan"))
                    .send()?
                    .text()?,
            ),
            JobCommands::Plan {
                tool,
                payload,
                payload_file,
                payload_stdin,
                payload_base64,
                title,
                node,
                workbench,
                os,
                max_attempts,
                shards,
                max_parallelism,
                reduce,
                partition_items,
                partition_range,
                idempotency_key,
            } => {
                let payload = read_json_payload(
                    "--payload",
                    payload,
                    payload_file,
                    payload_stdin,
                    payload_base64,
                    Some(serde_json::json!({})),
                )?;
                let partition = job_partition_payload(partition_items, partition_range)?;
                print_json(
                    client
                        .post(format!("{base}/api/jobs/plan"))
                        .json(&job_request_json(
                            tool,
                            title,
                            payload,
                            node,
                            workbench,
                            os,
                            max_attempts,
                            shards,
                            max_parallelism,
                            reduce,
                            partition,
                            idempotency_key,
                        ))
                        .send()?
                        .text()?,
                )
            }
            JobCommands::Submit {
                tool,
                payload,
                payload_file,
                payload_stdin,
                payload_base64,
                title,
                node,
                workbench,
                os,
                max_attempts,
                shards,
                max_parallelism,
                reduce,
                partition_items,
                partition_range,
                idempotency_key,
                wait,
                wait_timeout_seconds,
            } => {
                let payload = read_json_payload(
                    "--payload",
                    payload,
                    payload_file,
                    payload_stdin,
                    payload_base64,
                    Some(serde_json::json!({})),
                )?;
                let partition = job_partition_payload(partition_items, partition_range)?;
                let raw = client
                    .post(format!("{base}/api/jobs"))
                    .json(&job_request_json(
                        tool,
                        title,
                        payload,
                        node,
                        workbench,
                        os,
                        max_attempts,
                        shards,
                        max_parallelism,
                        reduce,
                        partition,
                        idempotency_key,
                    ))
                    .send()?
                    .text()?;
                let value: Value = serde_json::from_str(&raw)?;
                let job_id = value
                    .pointer("/item/metadata/id")
                    .and_then(Value::as_str)
                    .context("job id missing in response")?;
                if wait {
                    let job = wait_for_job(&client, base, job_id, wait_timeout_seconds)?;
                    println!("{}", serde_json::to_string_pretty(&job)?);
                } else {
                    println!("{}", serde_json::to_string_pretty(&value)?);
                }
            }
            JobCommands::Checkpoint {
                id,
                attempt,
                task,
                node,
                sequence,
                progress,
                resume_token,
            } => {
                let token: Value = serde_json::from_str(&resume_token)
                    .with_context(|| "--resume-token must be valid JSON")?;
                print_json(
                    client
                        .post(format!("{base}/api/jobs/{id}/checkpoints"))
                        .json(&serde_json::json!({
                            "attempt_id": attempt,
                            "task_id": task,
                            "node_id": node,
                            "sequence": sequence,
                            "progress": progress,
                            "resume_token": token,
                            "artifacts": []
                        }))
                        .send()?
                        .text()?,
                )
            }
            JobCommands::Event {
                source,
                event_type,
                job,
                node,
                payload,
            } => {
                let payload: Value = serde_json::from_str(&payload)
                    .with_context(|| "--payload must be valid JSON")?;
                print_json(
                    client
                        .post(format!("{base}/api/events/ingress"))
                        .json(&serde_json::json!({
                            "source": source,
                            "type": event_type,
                            "target": {
                                "job_id": job,
                                "node_id": node
                            },
                            "idempotency_key": format!("evt-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|value| value.as_nanos()).unwrap_or_default()),
                            "payload": payload,
                            "ttl_seconds": 300
                        }))
                        .send()?
                        .text()?,
                )
            }
        },
        Commands::Workflows { command } => match command
            .unwrap_or(WorkflowCommands::List { limit: 100 })
        {
            WorkflowCommands::List { limit } => print_json(
                client
                    .get(format!("{base}/api/workflows?limit={limit}"))
                    .send()?
                    .text()?,
            ),
            WorkflowCommands::Get { id } => print_json(
                client
                    .get(format!("{base}/api/workflows/{id}"))
                    .send()?
                    .text()?,
            ),
            WorkflowCommands::Submit {
                file,
                start,
                wait,
                timeout_seconds,
                output,
            } => {
                let raw = fs::read_to_string(&file)
                    .with_context(|| format!("failed to read workflow file: {file}"))?;
                let body: Value = serde_json::from_str(&raw)
                    .with_context(|| format!("workflow file is not valid JSON: {file}"))?;
                let created = post_json(&client, &format!("{base}/api/workflows"), &body)?;
                let workflow_id = created
                    .pointer("/item/metadata/id")
                    .and_then(Value::as_str)
                    .context("workflow id missing in create response")?
                    .to_string();
                let current = if start {
                    post_json(
                        &client,
                        &format!("{base}/api/workflows/{workflow_id}/start"),
                        &serde_json::json!({ "actor": "architect-agent" }),
                    )?
                } else {
                    created
                };
                if wait {
                    let workflow = wait_for_workflow(&client, base, &workflow_id, timeout_seconds)?;
                    print_workflow_summary(&workflow, &output);
                } else if output == "json" {
                    println!("{}", serde_json::to_string_pretty(&current)?);
                } else {
                    println!("Workflow ID: {workflow_id}");
                    println!("State: {}", workflow_state_from_response(&current));
                    println!();
                    println!("查看详情: agentgrid workflows get --id {workflow_id}");
                    if start {
                        println!("等待完成: agentgrid workflows watch --id {workflow_id}");
                    } else {
                        println!("启动工作流: agentgrid workflows start --id {workflow_id}");
                    }
                }
            }
            WorkflowCommands::Start {
                id,
                wait,
                timeout_seconds,
                output,
            } => {
                let started = post_json(
                    &client,
                    &format!("{base}/api/workflows/{id}/start"),
                    &serde_json::json!({ "actor": "architect-agent" }),
                )?;
                if wait {
                    let workflow = wait_for_workflow(&client, base, &id, timeout_seconds)?;
                    print_workflow_summary(&workflow, &output);
                } else if output == "json" {
                    println!("{}", serde_json::to_string_pretty(&started)?);
                } else {
                    println!("Workflow ID: {id}");
                    println!("State: {}", workflow_state_from_response(&started));
                    println!("等待完成: agentgrid workflows watch --id {id}");
                }
            }
            WorkflowCommands::Watch {
                id,
                timeout_seconds,
                output,
            } => {
                let workflow = wait_for_workflow(&client, base, &id, timeout_seconds)?;
                print_workflow_summary(&workflow, &output);
            }
            WorkflowCommands::Cancel { id, reason } => print_json(
                client
                    .post(format!("{base}/api/workflows/{id}/cancel"))
                    .json(&serde_json::json!({
                        "actor": "architect-agent",
                        "reason": reason
                    }))
                    .send()?
                    .text()?,
            ),
        },
        Commands::Runtime { command } => match command.unwrap_or(RuntimeCommands::Manifest) {
            RuntimeCommands::Manifest => print_json(
                client
                    .get(format!("{base}/api/agent-runtime/manifest"))
                    .send()?
                    .text()?,
            ),
            RuntimeCommands::Submit {
                tool,
                payload,
                payload_file,
                payload_stdin,
                payload_base64,
                title,
                node,
                workbench,
                os,
                verify_json,
                wait,
                wait_timeout_seconds,
                output,
            } => {
                let payload = read_json_payload(
                    "--payload",
                    payload,
                    payload_file,
                    payload_stdin,
                    payload_base64,
                    Some(serde_json::json!({})),
                )?;
                let verify = parse_verify_json(verify_json)?;
                let mut body = serde_json::json!({
                    "tool_id": tool,
                    "payload": payload,
                    "title": title,
                    "node_id": node,
                    "workbench_id": workbench,
                    "os": os,
                    "created_by": "agent-runtime-cli"
                });
                insert_verify(&mut body, verify);
                let raw = client
                    .post(format!("{base}/api/agent-runtime/tasks"))
                    .json(&body)
                    .send()?
                    .text()?;
                let value: Value = serde_json::from_str(&raw)?;
                let task_id = value
                    .get("task_id")
                    .and_then(Value::as_str)
                    .or_else(|| value.pointer("/item/metadata/id").and_then(Value::as_str))
                    .context("task id missing in runtime response")?;
                if !wait && output == "json" {
                    println!("{}", serde_json::to_string_pretty(&value)?);
                } else if !wait {
                    println!("Task ID: {task_id}");
                    println!("查看结果: agentgrid runtime get --id {task_id}");
                    println!("等待完成: agentgrid tasks watch --id {task_id}");
                } else {
                    let snapshot = wait_for_task(&client, base, task_id, wait_timeout_seconds)?;
                    print_task_summary(&snapshot, &output);
                }
            }
            RuntimeCommands::Get { id } => print_json(
                client
                    .get(format!("{base}/api/agent-runtime/tasks/{id}"))
                    .send()?
                    .text()?,
            ),
        },
        Commands::Records { command } => match command {
            RecordCommands::Task { id } => print_json(
                client
                    .get(format!("{base}/api/execution-records/tasks/{id}"))
                    .send()?
                    .text()?,
            ),
            RecordCommands::Workflow { id } => print_json(
                client
                    .get(format!("{base}/api/execution-records/workflows/{id}"))
                    .send()?
                    .text()?,
            ),
        },
        Commands::TaskTemplates { command } => {
            match command.unwrap_or(TaskTemplateCommands::List) {
                TaskTemplateCommands::List => print_json(
                    client
                        .get(format!("{base}/api/task-templates"))
                        .send()?
                        .text()?,
                ),
                TaskTemplateCommands::Get { id } => print_json(
                    client
                        .get(format!("{base}/api/task-templates/{id}"))
                        .send()?
                        .text()?,
                ),
                TaskTemplateCommands::Start {
                    id,
                    params,
                    parameters_json,
                    title,
                    node_id,
                    os,
                    created_by,
                    wait,
                    wait_timeout_seconds,
                    output,
                } => {
                    let parameters = parse_parameters(params, parameters_json)?;
                    let mut body = serde_json::json!({
                        "parameters": parameters,
                        "title": title,
                        "node_id": node_id,
                        "os": os,
                        "created_by": created_by
                    });
                    remove_null_fields(&mut body);
                    handle_task_submit_response(
                        &client,
                        base,
                        client
                            .post(format!("{base}/api/task-templates/{id}/start"))
                            .json(&body)
                            .send()?
                            .text()?,
                        wait,
                        wait_timeout_seconds,
                        &output,
                    )?;
                }
            }
        }
        Commands::Webhooks { command } => match command.unwrap_or(WebhookCommands::List) {
            WebhookCommands::List => {
                print_json(client.get(format!("{base}/api/webhooks")).send()?.text()?)
            }
            WebhookCommands::Create {
                name,
                url,
                events,
                secret,
                created_by,
                enabled,
            } => {
                let body = serde_json::json!({
                    "name": name,
                    "url": url,
                    "events": if events.is_empty() { vec!["task.completed".to_string(), "task.failed".to_string()] } else { events },
                    "secret": secret,
                    "created_by": created_by,
                    "enabled": enabled
                });
                print_json(
                    client
                        .post(format!("{base}/api/webhooks"))
                        .json(&body)
                        .send()?
                        .text()?,
                )
            }
            WebhookCommands::Delete { id } => print_json(
                client
                    .delete(format!("{base}/api/webhooks/{id}"))
                    .send()?
                    .text()?,
            ),
            WebhookCommands::Deliveries { limit } => print_json(
                client
                    .get(format!("{base}/api/webhooks/deliveries?limit={limit}"))
                    .send()?
                    .text()?,
            ),
        },
        Commands::PortBridges { command } => match command.unwrap_or(PortBridgeCommands::List) {
            PortBridgeCommands::List => print_json(
                client
                    .get(format!("{base}/api/port-bridges"))
                    .send()?
                    .text()?,
            ),
            PortBridgeCommands::Get { id } => print_json(
                client
                    .get(format!("{base}/api/port-bridges/{id}"))
                    .send()?
                    .text()?,
            ),
            PortBridgeCommands::Create {
                source_node,
                target_node,
                target_port,
                source_port,
                target_host,
                bind_host,
                ttl_seconds,
                protocol,
                purpose,
                created_by,
                output,
            } => {
                let body = port_bridge_create_json(
                    source_node,
                    target_node,
                    target_port,
                    source_port,
                    target_host,
                    bind_host,
                    ttl_seconds,
                    protocol,
                    purpose,
                    created_by,
                );
                let value = post_json(&client, &format!("{base}/api/port-bridges"), &body)?;
                print_port_bridge_create(&value, &output)?;
            }
            PortBridgeCommands::Close { id } => print_json(
                client
                    .delete(format!("{base}/api/port-bridges/{id}"))
                    .send()?
                    .text()?,
            ),
        },
        Commands::BridgePort {
            source_node,
            target_node,
            target_port,
            source_port,
            target_host,
            bind_host,
            ttl_seconds,
            protocol,
            purpose,
            created_by,
            output,
        } => {
            let body = port_bridge_create_json(
                source_node,
                target_node,
                target_port,
                source_port,
                target_host,
                bind_host,
                ttl_seconds,
                protocol,
                purpose,
                created_by,
            );
            let value = post_json(&client, &format!("{base}/api/port-bridges"), &body)?;
            print_port_bridge_create(&value, &output)?;
        }
        Commands::SubmitHttp {
            url,
            method,
            headers,
            body,
            timeout_seconds,
            max_response_bytes,
            node_id,
            workbench_id,
            os,
            created_by,
            owner,
            priority,
            title,
            wait,
            wait_timeout_seconds,
            output,
            verify_json,
            expect_status_2xx,
            expect_status,
            expect_body_contains,
        } => {
            let payload = serde_json::json!({
                "type": "http_request",
                "method": method.to_uppercase(),
                "url": url,
                "headers": parse_headers(headers)?,
                "body": parse_body(body)?,
                "timeout_seconds": timeout_seconds,
                "max_response_bytes": max_response_bytes
            });
            let task_title = title.unwrap_or_else(|| {
                format!(
                    "HTTP {} {}",
                    payload["method"].as_str().unwrap_or("GET"),
                    payload["url"].as_str().unwrap_or("")
                )
            });
            let verify = build_http_verify(
                verify_json,
                expect_status_2xx,
                expect_status,
                expect_body_contains,
            )?;
            let mut labels = vec!["compute".to_string(), "http_request".to_string()];
            push_placement_labels(&mut labels, node_id, workbench_id, os);
            let mut body = serde_json::json!({
                "title": task_title,
                "summary": "提交一个 HTTP 请求任务，等待 Worker 执行并回传结果。",
                "created_by": created_by,
                "owner": owner,
                "assigned_to": [owner],
                "priority": priority,
                "labels": labels,
                "inputs": [serde_json::to_string_pretty(&payload)?],
                "outputs": ["HTTP 状态码", "响应头", "响应体", "执行耗时"],
                "acceptance_criteria": [
                    "Worker 能读取 HTTP 请求参数",
                    "请求完成后写回结构化结果",
                    "失败时写回错误原因"
                ]
            });
            insert_verify(&mut body, verify);
            handle_task_submit_response(
                &client,
                base,
                client
                    .post(format!("{base}/api/tasks"))
                    .json(&body)
                    .send()?
                    .text()?,
                wait,
                wait_timeout_seconds,
                &output,
            )?;
        }
        Commands::SubmitCommand {
            program,
            args,
            working_dir,
            timeout_seconds,
            node_id,
            workbench_id,
            os,
            created_by,
            owner,
            priority,
            title,
            wait,
            wait_timeout_seconds,
            output,
            verify_json,
            expect_exit_code,
            expect_stdout_contains,
            expect_stderr_contains,
        } => {
            let payload = serde_json::json!({
                "type": "command",
                "program": program,
                "args": args,
                "working_dir": working_dir,
                "timeout_seconds": timeout_seconds
            });
            let mut labels = vec!["compute".to_string(), "command".to_string()];
            push_placement_labels(&mut labels, node_id, workbench_id, os);
            let task_title = title.unwrap_or_else(|| {
                format!("命令任务 {}", payload["program"].as_str().unwrap_or(""))
            });
            let verify = build_command_verify(
                verify_json,
                expect_exit_code,
                expect_stdout_contains,
                expect_stderr_contains,
            )?;
            let mut body = serde_json::json!({
                "title": task_title,
                "summary": "提交一个主机命令任务，等待 Worker 执行并回传 stdout/stderr/退出码。",
                "created_by": created_by,
                "owner": owner,
                "assigned_to": [owner],
                "priority": priority,
                "labels": labels,
                "inputs": [serde_json::to_string_pretty(&payload)?],
                "outputs": ["退出码", "stdout", "stderr", "执行耗时"],
                "acceptance_criteria": [
                    "Hub 根据节点资源选择可执行主机",
                    "Worker 按安全策略允许名单执行命令",
                    "结果结构化写回 AgentGrid Hub"
                ]
            });
            insert_verify(&mut body, verify);
            handle_task_submit_response(
                &client,
                base,
                client
                    .post(format!("{base}/api/tasks"))
                    .json(&body)
                    .send()?
                    .text()?,
                wait,
                wait_timeout_seconds,
                &output,
            )?;
        }
        Commands::SubmitSoftwareInstall {
            node_id,
            os,
            name,
            source_url,
            installer,
            installer_args,
            sha256,
            timeout_seconds,
            created_by,
            owner,
            priority,
            title,
            wait,
            wait_timeout_seconds,
            output,
        } => {
            if node_id.trim().is_empty() {
                anyhow::bail!("--node is required for software install operations");
            }
            let payload = software_install_command_payload(
                &os,
                &name,
                &source_url,
                &installer,
                &installer_args,
                sha256.as_deref(),
                timeout_seconds,
            )?;
            let operation = serde_json::json!({
                "api_version": "agentgrid.operations/v1",
                "kind": "NodeOperation",
                "type": "software_install",
                "target_node_id": node_id,
                "os": os,
                "software": {
                    "name": name,
                    "source_url": source_url,
                    "installer": installer,
                    "installer_args": installer_args,
                    "sha256": sha256
                }
            });
            let labels = vec![
                "compute".to_string(),
                "command".to_string(),
                "operation:software_install".to_string(),
                format!("node:{node_id}"),
                format!("os:{os}"),
            ];
            let task_title = title.unwrap_or_else(|| format!("安装软件 {name} 到 {node_id}"));
            let mut body = serde_json::json!({
                "title": task_title,
                "summary": "提交一个标准化节点软件安装操作；当前执行层使用命令能力，业务层按 software_install 审计。",
                "created_by": created_by,
                "owner": owner,
                "assigned_to": [owner],
                "priority": priority,
                "labels": labels,
                "inputs": [
                    serde_json::to_string_pretty(&payload)?,
                    serde_json::to_string_pretty(&operation)?
                ],
                "outputs": ["退出码", "stdout", "stderr", "执行耗时"],
                "acceptance_criteria": [
                    "Hub 必须按 node_id 定点投递，不能被资源评分改派",
                    "Worker 下载指定安装包并执行静默安装参数",
                    "结果结构化写回，审计日志保留安装来源和目标节点"
                ]
            });
            body["operation"] = operation;
            handle_task_submit_response(
                &client,
                base,
                client
                    .post(format!("{base}/api/tasks"))
                    .json(&body)
                    .send()?
                    .text()?,
                wait,
                wait_timeout_seconds,
                &output,
            )?;
        }
        Commands::SubmitFile {
            operation,
            path,
            content,
            append,
            create_dirs,
            recursive,
            max_bytes,
            max_entries,
            node_id,
            workbench_id,
            os,
            created_by,
            owner,
            priority,
            title,
            wait,
            wait_timeout_seconds,
            output,
        } => {
            let payload = file_task_payload(
                &operation,
                path,
                content,
                append,
                create_dirs,
                recursive,
                max_bytes,
                max_entries,
            )?;
            let task_title = title.unwrap_or_else(|| {
                format!(
                    "文件任务 {} {}",
                    operation,
                    payload["path"].as_str().unwrap_or("")
                )
            });
            submit_compute_task(
                &client,
                base,
                "file",
                payload,
                task_title,
                "提交一个文件读写或目录列表任务。",
                created_by,
                owner,
                priority,
                node_id,
                workbench_id,
                os,
                vec!["文件内容或目录项", "执行耗时"],
                wait,
                wait_timeout_seconds,
                &output,
            )?;
        }
        Commands::SubmitGit {
            operation,
            repo,
            dest,
            repo_dir,
            branch,
            reference,
            depth,
            node_id,
            workbench_id,
            os,
            created_by,
            owner,
            priority,
            title,
            wait,
            wait_timeout_seconds,
            output,
        } => {
            let payload = match operation.as_str() {
                "clone" => serde_json::json!({
                    "type": "git",
                    "operation": "clone",
                    "repo": repo.ok_or_else(|| anyhow::anyhow!("--repo is required for clone"))?,
                    "dest": dest.ok_or_else(|| anyhow::anyhow!("--dest is required for clone"))?,
                    "branch": branch,
                    "depth": depth
                }),
                "pull" => serde_json::json!({
                    "type": "git",
                    "operation": "pull",
                    "repo_dir": repo_dir.ok_or_else(|| anyhow::anyhow!("--repo-dir is required for pull"))?
                }),
                "status" => serde_json::json!({
                    "type": "git",
                    "operation": "status",
                    "repo_dir": repo_dir.ok_or_else(|| anyhow::anyhow!("--repo-dir is required for status"))?
                }),
                "checkout" => serde_json::json!({
                    "type": "git",
                    "operation": "checkout",
                    "repo_dir": repo_dir.ok_or_else(|| anyhow::anyhow!("--repo-dir is required for checkout"))?,
                    "reference": reference.ok_or_else(|| anyhow::anyhow!("--reference is required for checkout"))?
                }),
                other => anyhow::bail!("unsupported git operation: {other}"),
            };
            let task_title = title.unwrap_or_else(|| format!("Git 任务 {operation}"));
            submit_compute_task(
                &client,
                base,
                "git",
                payload,
                task_title,
                "提交一个 Git 仓库操作任务。",
                created_by,
                owner,
                priority,
                node_id,
                workbench_id,
                os,
                vec!["退出码", "stdout", "stderr", "执行耗时"],
                wait,
                wait_timeout_seconds,
                &output,
            )?;
        }
        Commands::SubmitDocker {
            operation,
            image,
            args,
            timeout_seconds,
            node_id,
            workbench_id,
            os,
            created_by,
            owner,
            priority,
            title,
            wait,
            wait_timeout_seconds,
            output,
        } => {
            let payload = match operation.as_str() {
                "ps" | "images" => serde_json::json!({
                    "type": "docker",
                    "operation": operation
                }),
                "run" => serde_json::json!({
                    "type": "docker",
                    "operation": "run",
                    "image": image.ok_or_else(|| anyhow::anyhow!("--image is required for run"))?,
                    "args": args,
                    "timeout_seconds": timeout_seconds
                }),
                other => anyhow::bail!("unsupported docker operation: {other}"),
            };
            let task_title = title.unwrap_or_else(|| format!("Docker 任务 {operation}"));
            submit_compute_task(
                &client,
                base,
                "docker",
                payload,
                task_title,
                "提交一个 Docker/容器操作任务。",
                created_by,
                owner,
                priority,
                node_id,
                workbench_id,
                os,
                vec!["退出码", "stdout", "stderr", "执行耗时"],
                wait,
                wait_timeout_seconds,
                &output,
            )?;
        }
        Commands::SubmitBrowser {
            url,
            selector,
            timeout_seconds,
            max_response_bytes,
            node_id,
            workbench_id,
            os,
            created_by,
            owner,
            priority,
            title,
            wait,
            wait_timeout_seconds,
            output,
        } => {
            let payload = serde_json::json!({
                "type": "browser",
                "operation": "fetch",
                "url": url,
                "selector": selector,
                "timeout_seconds": timeout_seconds,
                "max_response_bytes": max_response_bytes
            });
            let task_title = title
                .unwrap_or_else(|| format!("浏览器抓取 {}", payload["url"].as_str().unwrap_or("")));
            submit_compute_task(
                &client,
                base,
                "browser",
                payload,
                task_title,
                "提交一个浏览器抓取任务。",
                created_by,
                owner,
                priority,
                node_id,
                workbench_id,
                os,
                vec!["页面状态码", "标题", "文本内容", "执行耗时"],
                wait,
                wait_timeout_seconds,
                &output,
            )?;
        }
        Commands::SubmitDesktopScreenshot {
            node_id,
            workbench_id,
            path,
            timeout_seconds,
            created_by,
            owner,
            priority,
            title,
            wait,
            wait_timeout_seconds,
            output,
        } => {
            let payload = serde_json::json!({
                "type": "desktop",
                "operation": "screenshot",
                "path": path,
                "timeout_seconds": timeout_seconds
            });
            let task_title = title.unwrap_or_else(|| "截取 Windows 交互桌面".to_string());
            submit_compute_task(
                &client,
                base,
                "desktop",
                payload,
                task_title,
                "提交一个交互桌面截图任务；Windows 需要目标节点运行 Desktop Helper。",
                created_by,
                owner,
                priority,
                node_id,
                workbench_id,
                Some("windows".to_string()),
                vec!["截图文件路径", "屏幕尺寸", "执行耗时"],
                wait,
                wait_timeout_seconds,
                &output,
            )?;
        }
        Commands::SubmitDesktopClick {
            node_id,
            workbench_id,
            x,
            y,
            button,
            timeout_seconds,
            created_by,
            owner,
            priority,
            title,
            wait,
            wait_timeout_seconds,
            output,
        } => {
            let payload = serde_json::json!({
                "type": "desktop",
                "operation": "click",
                "x": x,
                "y": y,
                "button": button,
                "timeout_seconds": timeout_seconds
            });
            let task_title = title.unwrap_or_else(|| format!("桌面点击 ({x}, {y})"));
            submit_compute_task(
                &client,
                base,
                "desktop",
                payload,
                task_title,
                "提交一个交互桌面点击任务；必须投递到 Windows Desktop Helper 节点。",
                created_by,
                owner,
                priority,
                node_id,
                workbench_id,
                Some("windows".to_string()),
                vec!["点击坐标", "按钮", "执行耗时"],
                wait,
                wait_timeout_seconds,
                &output,
            )?;
        }
        Commands::SubmitDesktopTypeText {
            node_id,
            workbench_id,
            text,
            interval_ms,
            timeout_seconds,
            created_by,
            owner,
            priority,
            title,
            wait,
            wait_timeout_seconds,
            output,
        } => {
            let payload = serde_json::json!({
                "type": "desktop",
                "operation": "type_text",
                "text": text,
                "interval_ms": interval_ms,
                "timeout_seconds": timeout_seconds
            });
            let task_title = title.unwrap_or_else(|| "桌面输入文本".to_string());
            submit_compute_task(
                &client,
                base,
                "desktop",
                payload,
                task_title,
                "提交一个交互桌面文本输入任务；必须投递到 Windows Desktop Helper 节点。",
                created_by,
                owner,
                priority,
                node_id,
                workbench_id,
                Some("windows".to_string()),
                vec!["输入字符数", "执行耗时"],
                wait,
                wait_timeout_seconds,
                &output,
            )?;
        }
        Commands::SubmitDesktopKey {
            node_id,
            workbench_id,
            key,
            modifiers,
            timeout_seconds,
            created_by,
            owner,
            priority,
            title,
            wait,
            wait_timeout_seconds,
            output,
        } => {
            let payload = serde_json::json!({
                "type": "desktop",
                "operation": "key",
                "key": key,
                "modifiers": modifiers,
                "timeout_seconds": timeout_seconds
            });
            let task_title = title.unwrap_or_else(|| "桌面按键".to_string());
            submit_compute_task(
                &client,
                base,
                "desktop",
                payload,
                task_title,
                "提交一个交互桌面按键任务；必须投递到 Windows Desktop Helper 节点。",
                created_by,
                owner,
                priority,
                node_id,
                workbench_id,
                Some("windows".to_string()),
                vec!["按键", "修饰键", "执行耗时"],
                wait,
                wait_timeout_seconds,
                &output,
            )?;
        }
        Commands::SubmitAgentMessage {
            from,
            to,
            message_type,
            subject,
            summary,
            payload,
            node_id,
            workbench_id,
            os,
            created_by,
            owner,
            priority,
            title,
            wait,
            wait_timeout_seconds,
            output,
        } => {
            let payload = serde_json::json!({
                "type": "agent_message",
                "from": from,
                "to": to,
                "message_type": message_type,
                "subject": subject,
                "summary": summary,
                "payload": parse_body(payload)?.unwrap_or_else(|| serde_json::json!({}))
            });
            let task_title = title.unwrap_or_else(|| {
                format!("AgentMessage {}", payload["subject"].as_str().unwrap_or(""))
            });
            submit_compute_task(
                &client,
                base,
                "agentmessage",
                payload,
                task_title,
                "提交一个 AI 协作消息投递任务。",
                created_by,
                owner,
                priority,
                node_id,
                workbench_id,
                os,
                vec!["投递状态", "消息编号", "执行耗时"],
                wait,
                wait_timeout_seconds,
                &output,
            )?;
        }
        Commands::SubmitPlugin {
            plugin_id,
            action,
            input,
            input_file,
            input_stdin,
            input_base64,
            timeout_seconds,
            node_id,
            workbench_id,
            os,
            created_by,
            owner,
            priority,
            title,
            wait,
            wait_timeout_seconds,
            output,
        } => {
            let input = read_json_payload(
                "--input",
                input,
                input_file,
                input_stdin,
                input_base64,
                Some(serde_json::json!({})),
            )?;
            let payload = serde_json::json!({
                "type": "plugin",
                "plugin_id": plugin_id,
                "action": action,
                "input": input,
                "timeout_seconds": timeout_seconds
            });
            let task_title = title.unwrap_or_else(|| {
                format!(
                    "插件任务 {}:{}",
                    payload["plugin_id"].as_str().unwrap_or("plugin"),
                    payload["action"].as_str().unwrap_or("run")
                )
            });
            submit_compute_task(
                &client,
                base,
                "plugin",
                payload,
                task_title,
                "提交一个 Worker 插件任务。",
                created_by,
                owner,
                priority,
                node_id,
                workbench_id,
                os,
                vec!["插件输出", "执行耗时"],
                wait,
                wait_timeout_seconds,
                &output,
            )?;
        }
        Commands::Send {
            from,
            to,
            message_type,
            subject,
            summary,
        } => {
            let body = serde_json::json!({
                "from": from,
                "to": to,
                "type": message_type,
                "subject": subject,
                "summary": summary,
                "priority": "normal",
                "requires_ack": false,
                "payload": {}
            });
            print_json(
                client
                    .post(format!("{base}/api/messages"))
                    .json(&body)
                    .send()?
                    .text()?,
            );
        }
    }

    Ok(())
}

fn submit_compute_task(
    client: &reqwest::blocking::Client,
    base: &str,
    task_label: &str,
    payload: serde_json::Value,
    title: String,
    summary: &str,
    created_by: String,
    owner: String,
    priority: String,
    node_id: Option<String>,
    workbench_id: Option<String>,
    os: Option<String>,
    outputs: Vec<&str>,
    wait: bool,
    wait_timeout_seconds: u64,
    output: &str,
) -> Result<()> {
    let mut labels = vec!["compute".to_string(), task_label.to_string()];
    push_placement_labels(&mut labels, node_id, workbench_id, os);
    let body = serde_json::json!({
        "title": title,
        "summary": summary,
        "created_by": created_by,
        "owner": owner,
        "assigned_to": [owner],
        "priority": priority,
        "labels": labels,
        "inputs": [serde_json::to_string_pretty(&payload)?],
        "outputs": outputs,
        "acceptance_criteria": [
            "Hub 根据资源、能力和路由标签选择节点",
            "Worker 执行任务并写回结构化结果",
            "审计日志记录调度原因和执行时间线"
        ]
    });
    handle_task_submit_response(
        client,
        base,
        client
            .post(format!("{base}/api/tasks"))
            .json(&body)
            .send()?
            .text()?,
        wait,
        wait_timeout_seconds,
        output,
    )?;
    Ok(())
}

fn submit_workbench_action(
    client: &reqwest::blocking::Client,
    base: &str,
    workbench_id: String,
    action: String,
    payload: Value,
    title: String,
    wait: bool,
    wait_timeout_seconds: u64,
    output: &str,
) -> Result<()> {
    let body = serde_json::json!({
        "action": action,
        "payload": payload,
        "title": title,
        "created_by": "workbench-cli"
    });
    let raw = client
        .post(format!("{base}/api/workbenches/{workbench_id}/actions"))
        .json(&body)
        .send()?
        .text()?;
    handle_workbench_action_response(client, base, raw, wait, wait_timeout_seconds, output)
}

fn handle_workbench_action_response(
    client: &reqwest::blocking::Client,
    base: &str,
    raw: String,
    wait: bool,
    wait_timeout_seconds: u64,
    output: &str,
) -> Result<()> {
    let value: Value = serde_json::from_str(&raw)?;
    if value.get("ok").and_then(Value::as_bool) == Some(false) {
        print_json(raw);
        return Ok(());
    }
    if output == "json" && !wait {
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }
    if let Some(task_id) = value.pointer("/item/task_id").and_then(Value::as_str) {
        let message_id = value.pointer("/item/message_id").and_then(Value::as_str);
        if !wait {
            println!("Task ID: {task_id}");
            if let Some(message_id) = message_id {
                println!("Message ID: {message_id}");
            }
            if let Some(node_id) = value
                .pointer("/item/selected_channel/node_id")
                .and_then(Value::as_str)
            {
                println!("Selected Node: {node_id}");
            }
            if let Some(reason) = value
                .pointer("/item/routing_reason")
                .and_then(Value::as_str)
            {
                println!("Routing: {reason}");
            }
            println!();
            println!("查看结果: agentgrid tasks get --id {task_id}");
            println!("等待完成: agentgrid tasks watch --id {task_id}");
            return Ok(());
        }
        let snapshot = wait_for_task(client, base, task_id, wait_timeout_seconds)?;
        print_task_summary(&snapshot, output);
        return Ok(());
    }
    if value.pointer("/item/port_bridge").is_some() {
        print_port_bridge_create(value.get("item").unwrap_or(&value), output)?;
        return Ok(());
    }
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn file_task_payload(
    operation: &str,
    path: String,
    content: Option<String>,
    append: bool,
    create_dirs: bool,
    recursive: bool,
    max_bytes: Option<u64>,
    max_entries: Option<u64>,
) -> Result<Value> {
    match operation {
        "read" => Ok(serde_json::json!({
            "type": "file",
            "operation": "read",
            "path": path,
            "max_bytes": max_bytes
        })),
        "write" => Ok(serde_json::json!({
            "type": "file",
            "operation": "write",
            "path": path,
            "content": content.unwrap_or_default(),
            "append": append,
            "create_dirs": create_dirs
        })),
        "list" => Ok(serde_json::json!({
            "type": "file",
            "operation": "list",
            "path": path,
            "recursive": recursive,
            "max_entries": max_entries
        })),
        other => anyhow::bail!("unsupported file operation: {other}"),
    }
}

fn push_placement_labels(
    labels: &mut Vec<String>,
    node_id: Option<String>,
    workbench_id: Option<String>,
    os: Option<String>,
) {
    if let Some(node_id) = node_id.filter(|value| !value.trim().is_empty()) {
        labels.push(format!("node:{node_id}"));
    }
    if let Some(workbench_id) = workbench_id.filter(|value| !value.trim().is_empty()) {
        labels.push(format!("workbench:{workbench_id}"));
    }
    if let Some(os) = os.filter(|value| !value.trim().is_empty()) {
        labels.push(format!("os:{os}"));
    }
}

fn parse_headers(values: Vec<String>) -> Result<Vec<(String, String)>> {
    values
        .into_iter()
        .map(|value| {
            let Some((name, header_value)) = value.split_once(':') else {
                anyhow::bail!("header must use 'Name: Value' format");
            };
            Ok((name.trim().to_string(), header_value.trim().to_string()))
        })
        .collect()
}

fn parse_body(value: Option<String>) -> Result<Option<Value>> {
    value
        .map(|raw| serde_json::from_str(&raw).map_err(Into::into))
        .transpose()
}

fn read_json_payload(
    flag_name: &str,
    inline: Option<String>,
    file: Option<String>,
    stdin_enabled: bool,
    base64_value: Option<String>,
    default: Option<Value>,
) -> Result<Value> {
    let source_count = inline.is_some() as u8
        + file.is_some() as u8
        + stdin_enabled as u8
        + base64_value.is_some() as u8;
    if source_count == 0 {
        return default.ok_or_else(|| anyhow::anyhow!("{flag_name} is required"));
    }
    if source_count > 1 {
        anyhow::bail!(
            "use only one of {flag_name}, {flag_name}-file, {flag_name}-stdin, or {flag_name}-base64"
        );
    }

    let raw = if let Some(raw) = inline {
        raw
    } else if let Some(path) = file {
        fs::read_to_string(&path)
            .with_context(|| format!("failed to read {flag_name}-file: {path}"))?
    } else if stdin_enabled {
        let mut raw = String::new();
        io::stdin()
            .read_to_string(&mut raw)
            .with_context(|| format!("failed to read {flag_name}-stdin"))?;
        raw
    } else if let Some(encoded) = base64_value {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .with_context(|| format!("{flag_name}-base64 must be valid base64"))?;
        String::from_utf8(bytes)
            .with_context(|| format!("{flag_name}-base64 must decode to UTF-8 JSON"))?
    } else {
        unreachable!();
    };

    serde_json::from_str(&raw).with_context(|| format!("{flag_name} must be valid JSON"))
}

fn parse_verify_json(value: Option<String>) -> Result<Option<Value>> {
    value
        .map(|raw| serde_json::from_str(&raw).context("--verify-json must be valid JSON"))
        .transpose()
}

fn parse_parameters(params: Vec<String>, parameters_json: Option<String>) -> Result<Value> {
    let mut value = if let Some(raw) = parameters_json {
        serde_json::from_str(&raw).context("--parameters-json must be valid JSON")?
    } else {
        serde_json::json!({})
    };
    let map = value
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("parameters must be a JSON object"))?;
    for item in params {
        let Some((key, raw_value)) = item.split_once('=') else {
            anyhow::bail!("--param must use key=value format");
        };
        let parsed = serde_json::from_str(raw_value)
            .unwrap_or_else(|_| Value::String(raw_value.to_string()));
        map.insert(key.to_string(), parsed);
    }
    Ok(value)
}

fn job_partition_payload(
    partition_items: Option<String>,
    partition_range: Option<String>,
) -> Result<Value> {
    match (partition_items, partition_range) {
        (Some(_), Some(_)) => {
            anyhow::bail!("use only one of --partition-items or --partition-range")
        }
        (Some(raw), None) => {
            let items: Value =
                serde_json::from_str(&raw).context("--partition-items must be a JSON array")?;
            if !items.is_array() {
                anyhow::bail!("--partition-items must be a JSON array");
            }
            Ok(serde_json::json!({
                "type": "items",
                "items": items
            }))
        }
        (None, Some(raw)) => {
            let mut start = None;
            let mut end = None;
            let mut step = 1i64;
            for part in raw.split(',') {
                let Some((key, value)) = part.split_once('=') else {
                    anyhow::bail!("--partition-range must use start=0,end=100[,step=1]");
                };
                let parsed = value
                    .trim()
                    .parse::<i64>()
                    .with_context(|| format!("invalid range value for {}", key.trim()))?;
                match key.trim() {
                    "start" => start = Some(parsed),
                    "end" => end = Some(parsed),
                    "step" => step = parsed.max(1),
                    other => anyhow::bail!("unknown range key: {other}"),
                }
            }
            Ok(serde_json::json!({
                "type": "range",
                "start": start.context("--partition-range requires start")?,
                "end": end.context("--partition-range requires end")?,
                "step": step
            }))
        }
        (None, None) => Ok(serde_json::json!({ "type": "none" })),
    }
}

#[allow(clippy::too_many_arguments)]
fn job_request_json(
    tool: String,
    title: String,
    payload: Value,
    node: Option<String>,
    workbench: Option<String>,
    os: Option<String>,
    max_attempts: i64,
    shards: i64,
    max_parallelism: Option<i64>,
    reduce: String,
    partition: Value,
    idempotency_key: Option<String>,
) -> Value {
    let has_idempotency_key = idempotency_key.is_some();
    serde_json::json!({
        "tool_id": tool,
        "title": title,
        "payload": payload,
        "placement": {
            "node_id": node,
            "workbench_id": workbench,
            "os": os
        },
        "strategy": if shards > 1 {
            serde_json::json!({
                "type": "sharded",
                "shard_count": shards,
                "max_parallelism": max_parallelism.unwrap_or(shards),
                "payload_mode": "inject_shard"
            })
        } else {
            serde_json::json!({ "type": "single" })
        },
        "partition": partition,
        "reduce": {
            "type": reduce
        },
        "retry_policy": {
            "max_attempts": max_attempts,
            "on_node_lost": "reschedule",
            "on_process_failed": "reschedule_if_idempotent"
        },
        "checkpoint_policy": {
            "enabled": true,
            "mode": "worker_reported"
        },
        "idempotency": {
            "key": idempotency_key,
            "mode": if has_idempotency_key { "idempotent" } else { "at_least_once" }
        },
        "created_by": "agentgrid-cli"
    })
}

fn remove_null_fields(value: &mut Value) {
    if let Some(map) = value.as_object_mut() {
        map.retain(|_, item| !item.is_null());
    }
}

#[allow(clippy::too_many_arguments)]
fn port_bridge_create_json(
    source_node: String,
    target_node: String,
    target_port: u16,
    source_port: u16,
    target_host: String,
    bind_host: String,
    ttl_seconds: u64,
    protocol: String,
    purpose: Option<String>,
    created_by: String,
) -> Value {
    let mut body = serde_json::json!({
        "source_node_id": source_node,
        "target_node_id": target_node,
        "source_bind_host": bind_host,
        "source_bind_port": source_port,
        "target_host": target_host,
        "target_port": target_port,
        "protocol": protocol,
        "ttl_seconds": ttl_seconds,
        "purpose": purpose,
        "created_by": created_by
    });
    remove_null_fields(&mut body);
    body
}

fn print_port_bridge_create(value: &Value, output: &str) -> Result<()> {
    if output == "json" {
        println!("{}", serde_json::to_string_pretty(value)?);
        return Ok(());
    }
    let item = value.get("item").unwrap_or(value);
    let id = item
        .pointer("/metadata/id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let state = item
        .pointer("/status/state")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let source_node = item
        .pointer("/spec/source_node_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let target_node = item
        .pointer("/spec/target_node_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let target_host = item
        .pointer("/spec/target_host")
        .and_then(Value::as_str)
        .unwrap_or("");
    let target_port = item
        .pointer("/spec/target_port")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let source_url = item
        .pointer("/status/source_url")
        .and_then(Value::as_str)
        .unwrap_or("");
    println!("Port Bridge ID: {id}");
    println!("State: {state}");
    println!("Source Node: {source_node}");
    println!("Target: {target_node} -> {target_host}:{target_port}");
    if !source_url.is_empty() {
        println!("Source URL: {source_url}");
    }
    println!();
    println!("查看详情: agentgrid port-bridges get --id {id}");
    println!("关闭桥接: agentgrid port-bridges close --id {id}");
    Ok(())
}

fn build_command_verify(
    verify_json: Option<String>,
    expect_exit_code: Option<i64>,
    expect_stdout_contains: Option<String>,
    expect_stderr_contains: Option<String>,
) -> Result<Option<Value>> {
    if verify_json.is_some() {
        return parse_verify_json(verify_json);
    }
    let mut rules = Vec::new();
    if let Some(exit_code) = expect_exit_code {
        rules.push(serde_json::json!({
            "path": "result.exit_code",
            "op": "eq",
            "value": exit_code,
            "description": "命令退出码符合预期"
        }));
    }
    if let Some(text) = expect_stdout_contains {
        rules.push(serde_json::json!({
            "path": "result.stdout",
            "op": "contains",
            "value": text,
            "description": "stdout 包含预期文本"
        }));
    }
    if let Some(text) = expect_stderr_contains {
        rules.push(serde_json::json!({
            "path": "result.stderr",
            "op": "contains",
            "value": text,
            "description": "stderr 包含预期文本"
        }));
    }
    if rules.is_empty() {
        Ok(None)
    } else {
        Ok(Some(serde_json::json!({ "rules": rules })))
    }
}

fn software_install_command_payload(
    os: &str,
    name: &str,
    source_url: &str,
    installer: &str,
    installer_args: &[String],
    sha256: Option<&str>,
    timeout_seconds: u64,
) -> Result<Value> {
    if !matches!(os.to_ascii_lowercase().as_str(), "windows" | "win") {
        anyhow::bail!("submit-software-install v1 currently supports windows only");
    }
    if source_url.trim().is_empty() {
        anyhow::bail!("--source-url is required");
    }
    let extension = match installer.to_ascii_lowercase().as_str() {
        "msi" => "msi",
        "exe" => "exe",
        other => anyhow::bail!("unsupported installer type: {other}; use exe or msi"),
    };
    let target = format!(
        "$env:TEMP\\agentgrid-install-{}.{extension}",
        sanitize_filename(name)
    );
    let mut lines = vec![
        "$ErrorActionPreference = 'Stop'".to_string(),
        format!("$Installer = \"{target}\""),
        format!(
            "Invoke-WebRequest -Uri \"{}\" -OutFile $Installer",
            escape_powershell_double_quoted(source_url)
        ),
    ];
    if let Some(expected) = sha256.filter(|value| !value.trim().is_empty()) {
        lines.push(format!(
            "if ((Get-FileHash -Algorithm SHA256 $Installer).Hash.ToLower() -ne \"{}\") {{ throw \"sha256 mismatch for {name}\" }}",
            escape_powershell_double_quoted(&expected.to_ascii_lowercase())
        ));
    }
    let args_json = serde_json::to_string(installer_args)?;
    lines.push(format!("$Args = ConvertFrom-Json @'\n{args_json}\n'@"));
    match extension {
        "msi" => lines.push(
            "$Process = Start-Process msiexec.exe -ArgumentList (@('/i', $Installer, '/qn', '/norestart') + $Args) -Wait -PassThru"
                .to_string(),
        ),
        "exe" => lines.push(
            "$Process = Start-Process $Installer -ArgumentList $Args -Wait -PassThru".to_string(),
        ),
        _ => unreachable!(),
    }
    lines.push("Write-Output \"agentgrid.software_install.completed\"".to_string());
    lines.push("Write-Output \"name=".to_string() + &escape_powershell_double_quoted(name) + "\"");
    lines.push("Write-Output \"exit_code=$($Process.ExitCode)\"".to_string());
    lines.push("exit $Process.ExitCode".to_string());
    Ok(serde_json::json!({
        "type": "command",
        "program": "powershell",
        "args": ["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", lines.join("; ")],
        "working_dir": null,
        "timeout_seconds": timeout_seconds
    }))
}

fn sanitize_filename(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    sanitized.trim_matches('-').chars().take(48).collect()
}

fn escape_powershell_double_quoted(value: &str) -> String {
    value.replace('`', "``").replace('"', "`\"")
}

fn build_http_verify(
    verify_json: Option<String>,
    expect_status_2xx: bool,
    expect_status: Option<u16>,
    expect_body_contains: Option<String>,
) -> Result<Option<Value>> {
    if verify_json.is_some() {
        return parse_verify_json(verify_json);
    }
    let mut presets = Vec::new();
    let mut rules = Vec::new();
    if expect_status_2xx {
        presets.push("http.status_2xx");
    }
    if let Some(status) = expect_status {
        rules.push(serde_json::json!({
            "path": "result.status_code",
            "op": "eq",
            "value": status,
            "description": "HTTP 状态码符合预期"
        }));
    }
    if let Some(text) = expect_body_contains {
        rules.push(serde_json::json!({
            "path": "result.body",
            "op": "contains",
            "value": text,
            "description": "响应体包含预期文本"
        }));
    }
    if presets.is_empty() && rules.is_empty() {
        Ok(None)
    } else {
        Ok(Some(
            serde_json::json!({ "presets": presets, "rules": rules }),
        ))
    }
}

fn insert_verify(body: &mut Value, verify: Option<Value>) {
    if let Some(verify) = verify {
        if let Some(map) = body.as_object_mut() {
            map.insert("verify".to_string(), verify);
        }
    }
}

fn handle_task_submit_response(
    client: &reqwest::blocking::Client,
    base: &str,
    raw: String,
    wait: bool,
    wait_timeout_seconds: u64,
    output: &str,
) -> Result<()> {
    let value: Value = serde_json::from_str(&raw)?;
    if value.get("ok").and_then(Value::as_bool) == Some(false) {
        print_json(raw);
        return Ok(());
    }
    let task_id = value
        .pointer("/item/metadata/id")
        .and_then(Value::as_str)
        .context("task id missing in response")?;
    let message_id = value.get("message_id").and_then(Value::as_str);
    if !wait && output == "json" {
        print_json(raw);
        return Ok(());
    }
    println!("Task ID: {task_id}");
    if let Some(message_id) = message_id {
        println!("Message ID: {message_id}");
    }
    if !wait {
        println!("State: assigned");
        println!();
        println!("查看结果: agentgrid tasks get --id {task_id}");
        println!("等待完成: agentgrid tasks watch --id {task_id}");
        return Ok(());
    }
    let snapshot = wait_for_task(client, base, task_id, wait_timeout_seconds)?;
    print_task_summary(&snapshot, output);
    Ok(())
}

fn fetch_json(client: &reqwest::blocking::Client, url: &str) -> Result<Value> {
    let response = client.get(url).send()?.error_for_status()?;
    Ok(response.json::<Value>()?)
}

fn post_json(client: &reqwest::blocking::Client, url: &str, body: &Value) -> Result<Value> {
    let response = client.post(url).json(body).send()?.error_for_status()?;
    Ok(response.json::<Value>()?)
}

fn wait_for_task(
    client: &reqwest::blocking::Client,
    base: &str,
    task_id: &str,
    timeout_seconds: u64,
) -> Result<Value> {
    let started = std::time::Instant::now();
    loop {
        let snapshot = fetch_json(client, &format!("{base}/api/tasks/{task_id}/snapshot"))?;
        let state = snapshot
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        if matches!(
            state,
            "done" | "failed" | "cancelled" | "stopped" | "blocked" | "review"
        ) {
            return Ok(snapshot);
        }
        if started.elapsed().as_secs() >= timeout_seconds {
            anyhow::bail!("task {task_id} did not finish within {timeout_seconds}s");
        }
        thread::sleep(Duration::from_secs(2));
    }
}

fn wait_for_workflow(
    client: &reqwest::blocking::Client,
    base: &str,
    workflow_id: &str,
    timeout_seconds: u64,
) -> Result<Value> {
    let started = std::time::Instant::now();
    let mut last_state = String::new();
    let mut last_progress = -1_i64;
    loop {
        let response = fetch_json(client, &format!("{base}/api/workflows/{workflow_id}"))?;
        let workflow = response
            .get("item")
            .cloned()
            .unwrap_or_else(|| response.clone());
        let state = workflow
            .pointer("/status/state")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let progress = workflow
            .pointer("/status/progress")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        if state != last_state || progress != last_progress {
            eprintln!("workflow {workflow_id}: {state} {progress}%");
            last_state = state.to_string();
            last_progress = progress;
        }
        if matches!(state, "done" | "failed" | "cancelled") {
            return Ok(workflow);
        }
        if started.elapsed().as_secs() >= timeout_seconds {
            anyhow::bail!("workflow {workflow_id} did not finish within {timeout_seconds}s");
        }
        thread::sleep(Duration::from_secs(2));
    }
}

fn wait_for_job(
    client: &reqwest::blocking::Client,
    base: &str,
    job_id: &str,
    timeout_seconds: u64,
) -> Result<Value> {
    let started = std::time::Instant::now();
    let mut last_state = String::new();
    loop {
        let response = fetch_json(client, &format!("{base}/api/jobs/{job_id}"))?;
        let job = response
            .get("item")
            .cloned()
            .unwrap_or_else(|| response.clone());
        let state = job
            .pointer("/status/state")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        if state != last_state {
            eprintln!("job {job_id}: {state}");
            last_state = state.to_string();
        }
        if matches!(state, "done" | "failed" | "cancelled") {
            return Ok(job);
        }
        if started.elapsed().as_secs() >= timeout_seconds {
            anyhow::bail!("job {job_id} did not finish within {timeout_seconds}s");
        }
        thread::sleep(Duration::from_secs(2));
    }
}

fn watch_task(
    client: &reqwest::blocking::Client,
    base: &str,
    task_id: &str,
    timeout_seconds: u64,
) -> Result<Value> {
    let started = std::time::Instant::now();
    let mut printed_log_count = 0usize;
    loop {
        let snapshot = fetch_json(client, &format!("{base}/api/tasks/{task_id}/snapshot"))?;
        let logs = snapshot
            .get("logs")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for log in logs.iter().skip(printed_log_count) {
            let stream = log
                .pointer("/spec/stream")
                .and_then(Value::as_str)
                .unwrap_or("stdout");
            let line = log
                .pointer("/spec/line")
                .and_then(Value::as_str)
                .unwrap_or("");
            if stream == "stderr" {
                eprint!("{line}");
            } else {
                print!("{line}");
            }
        }
        printed_log_count = logs.len();
        let state = snapshot
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        if matches!(
            state,
            "done" | "failed" | "cancelled" | "stopped" | "blocked" | "review"
        ) {
            return Ok(snapshot);
        }
        if started.elapsed().as_secs() >= timeout_seconds {
            anyhow::bail!("task {task_id} did not finish within {timeout_seconds}s");
        }
        thread::sleep(Duration::from_secs(2));
    }
}

fn workflow_state_from_response(value: &Value) -> &str {
    value
        .pointer("/item/status/state")
        .or_else(|| value.pointer("/status/state"))
        .and_then(Value::as_str)
        .unwrap_or("unknown")
}

fn print_workflow_summary(workflow: &Value, output: &str) {
    if output == "json" {
        println!("{}", serde_json::to_string_pretty(workflow).unwrap());
        return;
    }
    let workflow_id = workflow
        .pointer("/metadata/id")
        .and_then(Value::as_str)
        .unwrap_or("-");
    let name = workflow
        .pointer("/spec/name")
        .and_then(Value::as_str)
        .unwrap_or("-");
    let state = workflow
        .pointer("/status/state")
        .and_then(Value::as_str)
        .unwrap_or("-");
    let progress = workflow
        .pointer("/status/progress")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    println!("Workflow: {workflow_id}");
    println!("Name: {name}");
    println!("State: {state}");
    println!("Progress: {progress}%");
    println!();

    let runs = workflow
        .pointer("/spec/runs")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let nodes = workflow
        .pointer("/spec/nodes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for node in nodes {
        let node_id = node.get("id").and_then(Value::as_str).unwrap_or("-");
        let title = node.get("title").and_then(Value::as_str).unwrap_or("-");
        let run = runs.iter().find(|run| {
            run.pointer("/metadata/workflow_node_id")
                .and_then(Value::as_str)
                == Some(node_id)
        });
        let run_state = run
            .and_then(|run| run.pointer("/status/state").and_then(Value::as_str))
            .unwrap_or("pending");
        let task_id = run
            .and_then(|run| run.pointer("/metadata/task_id").and_then(Value::as_str))
            .unwrap_or("-");
        let rendered_title = run
            .and_then(|run| run.pointer("/spec/task/title").and_then(Value::as_str))
            .unwrap_or(title);
        println!("- [{run_state}] {node_id} / {title} / task: {task_id}");
        if rendered_title != title {
            println!("  rendered: {rendered_title}");
        }
    }

    if let Some(result) = workflow
        .pointer("/status/result")
        .filter(|value| !value.is_null())
    {
        println!();
        println!("Result:");
        println!("{}", serde_json::to_string_pretty(result).unwrap());
    }
    if let Some(error) = workflow
        .pointer("/status/error")
        .filter(|value| !value.is_null())
    {
        println!();
        eprintln!("Error:");
        eprintln!("{}", serde_json::to_string_pretty(error).unwrap());
    }
}

fn print_task_summary(snapshot: &Value, output: &str) {
    if output == "json" {
        println!("{}", serde_json::to_string_pretty(snapshot).unwrap());
        return;
    }
    let task_id = snapshot
        .get("task_id")
        .and_then(Value::as_str)
        .unwrap_or("-");
    let state = snapshot.get("state").and_then(Value::as_str).unwrap_or("-");
    let node = snapshot
        .get("leased_by_node_id")
        .and_then(Value::as_str)
        .unwrap_or("-");
    let result = snapshot.get("result").unwrap_or(&Value::Null);
    let error = snapshot.get("error").unwrap_or(&Value::Null);
    println!("Task: {task_id}");
    println!("Node: {node}");
    println!("State: {state}");
    if let Some(exit_code) = result.get("exit_code").and_then(Value::as_i64) {
        println!("Exit: {exit_code}");
    }
    if let Some(duration_ms) = result.get("duration_ms").and_then(Value::as_u64) {
        println!("Duration: {duration_ms} ms");
    }
    if result.get("type").and_then(Value::as_str) == Some("file_result") {
        print_file_result_summary(result);
    }
    if let Some(verification) = result.get("verification") {
        let state = verification
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or("-");
        let summary = verification
            .get("summary")
            .and_then(Value::as_str)
            .unwrap_or("");
        println!("Verification: {state} {summary}");
    }
    println!();
    if let Some(stdout) = snapshot
        .get("stdout")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
    {
        println!("{stdout}");
    }
    if let Some(stderr) = snapshot
        .get("stderr")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
    {
        eprintln!("{stderr}");
    }
    if !error.is_null() {
        eprintln!("{}", serde_json::to_string_pretty(error).unwrap());
    }
}

fn print_file_result_summary(result: &Value) {
    let operation = result
        .get("operation")
        .and_then(Value::as_str)
        .unwrap_or("file");
    let path = result.get("path").and_then(Value::as_str).unwrap_or("-");
    println!("File: {operation} {path}");
    if let Some(content) = result
        .get("content")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
    {
        println!();
        println!("{content}");
    }
    let Some(entries) = result.get("entries").and_then(Value::as_array) else {
        return;
    };
    println!("Entries: {}", entries.len());
    for entry in entries.iter().take(200) {
        let is_dir = entry
            .get("is_dir")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let len = entry.get("len").and_then(Value::as_u64).unwrap_or(0);
        let path = entry.get("path").and_then(Value::as_str).unwrap_or("-");
        let kind = if is_dir { "dir " } else { "file" };
        println!("{kind}\t{len}\t{path}");
    }
    if entries.len() > 200 {
        println!("... {} more entries", entries.len() - 200);
    }
}

fn print_task_logs(snapshot: &Value) {
    let logs = snapshot
        .get("logs")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if logs.is_empty() {
        if let Some(stdout) = snapshot.get("stdout").and_then(Value::as_str) {
            print!("{stdout}");
        }
        if let Some(stderr) = snapshot.get("stderr").and_then(Value::as_str) {
            eprint!("{stderr}");
        }
        return;
    }
    for log in logs {
        let stream = log
            .pointer("/spec/stream")
            .and_then(Value::as_str)
            .unwrap_or("stdout");
        let line = log
            .pointer("/spec/line")
            .and_then(Value::as_str)
            .unwrap_or("");
        if stream == "stderr" {
            eprint!("{line}");
        } else {
            print!("{line}");
        }
    }
}

fn print_json(raw: String) {
    match serde_json::from_str::<serde_json::Value>(&raw) {
        Ok(value) => println!("{}", serde_json::to_string_pretty(&value).unwrap()),
        Err(_) => println!("{raw}"),
    }
}
