# AgentGrid Standards v1

AgentGrid is an AI-native workbench scheduling layer. It lets AI discover,
call, verify, and audit real machines, desktop benches, hardware benches,
devices, and local tools through structured contracts.

AgentGrid is not a generic remote execution platform, not an RDP replacement,
not a Jenkins/Ansible replacement, and not a natural-language automation layer.
Natural language belongs to the AI client. AgentGrid receives structured JSON.

## 1. AgentMessage Standard

AgentMessage is the AI-to-AI communication protocol.

Required envelope:

```json
{
  "api_version": "agentmessage.io/v1",
  "kind": "AgentMessage",
  "metadata": {
    "id": "msg_xxx",
    "project_id": "agentgrid",
    "from": "architect-agent",
    "to": ["worker-agent"],
    "created_at": "2026-05-31T00:00:00Z"
  },
  "spec": {
    "type": "task.assigned",
    "subject": "任务标题",
    "summary": "任务摘要",
    "priority": "normal",
    "requires_ack": false,
    "payload": {}
  }
}
```

Allowed message families:

- `task.*`: task assignment and progress.
- `review.*`: review workflow.
- `test.*`: QA workflow.
- `contract.*`: API/schema change notices.
- `decision.*`: architecture or product decisions.
- `agent.*`: agent status.
- `broadcast.*`: system notices.

## 2. Task Protocol Standard

AgentGrid has two task classes:

- Collaboration tasks: assigned to AI employees, not executable by Worker.
- Compute tasks: executable by Worker nodes.

Compute tasks must include a routable label and machine-readable payload:

```json
{
  "labels": ["compute", "http_request"],
  "inputs": [
    "{\"type\":\"http_request\",\"method\":\"GET\",\"url\":\"https://example.com\",\"headers\":[],\"body\":null}"
  ]
}
```

Only tasks with an executable label may be leased by Worker:

- `http_request`
- `command`
- `file`
- `git`
- `docker`
- `browser`
- `session`
- `desktop`
- `agentmessage`
- `plugin`

## 3. Node Capability Discovery

Each Worker periodically reports a `Node` object to Hub.

Required fields:

```json
{
  "id": "local-mac",
  "name": "本机 Mac 节点",
  "os": "Darwin",
  "arch": "aarch64",
  "address": "host.local",
  "tags": ["local", "macos"],
  "capabilities": ["http", "command", "agentmessage"],
  "cpu_cores": 14,
  "memory_mb": 24576,
  "cpu_usage_percent": 20,
  "memory_used_mb": 18000,
  "disk_total_mb": 1898048,
  "disk_free_mb": 940000,
  "running_jobs": 2,
  "max_concurrent_jobs": 4,
  "status": "online"
}
```

Capability names are stable routing keys. Workers must not advertise a capability unless they can enforce its safety policy.

## 4. Resource-Aware Scheduling

Hub owns scheduling decisions. Worker owns outbound connection and execution.

Scheduling inputs:

- Required capability.
- Required tags or OS.
- CPU utilization.
- Memory utilization.
- Disk utilization.
- Current running jobs.
- Maximum concurrent jobs.
- Lease expiration.

Score formula v1:

```text
score = cpu_usage * 0.45
      + memory_usage * 0.30
      + disk_usage * 0.15
      + running_jobs * 0.8
```

Lower score is better.

Workers ask for a lease:

```json
{
  "node_id": "chenqi-center",
  "max_tasks": 8,
  "lease_seconds": 120,
  "capabilities": ["http", "command"]
}
```

Hub returns only tasks that match the worker capability and are not leased.

## 5. Safe Execution Sandbox

Worker must enforce local safety before executing a task.

Policy decision values:

- `allow`: execute.
- `deny`: reject and report error.
- `ask_user`: reserved for interactive approval.

HTTP v1 policy:

- Allow only `http` and `https`.
- Deny localhost/private network targets by default unless enabled.
- Limit response size.
- Enforce timeout.
- Preserve structured error.

Command v1 policy:

- Disabled by default.
- Future support must require an allowlist.

## 6. Structured Result Writeback

Worker completion must write structured JSON.

HTTP result:

```json
{
  "type": "http_response",
  "status_code": 200,
  "headers": [["content-type", "application/json"]],
  "body": {},
  "duration_ms": 120
}
```

Failure result:

```json
{
  "code": "policy_denied",
  "message": "localhost targets are disabled",
  "retryable": false
}
```

## 7. Multi-Agent Audit

All important events must be audit-recorded:

- `node.heartbeat`
- `task.created`
- `task.leased`
- `task.completed`
- `task.failed`
- `policy.denied`
- `message.created`
- `agent.changed`

Audit records are append-only.

## 8. Hybrid Local/Cloud Scheduling

AgentGrid assumes mixed nodes:

- Cloud Linux servers.
- Local macOS/Windows machines.
- Future private LAN nodes.

Therefore the default network model is:

```text
Worker -> Hub outbound connection
Hub never needs inbound access to Worker
```

This keeps NAT, home networks, laptops, and private servers easy to join.

## 9. Agent Identity And Node Scope

AgentGrid should not rely on natural language to decide who may operate a
machine. AI clients may understand natural language, but AgentGrid must receive
structured identity and scope data.

An AI employee is a business identity:

```json
{
  "api_version": "agentmessage.io/v1",
  "kind": "Agent",
  "metadata": {
    "id": "ops-agent",
    "project_id": "agentgrid",
    "name": "运维员工"
  },
  "spec": {
    "role": "Ops 运维工程师",
    "skills": ["节点运维", "软件安装", "服务重启"],
    "permissions": ["管理全部节点", "下发命令", "文件操作"],
    "responsibility": "负责所有 AgentGrid 节点的纳管、巡检、安装、更新和故障处理。"
  },
  "credentials": {
    "auth_type": "bearer_token",
    "token_configured": true,
    "token_hint": "****abcd",
    "credential_status": "active",
    "account_username": "ops-agent",
    "credential_refs": {
      "ssh": "operator-provided-session"
    }
  },
  "access": {
    "node_scope": {
      "mode": "all",
      "nodes": [],
      "groups": [],
      "os": [],
      "reason": "运维员工负责全节点维护，允许挂所有节点"
    },
    "tool_scope": {
      "mode": "all",
      "tools": [],
      "reason": "运维需要按任务协议调用命令、文件、安装、桌面等能力"
    }
  },
  "status": {
    "state": "online"
  }
}
```

Node scope modes:

- `all`: employee may operate every online eligible node.
- `nodes`: employee may operate only listed node IDs.
- `group` or `groups`: employee may operate nodes in listed groups.
- `os`: employee may operate nodes with listed operating systems.
- `none`: employee may not directly operate nodes.

Tool scope modes:

- `all`: employee may request any registered tool through the task protocol.
- `tools`: employee may request only listed tool IDs.
- `declared`: employee may create or review tasks, but direct tool execution must
  be declared by the task owner or template.
- `none`: employee may not request executable tools.

This model is intentionally practical:

- The console can show who can manage which nodes.
- Every task can carry `created_by` and `owner` as auditable identities.
- Scheduler and policy can later enforce the same data without changing the API
  shape.
- Real tokens are not returned by the API. Hub returns only `token_configured`
  and `token_hint`.

For example, an operations employee may have `node_scope.mode = "all"`. A review
employee may have `node_scope.mode = "none"` because review does not require
direct machine access.

## 10. Windows Worker And Desktop Helper Boundary

Windows has two different execution contexts. AgentGrid must model them as two
nodes so humans and AI clients know exactly which one to call.

Normal Windows Worker:

- Node id: `<node_id>`, for example `ZZH0610-windows`.
- Runtime identity: background service or scheduled task, usually
  `NT AUTHORITY\SYSTEM`.
- Business purpose: operate the machine in the background.
- Standard tasks: command execution, file read/write/list, service operations,
  software install, Git, Docker, long-running session, plugin execution.
- Must not be used for real user-desktop control because it is not the logged-in
  user's interactive session.

Windows Desktop Helper:

- Node id: `<node_id>-desktop`, for example `ZZH0610-windows-desktop`.
- Runtime identity: the logged-in Windows user session.
- Business purpose: sense and operate the visible desktop.
- Standard tasks: `desktop.screenshot`, `desktop.click`,
  `desktop.type_text`, `desktop.key`, and future foreground-app controls.
- Should be targeted only for desktop tasks. Background installs and service
  changes should still go to the normal Worker.

Routing rule:

```text
background machine operation -> normal Worker
visible screen / mouse / keyboard operation -> Desktop Helper
```

This boundary prevents the common mistake where a screenshot or click task is
scheduled to a service session that has high permissions but cannot see the real
desktop.

## 11. Desktop Operation Standard v1

Desktop tasks are structured compute tasks with `type = "desktop"`.

Supported operations:

```json
{ "type": "desktop", "operation": "screenshot", "path": null, "timeout_seconds": 30 }
```

```json
{ "type": "desktop", "operation": "click", "x": 100, "y": 100, "button": "left", "timeout_seconds": 10 }
```

```json
{ "type": "desktop", "operation": "type_text", "text": "hello", "interval_ms": 0, "timeout_seconds": 30 }
```

```json
{ "type": "desktop", "operation": "key", "key": "ESC", "modifiers": [], "timeout_seconds": 10 }
```

Every desktop task should create an auditable operation chain:

- who submitted it: `metadata.created_by`
- why it was placed on the node: scheduler decision or schedule preview
- what it executed: structured desktop payload
- what it produced: screenshot artifact or structured desktop result
- why it failed: structured error message

Screenshot artifacts should be stored in Hub when possible so the console can
show a screen timeline rather than only saying that a task completed.

## 12. AI Workbench Standard v1

A Workbench is a real machine or station that AI can use through structured
capabilities.

Workbench types:

- `hardware_bench`: a physical test station. Typical capabilities include
  `command`, `file`, `serial`, `flash`, `test`, and `plugin`.
- `desktop_bench`: a visible Windows/macOS desktop station. Typical
  capabilities include `desktop`, `browser`, and `file`.
- `compute_bench`: a background compute/tool station. Typical capabilities
  include `command`, `git`, `docker`, `session`, and `plugin`.

Routing rule:

```text
task depends on a real machine/device/desktop/account/local SDK/path
=> use node:<workbench_id>
```

AI client flow:

1. Discover workbenches from `/api/runtime-standard/workbench`.
2. Discover tools from `/api/capabilities/manifest` or `/api/tools`.
3. Prefer verified tools and online workbenches.
4. Submit structured tasks with hard placement when the operation is tied to a
   physical station.
5. Read execution record and evidence before deciding the next step.

## 13. Device Standard v1

A Device is an addressable thing behind a Workbench capability.

Device examples:

- `desktop`: screenshot, click, type text, key.
- `browser`: fetch, automate, screenshot, download.
- `filesystem`: list, read, write, upload, download.
- `serial`: open, write, read, capture log.
- `flasher`: erase, flash, verify.
- `test_rig`: run test, collect report.
- `plugin_runtime`: node-specific custom tools.

AgentGrid does not require every node to expose the same devices. Heterogeneity
is expected. The Hub standardizes schemas, probe status, scheduling, evidence,
and audit records.

## 14. Evidence Standard v1

Evidence is the proof an AI task leaves behind.

Evidence types:

- `screenshot`
- `stdout_log`
- `stderr_log`
- `serial_log`
- `file_artifact`
- `test_report`
- `operation_timeline`

Minimum evidence record:

```json
{
  "task_id": "task_xxx",
  "node_id": "workbench-id",
  "created_by": "agent-or-human",
  "operation": "desktop.screenshot",
  "scheduler_reason": "why this workbench was selected",
  "artifacts": [],
  "result": {},
  "error": null,
  "audit": []
}
```

For hardware and desktop automation, a task without evidence is not enough.
Screenshots, logs, reports, and output files should be stored as Hub artifacts
when possible.

## 15. Runbook Standard v1

A Runbook is a structured procedure AI can call against workbenches.

Core runbooks:

- `hardware.compile_flash_serial_test`: compile code, flash board, read serial,
  collect test report, judge pass/fail.
- `desktop.observe_operate_collect`: screenshot before, operate desktop,
  screenshot after, collect files.
- `capability.probe_and_use`: discover capability, probe tool, submit task,
  read execution record.

Implementation mapping:

- Single step: `AgentTask`.
- Dependent steps: `Workflow`.
- Recoverable batch: `Job Runtime`.
- Custom station action: `Node Tool` or `Worker Plugin`.

## 16. Capability Marketplace Standard v1

The capability marketplace is the AI-facing catalog of what real machines and
stations can do right now.

Marketplace rules:

- Every item has a stable `tool_id`.
- Every callable tool publishes `input_schema` and `output_schema`.
- Node-specific tools are allowed and expected.
- Probe status influences scheduling and AI trust.
- AgentGrid standardizes contracts, not the machines themselves.

## 17. Mobile SDK Standard v1

AgentGrid Mobile SDK is for phone-side console clients on iOS and Android.
The phone is not a Worker and does not execute scheduled tasks.

Mobile client responsibilities:

- Show cluster health, online/offline nodes, OS, address/IP, CPU, memory, disk,
  and running jobs.
- Show workbenches, devices, tools, and verified capabilities.
- Submit structured Agent Runtime tasks.
- Poll task status and task events.
- Read execution records and scheduler reasons.
- View Hub artifacts such as screenshots, logs, reports, and result files.

Mobile client non-goals:

- No Worker runtime.
- No scheduler implementation.
- No desktop helper.
- No local task execution.
- No natural-language parsing.

Required standard endpoint:

```http
GET /api/runtime-standard/mobile-sdk
```

Required SDK methods:

- `health`
- `runtimeStandard`
- `mobileSdkStandard`
- `workbenches`
- `devices`
- `evidenceStandard`
- `nodes`
- `tools`
- `submitTask`
- `getTask`
- `taskEvents`
- `executionRecord`
- `artifacts`
- `artifactDownloadUrl`
- `taskTemplates`
- `startTaskTemplate`

Recommended mobile screens:

- Cluster overview.
- Workbench detail.
- Submit task from tool or template.
- Task timeline.
- Screenshot/artifact viewer.

Polling rule:

```text
poll every 2 seconds by default
stop on done/failed/cancelled/stopped/skipped
future realtime task status uses SSE
interactive terminal/session uses WebSocket
```

## 18. Plugin Runtime Standard v1

Plugin Runtime v1 defines how AgentGrid nodes expose custom local tools without
hardcoding every ability into the Worker.

Core distinction:

- `plugin_id`: the installed implementation package, for example
  `agentgrid-plugin-document-parser`.
- `tool_id`: the AI-facing callable contract, for example `document.parse`.
- `executor`: `plugin:<plugin_id>`.

Required plugin manifest fields:

- `plugin_id`
- `name`
- `version`
- `platforms`
- `entrypoint`
- `tools`
- `dependencies`
- `probe`
- `risk`

Runtime contract:

- Plugin requests use `agentgrid.plugin/v1`.
- Plugin results must be structured JSON.
- Plugin errors use stable codes such as `plugin_not_found`,
  `dependency_missing`, `plugin_timeout`, and `invalid_plugin_output`.
- Plugin-backed tools should publish input schema, output schema, examples,
  default timeout, probe payload, and expected artifacts.

Scheduling rule:

```text
eligible nodes -> resource score -> probe trust -> plugin risk -> capability graph fit -> selected node
```

This means a declared plugin is not enough. The Hub should prefer verified
tools, down-rank high-risk or unverified plugins, and record the placement
reason.

Endpoint:

```http
GET /api/runtime-standard/plugin-runtime
```

CLI:

```bash
agentgrid standard plugin-runtime
```

## 19. Capability Graph Standard v1

Capability Graph is the relationship map behind AgentGrid scheduling.

It connects:

- Node -> capability.
- Node -> device.
- Capability -> tool.
- Node -> tool with probe status.
- Tool -> plugin dependency.
- Tool -> evidence type.
- Tool -> task intent.

This prevents AI clients from treating all nodes as equal. A task that needs a
visible desktop, a hardware flasher, a serial port, a local SDK, or a specific
plugin must be planned through the graph.

Endpoint:

```http
GET /api/runtime-standard/capability-graph
```

CLI:

```bash
agentgrid standard capability-graph
```

## 20. Execution Contract Standard v1

Execution Contract keeps tools and plugins from drifting into incompatible
result formats.

Every tool family should define:

- Input schema.
- Output schema.
- Error envelope.
- Default timeout.
- Retry and recovery behavior.
- Required artifacts.
- Audit events.

Common error envelope:

```json
{
  "code": "timeout",
  "message": "tool timed out",
  "retryable": false,
  "result": null
}
```

Endpoint:

```http
GET /api/runtime-standard/execution-contract
```

## 21. Evidence Pipeline Standard v1

Evidence Pipeline turns task outputs into trustworthy product records.

Stages:

- Capture: Worker/plugin/desktop helper produces screenshots, logs, files,
  reports, serial output, or structured JSON.
- Normalize: Hub records content type, size, hash, task id, node id, and
  evidence type.
- Index: Hub links evidence to execution records.
- Preview: Web and Mobile show images, logs, reports, and files.
- Audit: Timeline records who did what and what was produced.

Endpoint:

```http
GET /api/runtime-standard/evidence-pipeline
```

## 22. Node Capability Probe Engine Standard v1

Probe Engine checks whether declared node tools are truly usable.

Probe states:

- `declared_unverified`
- `pending`
- `verified`
- `failed`
- `expired`
- `unsupported`
- `unavailable`

Flow:

1. Hub reads node tool registration.
2. Hub creates a probe AgentTask with hard node placement.
3. Worker executes the same tool path real tasks use.
4. Hub verifies the result.
5. Scheduler prefers verified tools and avoids failed tools.

Endpoint:

```http
GET /api/runtime-standard/probe-engine
```

## 23. Placement Engine Standard v1

Placement Engine decides where a task should run.

Hard constraints:

- Node id.
- OS.
- Required tool.
- Required capability.
- Required device.
- Online state.
- Policy allowed.

Soft constraints:

- Preferred or avoided nodes.
- CPU, memory, disk pressure.
- Concurrency slots.
- Probe verification.
- Historical success rate.
- Node weight.
- Risk and cost.

Endpoint:

```http
GET /api/runtime-standard/placement-engine
```

## 24. Task Intent Schema Standard v1

Task Intent is structured JSON produced by an AI client or application.
AgentGrid does not parse natural language.

Example:

```json
{
  "intent_type": "desktop.screenshot",
  "title": "Capture Windows desktop",
  "tool_id": "desktop.screenshot",
  "placement": {
    "node_id": "ZZH0610-windows-desktop",
    "required_capabilities": ["desktop"]
  },
  "payload": {
    "type": "desktop",
    "operation": "screenshot"
  },
  "evidence": ["screenshot"]
}
```

Endpoint:

```http
GET /api/runtime-standard/task-intent
```

## 25. Artifact Store Standard v2

Artifact Store v2 defines how AgentGrid stores task products.

Artifact metadata:

- `id`
- `task_id`
- `node_id`
- `tool_id`
- `artifact_type`
- `content_type`
- `size_bytes`
- `sha256`
- `preview.kind`
- `retention.policy`
- `large_file`
- `created_at`
- `download_url`

Supported product types:

- Screenshot.
- Log.
- Report.
- Binary file.
- Directory listing.
- Future large-file chunks.

Endpoint:

```http
GET /api/runtime-standard/artifact-store
```

## 26. Event Timeline Standard v1

Event Timeline is the shared event stream for Web, Mobile, Webhook, MCP, and
audit consumers.

Sources:

- Task events.
- Audit events.
- Node heartbeats.
- Probe events.
- Artifact events.
- Job events.
- Workflow events.
- Webhook deliveries.

Subscriptions:

- `GET /api/events`
- `GET /api/events/stream`
- `GET /api/agent-runtime/tasks/{task_id}/events`

Endpoint:

```http
GET /api/runtime-standard/event-timeline
```
