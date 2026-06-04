# AgentGrid Command and Task Reference

Version: v0.1
Audience: humans, AI agents, automation clients
Hub URL: https://hub.example.com/agentgrid

## 1. Purpose

AgentGrid is a resource-aware workbench scheduling layer for AI clients. Clients submit structured tasks to the Hub. Worker nodes report heartbeat, resources, devices, and capabilities. The Hub schedules tasks to the correct real machine, desktop bench, hardware bench, browser station, or local tool station, then records structured evidence and audit trails.

AgentGrid should not be positioned as a generic remote execution platform. Its highest-value boundary is AI operation of real machines, devices, desktops, and hardware/test workbenches.

This document is the operational contract for AI agents and human operators.

## 1.1 AI Client Quick Contract

AI clients should call AgentGrid with structured JSON only. Do not send natural language as a task instruction unless the selected tool explicitly accepts natural language.

Default Hub:

```text
https://hub.example.com/agentgrid
```

Default CLI:

```bash
agentgrid <command>
```

Use this decision rule:

- Use `agentgrid capabilities` first when you need to discover what the current cluster can run.
- Use `GET /api/workbenches` when a task depends on a real machine, desktop, device, local SDK, or hardware station.
- Use `agentgrid submit-http` for one simple HTTP request.
- Use `agentgrid submit-command` for one simple host command.
- Use `agentgrid tasks` when you already know the low-level AgentTask shape.
- Use `agentgrid jobs submit` for recoverable work, batch work, partitioned work, or work that needs reduce/final aggregation.
- Use `agentgrid workflows` when multiple different steps depend on each other.

## 1.2 Workbench and Node Channel Contract

AgentGrid models a real physical computer or station as a `Workbench`.
A Workbench can contain multiple node channels. Users should see the
Workbench. AI clients and the Hub may select the exact channel.

Node channel roles:

| Role | Use for | Do not use for |
| --- | --- | --- |
| `worker` | command, file, plugin, Git, Docker, session, software install | visible desktop click/type |
| `desktop` | screenshot, click, type text, key press, foreground app control | background command/file/software install |
| `service` | Codex local bridge, node-local service bridge | normal task execution |
| `bridge` | node-to-node port bridge control | normal task execution |
| `device` | serial, flashing, hardware station, device SDK | normal task execution |

REST:

```http
GET /api/workbenches
GET /api/workbenches/{id}
GET /api/workbenches/{id}/timeline
POST /api/workbenches/{id}/actions
```

Response shape:

```json
{
  "ok": true,
  "api_version": "agentgrid.workbench/v1",
  "kind": "WorkbenchList",
  "items": [
    {
      "api_version": "agentgrid.workbench/v1",
      "kind": "Workbench",
      "metadata": {
        "id": "physical-machine-id",
        "name": "CHENGCHONG"
      },
      "spec": {
        "channels": {
          "worker": { "kind": "Node" },
          "desktop": { "kind": "Node" },
          "service": { "kind": "Node" },
          "bridge": { "kind": "Node" },
          "device": { "kind": "Node" }
        },
        "capabilities": ["command", "file", "desktop", "port_bridge"],
        "tools": [],
        "local_services": []
      },
      "resources": {
        "cpu_cores": 8,
        "memory_mb": 16384,
        "memory_used_mb": 4096,
        "disk_total_mb": 512000,
        "disk_free_mb": 300000
      },
      "status": {
        "state": "online",
        "online_channels": 2,
        "total_channels": 5
      }
    }
  ]
}
```

Scheduling explanation:

- Task detail and schedule preview include `channel_role`,
  `required_channel_role`, `channel_explanation`, and `task_requires`.
- Command/file/plugin/session tasks require `worker`.
- Desktop screenshot/click/type/key tasks require `desktop`.
- `service`, `bridge`, and `device` are specialized channels and do not lease
  normal background tasks unless a future task contract explicitly targets that
  channel.

Workbench Action API is the standard product and AI-client entry point for
operating one physical computer. The caller provides `workbench_id`, `action`,
and a structured `payload`. Hub validates the Workbench, selects the correct
channel, creates the task or stateful bridge session, and returns a routing
explanation.

```http
POST /api/workbenches/{workbench_id}/actions
Content-Type: application/json
```

```json
{
  "action": "command.run",
  "payload": {
    "program": "hostname",
    "args": [],
    "working_dir": null,
    "timeout_seconds": 30
  },
  "title": "Run hostname on this computer",
  "created_by": "agentgrid-mcp"
}
```

Supported v1 actions:

| Action | Channel | Payload |
| --- | --- | --- |
| `command.run` | `worker` | `program`, `args`, `working_dir`, `timeout_seconds` |
| `file.list` / `file.read` / `file.write` | `worker` | `operation`, `path`, file options |
| `runtime.submit` | `worker` | `tool_id`, `payload` |
| `desktop.screenshot` | `desktop` | `path`, `timeout_seconds` |
| `desktop.click` | `desktop` | `x`, `y`, `timeout_seconds` |
| `desktop.type_text` | `desktop` | `text`, `timeout_seconds` |
| `desktop.key` | `desktop` | `key`, `modifiers`, `timeout_seconds` |
| `port_bridge.create` | `bridge` or `worker` | `target_node_id` or `target_workbench_id`, `target_port`, bridge options |

Task-backed actions return `task_id`, `message_id`, `selected_channel`, and
`routing_reason`. Stateful bridge actions return `port_bridge_id` and
`port_bridge`.

## 1.3 Capability Manifest

Capability Manifest is the canonical discovery contract for AI clients. It answers one question:

```text
What can this AgentGrid cluster run right now?
```

CLI:

```bash
agentgrid capabilities
```

REST:

```http
GET /api/capabilities/manifest
```

Response shape:

```json
{
  "ok": true,
  "api_version": "agentgrid.capabilities/v1",
  "kind": "CapabilityManifest",
  "metadata": {
    "project_id": "agentgrid",
    "hub_url": "https://hub.example.com/agentgrid",
    "generated_at": "2026-06-01T00:00:00Z"
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
  "tools": []
}
```

Each `tools[]` item contains:

| Field | Meaning |
| --- | --- |
| `tool_id` | Stable tool id, for example `command.run` or `http.request`. |
| `input_schema` | JSON Schema-like payload contract. |
| `output_schema` | Expected structured result contract. |
| `available_nodes` | Online nodes that can run this tool now. |
| `verified_nodes` | Nodes that passed Tool Probe. |
| `supports_partition` | Whether this tool is suitable for sharded Job execution. |
| `supports_template` | Whether Job payload template variables can be rendered. |
| `recommended_reduce` | Suggested reduce strategy for batch jobs. |
| `nodes[]` | Concrete nodes with OS, address, resources, concurrency, and verification status. |
| `job_example` | Ready-to-adapt Job request body. |

AI client workflow:

1. Call `agentgrid capabilities`.
2. Select a `tool_id` whose `available_nodes > 0`.
3. Prefer tools with `verified_nodes > 0`.
4. Build payload from `input_schema` and `examples`.
5. For batch work, use `job_example`, `partition`, template variables, and `recommended_reduce`.
6. Submit through `POST /api/jobs` or `agentgrid jobs submit`.
7. Read `agentgrid jobs get --id job_xxx` until `status.state` becomes `done` or `failed`.

Important boundary:

- AgentGrid accepts structured tasks and tool payloads.
- AgentGrid does not convert natural language into actions.
- Natural-language planning belongs to the AI client before it calls AgentGrid.

Most AI clients should prefer Job Runtime for non-trivial work:

```text
Job -> Partition -> Shards -> Worker Tasks -> Reduce -> Final Result
```

Minimum sharded Job:

```bash
agentgrid jobs plan \
  --tool command.run \
  --title "hostname batch" \
  --payload '{"type":"command","program":"hostname","args":[],"timeout_seconds":30}' \
  --os linux \
  --shards 2 \
  --max-parallelism 2 \
  --reduce stdout_concat
```

If the plan is valid, submit:

```bash
agentgrid jobs submit \
  --tool command.run \
  --title "hostname batch" \
  --payload '{"type":"command","program":"hostname","args":[],"timeout_seconds":30}' \
  --os linux \
  --shards 2 \
  --max-parallelism 2 \
  --reduce stdout_concat \
  --wait
```

Submit a recoverable Job to a physical Workbench. Hub keeps the Workbench
constraint when it creates replacement attempts:

```bash
agentgrid jobs submit \
  --tool command.run \
  --title "hostname on CHENGCHONG" \
  --payload '{"type":"command","program":"hostname","args":[],"timeout_seconds":30}' \
  --workbench sha256:091c56d7a82c696b759efbf8836b6f8e82cf507deeda35e170fc42134a2caece \
  --wait
```

Minimum items partition Job:

```bash
agentgrid jobs submit \
  --tool command.run \
  --title "echo items" \
  --payload '{"type":"command","program":"echo","args":["${partition.items[0]}","shard-${shard.index}"],"timeout_seconds":30}' \
  --os linux \
  --shards 2 \
  --max-parallelism 2 \
  --partition-items '["alpha","beta"]' \
  --reduce stdout_concat \
  --wait
```

Minimum batch HTTP Job:

```bash
agentgrid jobs submit \
  --tool http.request \
  --title "fetch urls" \
  --payload '{"type":"http_request","method":"GET","url":"${partition.items[0]}","headers":[],"body":null,"timeout_seconds":30,"max_response_bytes":65536}' \
  --os linux \
  --shards 2 \
  --max-parallelism 2 \
  --partition-items '["https://example.com","https://httpbin.org/get"]' \
  --reduce json_array \
  --wait
```

REST equivalent:

```http
POST /api/jobs
Content-Type: application/json
```

```json
{
  "title": "fetch urls",
  "tool_id": "http.request",
  "payload": {
    "type": "http_request",
    "method": "GET",
    "url": "${partition.items[0]}",
    "headers": [],
    "body": null,
    "timeout_seconds": 30,
    "max_response_bytes": 65536
  },
  "placement": {
    "os": "linux",
    "workbench_id": "optional-physical-machine-id"
  },
  "strategy": {
    "type": "sharded",
    "shard_count": 2,
    "max_parallelism": 2,
    "payload_mode": "inject_shard"
  },
  "partition": {
    "type": "items",
    "items": ["https://example.com", "https://httpbin.org/get"]
  },
  "reduce": {
    "type": "json_array"
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
}
```

After submit, read the Job:

```bash
agentgrid jobs get --id job_xxx
```

Important response fields:

- `status.state`: `running`, `done`, or `failed`.
- `shards[]`: each shard payload, node, task, and result.
- `status.reducer_task_id`: reducer task created by Hub.
- `status.result`: final reduced result for the Job.

Template variables available in Job payload:

| Template | Meaning |
| --- | --- |
| `${shard.index}` | Current shard index. |
| `${shard.count}` | Total shard count. |
| `${partition.items[0]}` | First item assigned to this shard. |
| `${partition.range.start}` | Current shard range start. |
| `${partition.range.end}` | Current shard range end. |

Supported reduce strategies:

| Strategy | When to use |
| --- | --- |
| `summary` | Need only success/failure counts and shard records. |
| `stdout_concat` | Command shards produce text in stdout. |
| `json_array` | HTTP/file/browser shards produce structured JSON results. |

## 2. Node Model

Worker nodes do not need inbound ports. Every Worker actively connects to the Hub.

Node heartbeat rules:

- Last heartbeat <= 30 seconds: online
- Last heartbeat > 30 seconds and <= 120 seconds: unknown
- Last heartbeat > 120 seconds: offline
- offline and unknown nodes must not receive new tasks

Current node IDs:

- hub-node: center server node
- linux-worker-01: Linux child node
- local-mac: local macOS child node

## 3. Scheduler Rules

Hub owns scheduling decisions. Workers request leases, but the Hub only leases a task to the best eligible node.

AgentGrid scheduling is a two-stage contract:

1. Eligibility Gate: decide whether a node is allowed to run the task.
2. Optimization Score: choose the best node only from eligible nodes.

Hard constraints must never be overridden by scoring. If a task explicitly targets `node:ZZH0610-windows`, Linux nodes such as `linux-worker-01` are rejected before CPU, memory, weight, success rate, or trust multiplier are considered.

Eligibility hard constraints:

- Node state must be online
- Node capability must match task label
- Required node label must match when labels include node:<node_id>
- Required OS label must match when labels include os:<name>
- Required registered tool must be available when a dynamic tool is requested
- Avoided nodes must be rejected
- Nodes without free concurrent slots must be rejected
- High load nodes are skipped

Optimization score inputs:

- CPU usage
- Memory usage
- Disk usage
- Running job count
- Node weight
- Available concurrent slots
- Historical success rate
- Preferred or avoided nodes
- Tool Probe trust multiplier

Lower score is better.

Queue priority:

- p0 / urgent: first
- high / p1: second
- normal / p2: default
- low: last

The Hub records the scheduling reason and candidate scores in the `task.leased` audit event.

## 4. Task Labels

Required executable labels:

- compute
- http_request for HTTP tasks
- command for command tasks
- file for file read/write/list tasks
- git for Git repository tasks
- docker for Docker/container tasks
- browser for browser fetch tasks
- agentmessage for AgentMessage collaboration tasks

Optional routing labels:

- node:<node_id>
- workbench:<workbench_id>
- os:linux
- os:mac
- os:windows
- tag:<tag>
- group:<group>
- prefer:<node_id>
- avoid:<node_id>

Examples:

```json
["compute", "command", "node:linux-worker-01"]
```

```json
["compute", "command", "workbench:sha256:091c56d7..."]
```

```json
["compute", "http_request", "os:linux"]
```

Business classification:

- `node:<node_id>` is a hard placement constraint. It means "run on this exact node", not "prefer this node".
- `workbench:<workbench_id>` is a hard physical-machine constraint. It means
  "run on this physical computer or station"; Hub then selects the correct
  `worker`, `desktop`, `service`, `bridge`, or `device` channel for the task.
- If both `node:<node_id>` and `workbench:<workbench_id>` are present, the node
  must belong to that Workbench or Hub rejects the task.
- OS, capability, dynamic tool availability, node state, avoid list, slot availability, and high-load rejection are also hard constraints.
- CPU, memory, disk, weight, historical success rate, preferred nodes, and Tool Probe trust are optimization inputs after eligibility is decided.
- Most user-facing and AI-facing tasks should prefer `workbench:<workbench_id>`
  so people do not need to remember split channel node IDs.
- Node operations that mutate one exact channel runtime can still include
  `node:<node_id>`.

This prevents a task targeting a Windows machine from being optimized onto a Linux node just because the Linux node has a better resource score.

## 5. HTTP Task Payload

Submit an HTTP task by creating an AgentTask with the first input as a JSON string:

```json
{
  "type": "http_request",
  "method": "GET",
  "url": "https://httpbin.org/get",
  "headers": [],
  "body": null,
  "timeout_seconds": 30,
  "max_response_bytes": 65536
}
```

Expected result:

```json
{
  "type": "http_response",
  "status_code": 200,
  "headers": [["content-type", "application/json"]],
  "body": {},
  "duration_ms": 123
}
```

## 6. Command Task Payload

Submit a command task by creating an AgentTask with the first input as a JSON string:

```json
{
  "type": "command",
  "program": "hostname",
  "args": [],
  "working_dir": null,
  "timeout_seconds": 30
}
```

Expected result:

```json
{
  "type": "command_result",
  "exit_code": 0,
  "stdout": "hanfeihu\n",
  "stderr": "",
  "duration_ms": 3
}
```

Command execution is policy-controlled. Only allowlisted programs may run.

Execution state rule:

- `exit_code = 0`: task is completed successfully.
- `exit_code != 0`: Worker reports the task as `failed`.
- The failed task keeps the original `command_result` under `status.error.result`, including `stdout`, `stderr`, `exit_code`, and `duration_ms`.
- Workflow failure policy uses this task state. An optional workflow node can turn this failure into a skipped workflow run and continue downstream nodes.

## 7. File Task Payload

Read a file:

```json
{
  "type": "file",
  "operation": "read",
  "path": "/tmp/agentgrid.txt",
  "max_bytes": 65536
}
```

Write a file:

```json
{
  "type": "file",
  "operation": "write",
  "path": "/tmp/agentgrid.txt",
  "content": "hello",
  "append": false,
  "create_dirs": true
}
```

List a directory:

```json
{
  "type": "file",
  "operation": "list",
  "path": "/tmp",
  "recursive": false,
  "max_entries": 200
}
```

Expected result:

```json
{
  "type": "file_result",
  "operation": "list",
  "path": "/tmp",
  "content": null,
  "entries": [],
  "bytes": 0,
  "duration_ms": 10
}
```

## 8. Git Task Payload

Supported operations: `clone`, `pull`, `status`, `checkout`.

```json
{
  "type": "git",
  "operation": "status",
  "repo_dir": "/srv/project"
}
```

```json
{
  "type": "git",
  "operation": "clone",
  "repo": "https://github.com/example/repo.git",
  "dest": "/srv/repo",
  "branch": "main",
  "depth": 1
}
```

Expected result:

```json
{
  "type": "git_result",
  "operation": "git.status",
  "exit_code": 0,
  "stdout": "",
  "stderr": "",
  "duration_ms": 20
}
```

## 9. Docker Task Payload

Supported operations: `ps`, `images`, `run`.

```json
{
  "type": "docker",
  "operation": "ps"
}
```

```json
{
  "type": "docker",
  "operation": "run",
  "image": "alpine:latest",
  "args": ["echo", "hello"],
  "timeout_seconds": 60
}
```

Expected result:

```json
{
  "type": "docker_result",
  "operation": "docker.ps",
  "exit_code": 0,
  "stdout": "",
  "stderr": "",
  "duration_ms": 30
}
```

## 10. Browser Task Payload

The current browser task is a lightweight fetch/title extraction executor. A full Playwright-style browser runtime can be added later.

```json
{
  "type": "browser",
  "operation": "fetch",
  "url": "https://example.com",
  "selector": null,
  "timeout_seconds": 30,
  "max_response_bytes": 65536
}
```

Expected result:

```json
{
  "type": "browser_result",
  "url": "https://example.com",
  "status_code": 200,
  "title": "Example Domain",
  "text": "...",
  "duration_ms": 120
}
```

## 11. AgentMessage Task Payload

This task type lets the compute layer participate in AI collaboration workflows.

```json
{
  "type": "agent_message",
  "from": "architect-agent",
  "to": ["worker-agent"],
  "message_type": "task.assigned",
  "subject": "Check the queue",
  "summary": "Please inspect assigned tasks.",
  "payload": {}
}
```

Expected result:

```json
{
  "type": "agent_message_result",
  "delivered": true,
  "message_id": null,
  "summary": "message prepared by worker",
  "duration_ms": 1
}
```

## 12. REST API

Base URL:

```text
https://hub.example.com/agentgrid/api
```

Health:

```http
GET /health
```

List nodes:

```http
GET /nodes
```

List tasks:

```http
GET /tasks?limit=100
```

Get task:

```http
GET /tasks/{task_id}
```

Create task:

```http
POST /tasks
Content-Type: application/json
```

List audit events:

```http
GET /audit-events
```

Read policy:

```http
GET /policy
```

Worker lease endpoint:

```http
POST /worker/lease
```

Worker completion endpoint:

```http
POST /worker/tasks/{task_id}/complete
```

Worker failure endpoint:

```http
POST /worker/tasks/{task_id}/fail
```

## 12.1 Node Port Bridge API

Node Port Bridge lets the Hub ask one Worker node to expose a loopback TCP port
that forwards traffic to another Worker node's local service.

Business meaning:

```text
A node opens http://127.0.0.1:<source_port>
  -> AgentGrid Hub coordinates the bridge
  -> B node connects to <target_host>:<target_port>
```

Use this when a tool, browser, IDE, debugger, web console, or local service is
available only from one node, but another node needs to access it through
AgentGrid.

Important boundaries:

- v1 supports TCP only.
- The source Worker binds only to `127.0.0.1`.
- `source_bind_port` may be `0`; the source Worker then chooses an available port.
- `target_host` must be `127.0.0.1`, `localhost`, `::1`, or a private/link-local IP.
- Both source and target nodes must be online.
- Both nodes must declare `port_bridge`, `plugin`, or `session` capability.
- Both Workers must keep their outbound Hub bridge WebSocket connected.
- Child nodes do not need inbound public ports.
- This is node-to-node access. The client that creates the bridge does not relay TCP bytes.

List active bridges:

```http
GET /port-bridges
```

Create a bridge:

```http
POST /port-bridges
Content-Type: application/json
```

```json
{
  "source_node_id": "local-mac",
  "target_node_id": "linux-worker-01",
  "source_bind_host": "127.0.0.1",
  "source_bind_port": 18080,
  "target_host": "127.0.0.1",
  "target_port": 8080,
  "protocol": "tcp",
  "ttl_seconds": 1800,
  "purpose": "Open B node local web console from A node",
  "created_by": "agentgrid-cli"
}
```

Read one bridge:

```http
GET /port-bridges/{port_bridge_id}
```

Close one bridge:

```http
DELETE /port-bridges/{port_bridge_id}
```

Response shape:

```json
{
  "ok": true,
  "item": {
    "api_version": "agentgrid.bridge/v1",
    "kind": "PortBridgeSession",
    "metadata": {
      "id": "pbridge_xxx",
      "created_at": "2026-06-02T00:00:00Z",
      "expires_at": "2026-06-02T00:30:00Z",
      "created_by": "agentgrid-cli"
    },
    "spec": {
      "source_node_id": "local-mac",
      "target_node_id": "linux-worker-01",
      "source_bind_host": "127.0.0.1",
      "source_bind_port": 18080,
      "target_host": "127.0.0.1",
      "target_port": 8080,
      "protocol": "tcp",
      "purpose": "Open B node local web console from A node"
    },
    "status": {
      "state": "ready",
      "source_connected": true,
      "target_connected": true,
      "source_url": "http://127.0.0.1:18080",
      "last_error": null
    }
  }
}
```

State meanings:

| State | Meaning |
| --- | --- |
| `starting` | Hub accepted the request and is asking Workers to prepare the bridge. |
| `waiting_for_worker` | One or both Workers are not connected to the Hub bridge channel. |
| `ready` | The source node can open `status.source_url`. |
| `closed` | The bridge has been closed manually or by TTL. |
| `failed` | A Worker reported an error. Read `status.last_error`. |

## 13. Remote Interactive Terminal

The Web console supports an interactive terminal over WebSocket.

Topology:

- Browser connects to Hub.
- Worker keeps an outbound WebSocket connection to Hub.
- Hub forwards terminal input and output between Browser and Worker.
- Child nodes do not need inbound ports.

Browser WebSocket:

```text
wss://hub.example.com/agentgrid/api/terminal/ws?node_id=<node_id>
```

Worker WebSocket:

```text
wss://hub.example.com/agentgrid/api/worker/terminal/ws?node_id=<node_id>
```

Open message sent by Hub to Worker:

```json
{
  "type": "terminal.open",
  "session_id": "term_xxx",
  "node_id": "hub-node",
  "cols": 120,
  "rows": 32
}
```

Input message:

```json
{
  "type": "terminal.input",
  "session_id": "term_xxx",
  "data": "pwd\n"
}
```

Output message:

```json
{
  "type": "terminal.output",
  "session_id": "term_xxx",
  "stream": "pty",
  "data": "/home/agentgrid\n"
}
```

Close message:

```json
{
  "type": "terminal.close",
  "session_id": "term_xxx"
}
```

Implementation notes:

- Worker uses a cross-platform PTY for Linux, macOS, and Windows.
- This is for operator remote control, not normal queued task execution.
- Current v1 supports one or more live sessions while the Worker process is running.

## 14. Create Task Request

```json
{
  "title": "Run hostname on jia",
  "summary": "Run a safe command on a selected node.",
  "created_by": "architect-agent",
  "owner": "worker-agent",
  "assigned_to": ["worker-agent"],
  "priority": "normal",
  "labels": ["compute", "command", "node:linux-worker-01"],
  "inputs": [
    "{\n  \"type\": \"command\",\n  \"program\": \"hostname\",\n  \"args\": [],\n  \"working_dir\": null,\n  \"timeout_seconds\": 30\n}"
  ],
  "outputs": ["exit_code", "stdout", "stderr", "duration_ms"],
  "acceptance_criteria": [
    "Hub selects the eligible node",
    "Worker executes only if policy allows it",
    "Worker writes structured result back"
  ],
  "verify": {
    "presets": ["command.exit_zero"],
    "rules": [
      {
        "path": "result.stdout",
        "op": "contains",
        "value": "hanfeihu",
        "description": "stdout should include the expected hostname"
      }
    ]
  }
}
```

## 15. Result Verification

Result Verification is Hub-side task acceptance. Workers only execute and report structured results. The Hub evaluates `verify` after completion and writes the verdict to `status.result.verification`.

Supported presets:

- `command.exit_zero`: `result.exit_code == 0`
- `http.status_2xx`: `200 <= result.status_code < 300`
- `file.non_empty`: `result.bytes > 0`
- `browser.has_text`: `result.text` exists and is not empty
- `agentmessage.delivered`: `result.delivered == true`

Supported rule operations:

- `exists`
- `eq`
- `neq`
- `contains`
- `not_contains`
- `gt`
- `gte`
- `lt`
- `lte`
- `regex`
- `json_type`

Rule paths use dot notation against a wrapper object that contains the Worker result at `result`.

Example verification config:

```json
{
  "presets": ["command.exit_zero"],
  "rules": [
    {
      "path": "result.stdout",
      "op": "contains",
      "value": "hanfeihu",
      "description": "stdout contains expected hostname"
    }
  ]
}
```

Verification result shape:

```json
{
  "state": "passed",
  "passed": true,
  "checked_at": "2026-05-31T12:30:00.000000Z",
  "summary": "2/2 条规则通过",
  "rules": [
    {
      "ok": true,
      "path": "result.exit_code",
      "op": "eq",
      "expected": 0,
      "actual": 0
    }
  ]
}
```

The Hub records `task.result.verified` or `task.result.verification_failed` in audit events.

## 16. CLI Commands

The CLI has the production Hub URL built in. Users normally do not need to pass `--hub`.

Use the explicit Hub URL only for development or migration testing:

```bash
agentgrid --hub https://hub.example.com/agentgrid health
```

Health:

```bash
agentgrid health
```

List agents:

```bash
agentgrid agents
```

List messages:

```bash
agentgrid messages --limit 20
```

List tasks:

```bash
agentgrid tasks
```

List tasks with explicit subcommand:

```bash
agentgrid tasks list
```

Get one task by ID:

```bash
agentgrid tasks get --id task_xxx
```

Watch one task until it finishes:

```bash
agentgrid tasks watch --id task_xxx
```

Print task logs:

```bash
agentgrid tasks logs --id task_xxx
```

Explain scheduling:

```bash
agentgrid tasks explain --id task_xxx
```

List nodes:

```bash
agentgrid nodes
```

Get one node:

```bash
agentgrid nodes get --id linux-worker-01
```

Workbench/computer entry points:

```bash
agentgrid workbench list
agentgrid workbench get --id sha256:091c56d7a82c696b759efbf8836b6f8e82cf507deeda35e170fc42134a2caece
agentgrid workbench timeline --id sha256:091c56d7a82c696b759efbf8836b6f8e82cf507deeda35e170fc42134a2caece
```

Operate a physical computer/workbench. Hub chooses the concrete channel:

```bash
agentgrid workbench command \
  --id sha256:091c56d7a82c696b759efbf8836b6f8e82cf507deeda35e170fc42134a2caece \
  --program hostname \
  --wait
```

```bash
agentgrid workbench screenshot \
  --id sha256:091c56d7a82c696b759efbf8836b6f8e82cf507deeda35e170fc42134a2caece \
  --wait
```

```bash
agentgrid workbench file \
  --id sha256:091c56d7a82c696b759efbf8836b6f8e82cf507deeda35e170fc42134a2caece \
  --operation list \
  --path "C:\\" \
  --max-entries 200 \
  --wait
```

```bash
agentgrid workbench runtime \
  --id sha256:091c56d7a82c696b759efbf8836b6f8e82cf507deeda35e170fc42134a2caece \
  --tool audio.tts.clone \
  --payload-file payload.json \
  --wait
```

Generic Workbench Action entry point:

```bash
agentgrid workbench action \
  --id sha256:091c56d7a82c696b759efbf8836b6f8e82cf507deeda35e170fc42134a2caece \
  --action command.run \
  --payload '{"program":"hostname","args":[],"timeout_seconds":30}' \
  --wait
```

Workbench-based port bridge:

```bash
agentgrid workbench action \
  --id source_workbench_id \
  --action port_bridge.create \
  --payload '{"target_workbench_id":"target_workbench_id","target_port":8888,"source_bind_port":9999}'
```

Channel routing rule:

- `workbench command`, `workbench file`, and `workbench runtime` route to the `worker` channel.
- `workbench screenshot` routes to the `desktop` channel.
- The task timeline shows submitted tasks, selected channels, audit events, and execution state changes for that computer.

Show current policy:

```bash
agentgrid policy
```

Submit HTTP task:

```bash
agentgrid submit-http \
  --method GET \
  --url https://httpbin.org/get \
  --expect-status-2xx \
  --title "HTTP smoke test"
```

Submit command task to the best node:

```bash
agentgrid submit-command \
  --program hostname \
  --expect-exit-code 0 \
  --title "Run hostname"
```

Submit command task to a specific node:

```bash
agentgrid submit-command \
  --program hostname \
  --node linux-worker-01 \
  --expect-exit-code 0 \
  --expect-stdout-contains hanfeihu \
  --title "Run hostname on jia"
```

Submit command task to a physical computer/workbench. Hub chooses the
background `worker` channel automatically:

```bash
agentgrid submit-command \
  --program hostname \
  --workbench sha256:091c56d7a82c696b759efbf8836b6f8e82cf507deeda35e170fc42134a2caece \
  --wait
```

Submit command with raw verification JSON:

```bash
agentgrid submit-command \
  --program hostname \
  --wait \
  --verify-json '{"presets":["command.exit_zero"],"rules":[{"path":"result.stdout","op":"contains","value":"hanfeihu"}]}'
```

Submit command and wait for stdout/stderr:

```bash
agentgrid submit-command \
  --program hostname \
  --node linux-worker-01 \
  --wait
```

Submit command task to an OS class:

```bash
agentgrid submit-command \
  --program uname \
  --arg -a \
  --os linux \
  --title "Linux uname"
```

Submit file list task:

```bash
agentgrid submit-file \
  --operation list \
  --path /tmp \
  --max-entries 100 \
  --title "List tmp"
```

Submit file read task:

```bash
agentgrid submit-file \
  --operation read \
  --path /etc/hostname \
  --os linux \
  --title "Read Linux hostname file"
```

Submit Git status task:

```bash
agentgrid submit-git \
  --operation status \
  --repo-dir /srv/project \
  --title "Git status"
```

Submit Docker task:

```bash
agentgrid submit-docker \
  --operation ps \
  --node hub-node \
  --title "Docker containers"
```

Submit browser fetch task:

```bash
agentgrid submit-browser \
  --url https://example.com \
  --title "Fetch example.com"
```

Submit AgentMessage collaboration task:

```bash
agentgrid submit-agent-message \
  --from architect-agent \
  --to worker-agent \
  --type task.assigned \
  --subject "Check task queue" \
  --summary "Please inspect assigned tasks."
```

Send AgentMessage:

```bash
agentgrid send \
  --from architect-agent \
  --to worker-agent \
  --type task.assigned \
  --subject "Task assigned" \
  --summary "Please check the task queue."
```

Create a node-to-node port bridge:

```bash
agentgrid bridge-port \
  --source-node local-mac \
  --target-node linux-worker-01 \
  --target-port 8080 \
  --source-port 18080 \
  --purpose "Open linux-worker-01 local web console from local-mac"
```

Equivalent resource command:

```bash
agentgrid port-bridges create \
  --source-node local-mac \
  --target-node linux-worker-01 \
  --target-host 127.0.0.1 \
  --target-port 8080 \
  --source-port 18080 \
  --ttl-seconds 1800 \
  --purpose "Open linux-worker-01 local web console from local-mac"
```

List active port bridges:

```bash
agentgrid port-bridges
```

Get one port bridge:

```bash
agentgrid port-bridges get --id pbridge_xxx
```

Close one port bridge:

```bash
agentgrid port-bridges close --id pbridge_xxx
```

Text output example:

```text
Port Bridge ID: pbridge_xxx
State: ready
Source Node: local-mac
Target: linux-worker-01 -> 127.0.0.1:8080
Source URL: http://127.0.0.1:18080

查看详情: agentgrid port-bridges get --id pbridge_xxx
关闭桥接: agentgrid port-bridges close --id pbridge_xxx
```

JSON output:

```bash
agentgrid bridge-port \
  --source-node local-mac \
  --target-node linux-worker-01 \
  --target-port 8080 \
  --source-port 18080 \
  --output json
```

CLI parameter contract:

| Flag | Default | Meaning |
| --- | --- | --- |
| `--source-node` | required | Node that receives the local loopback port. Open `Source URL` on this node. |
| `--target-node` | required | Node that can reach the target service. |
| `--target-port` | required | Target TCP port on the target node side. |
| `--source-port` | `0` | Local loopback port on the source node. `0` lets Worker choose one. |
| `--target-host` | `127.0.0.1` | Host/IP reached from the target node. |
| `--bind-host` | `127.0.0.1` | Source bind host. v1 only allows `127.0.0.1`. |
| `--ttl-seconds` | `1800` | Auto-expire time, clamped by Hub to 30-86400 seconds. |
| `--protocol` | `tcp` | v1 only supports `tcp`. |
| `--purpose` | optional | Human-readable reason for audit and Web console. |
| `--created-by` | `agentgrid-cli` | Actor recorded in audit if no authenticated user session is present. |
| `--output` | `text` | Use `json` for machine-readable output. |

AI client rule:

- Use `bridge-port` for one quick bridge.
- Use `port-bridges create/get/close` when managing bridge lifecycle explicitly.
- After creation, poll `agentgrid port-bridges get --id pbridge_xxx` until `status.state` is `ready`.
- Only the source node can use `status.source_url`; opening it on the operator laptop works only when the operator laptop is the source node.
- Close the bridge when the task is done.

## 17. Workflow / DAG API

Workflow is the core protocol for multi-step automation. A workflow contains DAG nodes. Each node becomes a normal AgentTask only after all its dependencies are done.

Create workflow:

```http
POST /api/workflows
Content-Type: application/json
```

```json
{
  "name": "CI workflow",
  "summary": "Check repository and run tests",
  "created_by": "architect-agent",
  "nodes": [
    {
      "id": "git_status",
      "title": "Git status",
      "payload": {
        "type": "git",
        "operation": "status",
        "repo_dir": "/srv/project"
      },
      "labels": ["compute", "git", "node:hub-node"]
    },
    {
      "id": "run_tests",
      "title": "Run tests",
      "depends_on": ["git_status"],
      "payload": {
        "type": "command",
        "program": "sh",
        "args": ["-lc", "cargo test"],
        "working_dir": "/srv/project",
        "timeout_seconds": 600
      },
      "labels": ["compute", "command", "node:hub-node"]
    }
  ]
}
```

Start workflow:

```http
POST /api/workflows/{workflow_id}/start
Content-Type: application/json
```

```json
{
  "actor": "architect-agent"
}
```

List workflows:

```http
GET /api/workflows?limit=100
```

Get workflow detail:

```http
GET /api/workflows/{workflow_id}
```

Cancel workflow:

```http
POST /api/workflows/{workflow_id}/cancel
Content-Type: application/json
```

```json
{
  "actor": "architect-agent",
  "reason": "manual cancel"
}
```

Workflow states:

- draft: saved but not started
- running: workflow engine is releasing runnable nodes
- done: all DAG nodes completed
- failed: one node failed; v1 does not auto retry
- cancelled: cancelled by operator

Workflow run states:

- pending: waiting for dependencies
- ready: task created and waiting for Hub scheduler
- running: task leased by Worker
- done: task completed successfully
- failed: task failed
- cancelled: cancelled before execution

Rules:

- Node IDs must be unique.
- `depends_on` must reference existing node IDs.
- Cycles are rejected.
- Hub creates AgentTask records only when dependencies are done.
- Failed nodes stop the workflow in v1.
- Workflow-created tasks include `metadata.workflow_id` and `metadata.workflow_node_id`.

## 18. Task Detail Page Fields

The Web console task detail page should show:

- Input payload: the exact JSON submitted by the client
- Scheduling reason: selected node, score, and candidate list from `task.leased`
- Executing node: `status.leased_by_node_id`
- Result logs: `stdout`, `stderr`, command exit code, or typed result object
- Audit timeline: created, assigned, leased, completed, failed
- Failure reason: `status.error` and `status.blocked_reason`
- Result verification: `status.result.verification`

## 19. Security Policy

Policy is read from:

```http
GET /api/policy
```

Current policy shape:

```json
{
  "http": {
    "allowed_domains": [],
    "blocked_ips": ["127.0.0.1", "::1", "0.0.0.0"],
    "allow_private_network": false,
    "max_response_bytes": 65536
  },
  "command": {
    "enabled": true,
    "command_allowlist": ["pwd", "whoami", "hostname", "uname", "date", "ls"],
    "max_stdout_bytes": 65536,
    "max_stderr_bytes": 65536
  },
  "secrets": {
    "allow_env": false,
    "allowed_secret_refs": []
  }
}
```

Rules:

- Do not execute commands outside command_allowlist.
- Do not target localhost or private network addresses unless policy allows it.
- Do not expose environment secrets unless secrets.allow_env is true.
- Do not retry failed tasks automatically.

## 20. AI Agent Guidance

AI agents should:

- Prefer CLI commands for simple task submission.
- Prefer REST API for direct integration.
- Always set labels accurately.
- Use node:<node_id> only when a specific machine is required.
- Use os:<name> when the task depends on operating system behavior.
- Read task result from status.result.
- Read result verification from status.result.verification.
- Read failures from status.error.
- Use audit events for accountability.

AI agents must not:

- Submit destructive commands unless explicitly authorized.
- Assume offline or unknown nodes can run tasks.
- Retry failed tasks without human approval.
- Bypass policy by wrapping unsafe commands through shell interpreters.

## 20.1 AI Employee Identity And Scope

AgentGrid uses AI employee records as the business identity layer. This is the
right place to express "who this AI is", "what it is responsible for", and "which
nodes or tools it may operate".

Example operations employee:

```json
{
  "id": "ops-agent",
  "name": "运维员工",
  "role": "Ops 运维工程师",
  "skills": ["节点运维", "软件安装", "服务重启", "远程排障"],
  "permissions": ["管理全部节点", "下发命令", "文件操作", "桌面协助"],
  "responsibility": "负责所有 AgentGrid 节点的纳管、巡检、安装、更新和故障处理。",
  "auth_type": "bearer_token",
  "account_username": "ops-agent",
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
}
```

Register or update:

```bash
curl -X POST https://hub.example.com/agentgrid/api/agents \
  -H 'content-type: application/json' \
  -d '{
    "id": "ops-agent",
    "name": "运维员工",
    "role": "Ops 运维工程师",
    "skills": ["节点运维", "软件安装", "服务重启"],
    "permissions": ["管理全部节点", "下发命令", "文件操作"],
    "node_scope": { "mode": "all" },
    "tool_scope": { "mode": "all" },
    "status": "online"
  }'
```

Token rule:

- Submit `token` only when creating or rotating credentials.
- Hub stores only a hash.
- API responses return `credentials.token_configured` and
  `credentials.token_hint`, never the real token.

Scope rule:

- `node_scope` is a business placement boundary.
- `tool_scope` is a business capability boundary.
- Scheduler hard constraints such as `node:<node_id>`, `os:<name>`, and required
  tools still decide runtime eligibility.
- Later policy enforcement can use the same identity fields without changing task
  payloads.

## 20. Result States

Common task states:

- assigned: task exists and is waiting for a lease
- in_progress: task is leased or running
- done: task completed successfully
- failed: task failed and will not be retried automatically
- blocked: task requires human or system intervention

## 21. Worker Version and Compatibility

Nodes report Worker runtime metadata in heartbeat:

```json
{
  "worker_version": "0.1.0",
  "worker_target": "linux-x86_64",
  "glibc_version": "2.34",
  "auto_update_enabled": true,
  "update_channel": "stable"
}
```

The Hub update manifest accepts compatibility inputs:

```http
GET /api/worker/update-manifest?worker_target=linux-x86_64&glibc_version=2.34&current_sha256=...
```

The response includes:

- `compatible`: whether this binary can run on the node.
- `compatibility.required_glibc`: minimum glibc for Linux targets.
- `update_available`: true only when hash differs and compatibility passes.
- `sha256`: hash of the published Worker binary.
- `signature_algorithm`, `signature`, `signing_key_id`, `signing_public_key`: Worker update signature metadata. AgentGrid uses Ed25519 for signed updates.
- `signature_required`: when true, Worker must verify the update signature before replacing itself.

Worker update signing v1:

- Publish the Worker binary at `web/downloads/<target>/agentgrid-worker` or `agentgrid-worker.exe`.
- Publish `*.sha256` beside it for compatibility.
- Publish `agentgrid-worker(.exe).ed25519.sig` beside it when signing is enabled. The signature is base64 Ed25519 over the raw Worker binary bytes.
- Configure Hub with `AGENTGRID_WORKER_UPDATE_PUBLIC_KEY=<base64 ed25519 public key>` and optional `AGENTGRID_WORKER_UPDATE_KEY_ID`.
- Set `AGENTGRID_WORKER_UPDATE_SIGNATURE_REQUIRED=true` on Hub or start Worker with `--require-update-signature` to reject unsigned updates.

Known Linux compatibility targets:

- `linux-x86_64`: requires glibc 2.34.
- `linux-glibc-2.32-*`: requires glibc 2.32.
- `linux-glibc-2.34-*`: requires glibc 2.34.

## 22. Node Provisioning Plans

Create a standardized onboarding plan. The Hub does not store SSH passwords.

```http
POST /api/node-provisioning/plans
Content-Type: application/json
```

```json
{
  "node_id": "linux-worker-02",
  "node_name": "huarui 子节点",
  "ssh_host": "worker.example.com",
  "ssh_user": "root",
  "os": "linux",
  "arch": "x86_64",
  "hub_url": "https://hub.example.com/agentgrid/api",
  "notes": "credentials not stored"
}
```

List plans:

```http
GET /api/node-provisioning/plans
```

The plan contains human-readable install steps, a systemd service template, and the exact Worker startup command.

## 22.1 Hub Organization, Admin, And Node Authorization

AgentGrid Hub has a default organization and exactly one `super_admin`.
When the Hub has no super admin, the Web console shows a bootstrap page.
The first admin must create and save the account before normal operation.

Bootstrap status:

```http
GET /api/bootstrap
```

Create the only super admin:

```http
POST /api/bootstrap/admin
Content-Type: application/json
```

```json
{
  "email": "admin@example.com",
  "name": "超级管理员",
  "password": "change-me-strong"
}
```

Login:

```http
POST /api/auth/login
Content-Type: application/json
```

```json
{
  "email": "admin@example.com",
  "password": "change-me-strong"
}
```

The response contains a Hub session token:

```json
{
  "ok": true,
  "token": "ags_xxx",
  "user": {
    "metadata": { "id": "user_xxx" },
    "spec": { "email": "admin@example.com", "role": "super_admin" }
  }
}
```

Clients should send:

```http
Authorization: Bearer ags_xxx
```

Email registration uses verification code:

```http
POST /api/auth/register/request-code
Content-Type: application/json
```

```json
{ "email": "member@example.com" }
```

```http
POST /api/auth/register
Content-Type: application/json
```

```json
{
  "email": "member@example.com",
  "name": "成员",
  "password": "change-me-strong",
  "code": "123456"
}
```

System settings:

```http
GET /api/settings
POST /api/settings
```

Settings include:

- `hub_public_url`: public Hub URL shown to installers and users.
- `registration_enabled`: whether email self-registration is open.
- `smtp`: SMTP host, port, username, password, from address, enabled flag.

Node join authorization:

AgentGrid uses **AgentGrid Node Join Standard v1** for all Worker onboarding.
The design follows the same idea as the OAuth 2.0 Device Authorization Grant:
the machine can connect to the Hub, but the human approval happens on another
browser-capable device.

Workers should start with a stable node id, machine fingerprint, and join token.
The Hub records first contact as `pending`. Pending nodes may heartbeat, but they
cannot lease tasks. A super admin must approve the node once in the Web console.
After approval, the Hub binds:

- `node_id`
- `machine_fingerprint`
- `join_token_hash`

Why Linux does not need a browser:

- The installer prints the node id, token hint, and Hub approval URL.
- The Linux Worker only sends structured heartbeat and join data.
- The operator opens Hub on their own computer, logs in, checks the machine
  fingerprint, and approves the node.
- After approval the same Worker process can lease tasks without re-login.

Recommended headless Linux flow:

1. Admin creates a node provisioning plan in Hub.
2. Hub generates a one-time `agj_...` join token and systemd service template.
3. Operator runs the install commands on Linux.
4. Worker reports `auth_status = pending`.
5. Admin approves in Hub.
6. Hub stores only `join_token_hash`, token hint, and machine fingerprint.

Worker flags:

```bash
agentgrid-worker \
  --hub https://hub.example.com/agentgrid \
  --id zzh0610-windows \
  --name ZZH0610 \
  --join-token agj_xxx \
  --machine-fingerprint stable-machine-fingerprint
```

Environment fallback:

```bash
AGENTGRID_JOIN_TOKEN=agj_xxx
AG_JOIN_TOKEN=agj_xxx
```

Approve node join:

```http
POST /api/nodes/{node_id}/approve
Content-Type: application/json
```

```json
{ "actor": "super-admin" }
```

Scheduling rule:

- `auth_status = pending`: can heartbeat, cannot receive tasks.
- `auth_status = bound`: can receive tasks when online and eligible.
- `auth_status = legacy`: existing pre-auth nodes remain compatible until re-enrolled.

Node Join Contract:

```json
{
  "api_version": "agentgrid.node-join/v1",
  "kind": "NodeJoinRequest",
  "node_id": "linux-worker-02",
  "node_name": "huarui 子节点",
  "machine_fingerprint": "sha256:stable-machine-id",
  "join_token": "agj_xxx",
  "capabilities": ["command", "file", "git"],
  "system": {
    "os": "linux",
    "arch": "x86_64",
    "cpu_cores": 4,
    "memory_mb": 8192
  }
}
```

## 23. Workflow Templates

List reusable DAG templates:

```http
GET /api/workflow-templates
```

Create a template:

```http
POST /api/workflow-templates
Content-Type: application/json
```

```json
{
  "id": "http-probe",
  "name": "HTTP 探测流水线",
  "summary": "Probe URL then notify an AI employee.",
  "parameters": [
    { "name": "url", "label": "探测地址", "default": "https://httpbin.org/get" }
  ],
  "nodes": [
    {
      "id": "fetch",
      "title": "请求 ${url}",
      "payload": {
        "type": "http_request",
        "method": "GET",
        "url": "${url}",
        "headers": [],
        "body": null,
        "timeout_seconds": 30,
        "max_response_bytes": 65536
      },
      "labels": ["compute", "http_request"]
    }
  ]
}
```

Start a template:

```http
POST /api/workflow-templates/{template_id}/start
Content-Type: application/json
```

```json
{
  "actor": "architect-agent",
  "parameters": {
    "url": "https://example.com"
  }
}
```

The Hub replaces `${name}` placeholders recursively in strings, creates a Workflow, and starts it.

## 24. Scheduler Explanation

Preview scheduling for a task:

```http
GET /api/tasks/{task_id}/schedule-preview
```

Response includes:

- `selected_node_id`: current best node, or null.
- `requirements`: parsed task requirements from labels.
- `candidates`: every node with score, slot count, worker metadata, and reasons.
- `decision`: final scheduler choice and candidate score list.

Use this endpoint when an AI asks "why did this task run here?" or "why is this task not running?"

## 25. Unified Event Bus

Poll events:

```http
GET /api/events?limit=200
GET /api/events?type=task.leased&subject_id=task_xxx
```

Subscribe with SSE:

```http
GET /api/events/stream?limit=200
```

SSE event names:

- `events.snapshot`: latest event window.
- `events.error`: stream-side error.

Event bus v1 is backed by `audit_events`, so every important task, node, workflow, template, terminal, scheduler, and message action shares one read surface.

## 26. Workflow v1

Workflow is a DAG of AgentGrid tasks. The Hub creates a run for every node, releases entry nodes first, and automatically releases downstream nodes after all dependencies are done. If any node fails, the Workflow stops.

Workflow JSON:

```json
{
  "name": "跨节点主机巡检",
  "summary": "先查询 jia，再查询 huarui，最后发送协作消息。",
  "created_by": "architect-agent",
  "nodes": [
    {
      "id": "jia_hostname",
      "title": "查询 jia 主机名",
      "payload": {
        "type": "command",
        "program": "hostname",
        "args": [],
        "timeout_seconds": 30
      },
      "labels": ["compute", "command", "node:linux-worker-01"]
    },
    {
      "id": "huarui_hostname",
      "title": "查询 huarui 主机名",
      "depends_on": ["jia_hostname"],
      "payload": {
        "type": "command",
        "program": "hostname",
        "args": [],
        "timeout_seconds": 30
      },
      "labels": ["compute", "command", "node:linux-worker-02"]
    },
    {
      "id": "notify",
      "title": "发送完成通知",
      "depends_on": ["huarui_hostname"],
      "payload": {
        "type": "agent_message",
        "from": "workflow-engine",
        "to": ["architect-agent"],
        "message_type": "workflow.completed",
        "subject": "跨节点主机巡检完成",
        "summary": "jia 与 huarui 均已完成 hostname 检查。",
        "payload": {}
      },
      "labels": ["compute", "agentmessage"]
    }
  ]
}
```

CLI:

```bash
agentgrid workflows
agentgrid workflows get --id workflow_xxx
agentgrid workflows submit --file workflow.json
agentgrid workflows submit --file workflow.json --wait
agentgrid workflows start --id workflow_xxx --wait
agentgrid workflows watch --id workflow_xxx
agentgrid workflows cancel --id workflow_xxx --reason "不再需要执行"
```

API:

```http
POST /api/workflows
POST /api/workflows/{workflow_id}/start
GET /api/workflows/{workflow_id}
POST /api/workflows/{workflow_id}/cancel
```

Final Workflow result:

```json
{
  "type": "workflow_result",
  "workflow_id": "workflow_xxx",
  "done_count": 3,
  "completed_at": "2026-05-31T12:00:00Z",
  "steps": [
    {
      "workflow_node_id": "jia_hostname",
      "task_id": "task_xxx",
      "state": "done",
      "leased_by_node_id": "linux-worker-01",
      "result": {
        "type": "command_result",
        "exit_code": 0,
        "stdout": "hanfeihu\n"
      },
      "error": null
    }
  ]
}
```

Workflow v1 supports:

- Sequential and parallel task graphs.
- Dependency validation and cycle detection.
- Node, OS, group, tag, preference, and capability labels.
- HTTP, command, file, Git, Docker, browser, session, and AgentMessage payloads.
- Per-node task IDs, status, started/completed timestamps, result, and error.
- Result aggregation for AI clients.

## 27. Workflow Context References

Workflow nodes can reference completed upstream step data with `${...}` expressions. The Hub renders a node immediately before releasing it as a real task, so downstream nodes can consume upstream results.

Available context:

```json
{
  "workflow": {
    "id": "workflow_xxx"
  },
  "steps": {
    "read_hostname": {
      "task_id": "task_xxx",
      "state": "done",
      "leased_by_node_id": "linux-worker-01",
      "result": {
        "stdout": "hanfeihu\n",
        "exit_code": 0
      },
      "error": null
    }
  }
}
```

Example:

```json
{
  "id": "notify_hostname",
  "title": "通知主机名 ${steps.read_hostname.result.stdout}",
  "depends_on": ["read_hostname"],
  "payload": {
    "type": "agent_message",
    "from": "workflow-engine",
    "to": ["architect-agent"],
    "message_type": "workflow.context.demo",
    "subject": "工作流结果传递验证",
    "summary": "linux-worker-01 hostname 是：${steps.read_hostname.result.stdout}",
    "payload": {
      "source_task_id": "${steps.read_hostname.task_id}",
      "hostname": "${steps.read_hostname.result.stdout}",
      "executed_by": "${steps.read_hostname.leased_by_node_id}"
    }
  },
  "labels": ["compute", "agentmessage"]
}
```

Rules:

- References use dot paths, for example `${steps.build.result.stdout}`.
- Missing references fail the Workflow clearly with `workflow_template_render_failed`.
- Strings can contain one or more references.
- Object and array values are rendered recursively.
- This is intended for task chaining, AI report generation, and multi-step automation.

## 28. Node Operation Standard v1

Node Operation is the business-level wrapper for actions that mutate one concrete machine. It is different from generic cluster scheduling.

Examples:

- `software_install`
- `service_restart`
- `config_write`
- `host_command`
- `file_deploy`

Rules:

- Node Operation must include `target_node_id`.
- Hub must treat `target_node_id` as a hard placement constraint.
- The execution layer may use `command.run`, a Worker plugin, or a future native executor.
- The public business object should stay stable even if the executor changes.
- Mutating operations should write clear audit events and structured results.

Software install v1:

```bash
agentgrid submit-software-install \
  --node ZZH0610-windows \
  --name "ExampleApp" \
  --source-url "https://example.com/installer.exe" \
  --installer exe \
  --installer-arg "/quiet" \
  --installer-arg "/norestart" \
  --timeout-seconds 900 \
  --wait
```

Current implementation:

- Supports Windows `exe` and `msi` installers.
- Requires an explicit `--node`; software install is never a floating cluster task.
- Uses PowerShell and the existing command executor as the v1 execution backend.
- Optional `--sha256` verifies the downloaded installer before running it.
- Writes `operation:software_install` into task labels for audit and filtering.

Protocol shape:

```json
{
  "api_version": "agentgrid.operations/v1",
  "kind": "NodeOperation",
  "type": "software_install",
  "target_node_id": "ZZH0610-windows",
  "os": "windows",
  "software": {
    "name": "ExampleApp",
    "source_url": "https://example.com/installer.exe",
    "installer": "exe",
    "installer_args": ["/quiet", "/norestart"],
    "sha256": null
  }
}
```

## 28. Tool Registry v1

Tool Registry is the AI-facing capability catalog for the whole AgentGrid cluster. AI clients should query this catalog before submitting tasks, instead of guessing payload formats or node support.

API:

```http
GET /api/tools
GET /api/tools/{tool_id}
GET /api/tools/{tool_id}/nodes
```

CLI:

```bash
agentgrid tools
agentgrid tools get --id command.run
agentgrid tools nodes --id command.run
```

Tool object:

```json
{
  "id": "command.run",
  "name": "执行主机命令",
  "summary": "在被调度节点上执行 allowlist 内的系统命令。",
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
      "args": { "type": "array", "items": { "type": "string" } },
      "working_dir": { "type": ["string", "null"] },
      "timeout_seconds": { "type": "integer" }
    }
  },
  "output_schema": {
    "type": "object",
    "properties": {
      "exit_code": { "type": "integer" },
      "stdout": { "type": "string" },
      "stderr": { "type": "string" },
      "duration_ms": { "type": "integer" }
    }
  },
  "supported_nodes": ["hub-node", "linux-worker-01", "linux-worker-02"],
  "node_count": 3,
  "support_basis": "node_heartbeat_capabilities",
  "verification_status": "declared_unverified"
}
```

Built-in tools:

- `http.request`
- `command.run`
- `file.read`
- `file.write`
- `file.list`
- `git.status`
- `git.clone`
- `docker.run`
- `browser.fetch`
- `session.run`
- `agentmessage.send`
- dynamically registered node tools, for example `demo.hello`

Risk levels:

- `low`: collaboration or read-only metadata.
- `medium`: network, file read, browser fetch, repository operations.
- `high`: command execution, file write, Docker, long sessions.

AI client guidance:

- Query `/api/tools` before creating a new task or workflow.
- Use `labels` from the selected tool when creating an AgentTask.
- Use `input_schema` to generate payload.
- Check `node_count` and `/api/tools/{tool_id}/nodes` before selecting a specific node.
- If `requires_policy` is true, assume execution is constrained by Hub/Worker policy.
- `supported_nodes` is based on Worker heartbeat capabilities in v1. Treat `verification_status: declared_unverified` as "declared by node, not yet probed by Hub".

## 29. Node Tool Registration v1

Node Tool Registration lets every Worker node publish its own tool list. AgentGrid does not assume all machines have the same local software, plugins, hardware, accounts, or runtime adapters.

This is the standard path for heterogeneous nodes:

- A node registers one or more tools.
- The Hub merges node tools into `/api/tools` and `/api/agent-runtime/manifest`.
- AI clients submit by `tool_id` through Runtime API.
- The Hub leases the task only to nodes that registered that dynamic tool and are currently online.

REST API:

```http
POST /api/nodes/{node_id}/tools
GET /api/nodes/{node_id}/tools
GET /api/node-tools
GET /api/node-tools/{tool_id}
GET /api/tools/{tool_id}
```

CLI:

```bash
agentgrid node-tools
agentgrid node-tools get --id demo.hello
agentgrid node-tools node --node hub-node
agentgrid node-tools register --node hub-node --file node-tool.json
```

Registration file:

```json
{
  "tools": [
    {
      "tool_id": "demo.hello",
      "name": "Demo Hello Tool",
      "version": "0.1.0",
      "executor": "plugin:hello-plugin",
      "status": "available",
      "confidence": "declared",
      "input_schema": {
        "type": "object",
        "properties": {
          "name": { "type": "string" }
        }
      },
      "output_schema": { "type": "object" },
      "constraints": {},
      "labels": ["compute", "plugin", "tool:demo.hello"],
      "default_verify": {
        "rules": [
          {
            "path": "result.type",
            "op": "exists",
            "description": "动态工具必须回写结构化结果"
          }
        ]
      },
      "probe": {
        "enabled": true,
        "interval_seconds": 300,
        "timeout_seconds": 30,
        "payload": {
          "name": "AgentGrid"
        },
        "verify": {
          "rules": [
            {
              "path": "result.output.ok",
              "op": "eq",
              "value": true,
              "description": "插件 Probe 必须返回 ok=true"
            }
          ]
        }
      }
    }
  ]
}
```

Dynamic Runtime submit:

```bash
agentgrid runtime submit \
  --tool demo.hello \
  --payload '{"name":"AgentGrid"}' \
  --workbench sha256:091c56d7a82c696b759efbf8836b6f8e82cf507deeda35e170fc42134a2caece \
  --wait
```

Stable CLI payload input standard:

```bash
agentgrid runtime submit \
  --tool demo.hello \
  --payload-file payload.json \
  --workbench sha256:091c56d7a82c696b759efbf8836b6f8e82cf507deeda35e170fc42134a2caece \
  --wait
```

```bash
cat payload.json | agentgrid runtime submit \
  --tool demo.hello \
  --payload-stdin \
  --workbench sha256:091c56d7a82c696b759efbf8836b6f8e82cf507deeda35e170fc42134a2caece \
  --wait
```

```bash
agentgrid runtime submit \
  --tool demo.hello \
  --payload-base64 eyJuYW1lIjoiQWdlbnRHcmlkIn0= \
  --workbench sha256:091c56d7a82c696b759efbf8836b6f8e82cf507deeda35e170fc42134a2caece \
  --wait
```

`--payload-file`, `--payload-stdin`, and `--payload-base64` are designed for
Windows PowerShell, CI, and AI clients where inline JSON quoting is fragile.
The same payload input standard applies to `agentgrid jobs plan` and
`agentgrid jobs submit`. `agentgrid submit-plugin` uses the same pattern with
`--input-file`, `--input-stdin`, and `--input-base64`.

Dynamic task payload generated by Hub:

```json
{
  "type": "demo.hello",
  "tool_id": "demo.hello",
  "executor": "plugin:hello-plugin",
  "name": "AgentGrid"
}
```

Production-style node tool example: `audio.tts.clone`

`audio.tts.clone` is a business capability/tool contract for voice-clone text
to speech. The current `jia-node` implementation is backed by the
`index-tts-clone` Worker plugin and the node-local IndexTTS-2 service.

Tool boundary:

- Capability: `audio.tts`
- Tool: `audio.tts.clone`
- Plugin implementation: `index-tts-clone`
- Primary node today: `jia-node`
- Required input: `text`
- Optional voice input: `reference_audio_path` or `reference_audio_base64`
- Output: `audio_tts_clone_result` with generated wav path, mime type, bytes,
  duration, and audio evidence.

Example:

```bash
agentgrid runtime submit \
  --tool audio.tts.clone \
  --node jia-node \
  --title "Generate cloned voice audio" \
  --payload '{"text":"AgentGrid can schedule voice synthesis on jia-node.","timeout_seconds":600}' \
  --wait \
  --wait-timeout-seconds 900
```

Node-local generated audio is written under:

```text
/tmp/agentgrid-artifacts/audio.tts.clone/
```

The Worker plugin must write exactly one JSON object to stdout. Logs from
underlying clients, SDKs, model loaders, or web UI adapters must go to stderr,
otherwise the Worker cannot parse the plugin result as structured output.

Worker execution rule:

- `executor: "plugin:<plugin_id>"` is executed by the Worker plugin runtime.
- The plugin receives the payload as structured JSON.
- The result is written back as a normal AgentGrid structured task result.

Scheduling rule:

- Dynamic tools must include `tool:<tool_id>` labels.
- Offline, unknown, high-load, or unregistered nodes are skipped.
- If a task requires `demo.hello`, a node with only generic `plugin` capability is not enough; it must register `demo.hello`.
- If Probe state is `failed`, `unavailable`, or `unsupported`, the node will not receive that dynamic tool task.
- `verified` nodes are preferred over merely declared nodes.

## 30. Node Tool Probe v1

Node Tool Probe verifies dynamic tools registered by nodes. AgentGrid does not guess how to test a plugin; the registration must provide structured `probe.payload` and optional `probe.verify`.

Manual Probe API:

```http
POST /api/node-tools/probe
POST /api/node-tools/{tool_id}/probe
POST /api/node-tools/{tool_id}/nodes/{node_id}/probe
```

CLI:

```bash
agentgrid node-tools probe
agentgrid node-tools probe --id demo.hello
agentgrid node-tools probe --id demo.hello --node hub-node
```

Probe task generated by Hub:

```json
{
  "title": "Node Tool Probe demo.hello on hub-node",
  "created_by": "node-tool-probe-engine",
  "labels": [
    "compute",
    "plugin",
    "tool:demo.hello",
    "node:hub-node",
    "probe:demo.hello",
    "node_tool_probe"
  ],
  "inputs": [
    "{ \"type\":\"demo.hello\", \"tool_id\":\"demo.hello\", \"executor\":\"plugin:hello-plugin\", \"name\":\"AgentGrid\" }"
  ],
  "verify": {
    "rules": [
      {
        "path": "result.output.ok",
        "op": "eq",
        "value": true
      }
    ]
  }
}
```

Probe states:

- `declared_unverified`: registered but not verified.
- `pending`: Probe task has been submitted.
- `verified`: latest Probe passed.
- `failed`: latest Probe failed.
- `unsupported`: no Probe payload is configured.
- `expired`: previous verification is stale and should be refreshed.

Automatic Probe:

- Hub runs a lightweight scheduler every 60 seconds.
- It checks node tools with `probe.enabled != false`.
- `next_probe_at <= now` creates a normal low-priority Probe task.
- Probe interval defaults to 300 seconds and can be set with `probe.interval_seconds`.
- Probe results are written to both `tool_probes` and the node tool health fields.

## 31. Tool Probe v1

Tool Probe verifies whether a declared tool actually works on a node. Probe v1 creates normal AgentTask items with `probe:<tool_id>` labels, lets Worker execute them, and updates `tool_probes` after completion or failure.

API:

```http
GET /api/tools/probe-center
GET /api/tools/probes
POST /api/tools/probe
POST /api/tools/{tool_id}/probe
POST /api/tools/{tool_id}/nodes/{node_id}/probe
```

CLI:

```bash
agentgrid tools probe-center
agentgrid tools probes
agentgrid tools probe
agentgrid tools probe --id command.run
agentgrid tools probe --id command.run --node linux-worker-01
```

Probe Center response:

```json
{
  "api_version": "agentgrid.probe-center/v1",
  "kind": "ToolProbeCenter",
  "summary": {
    "readiness": "needs_probe",
    "tool_count": 18,
    "node_count": 6,
    "workbench_count": 4,
    "verified_edges": 5,
    "failed_edges": 0,
    "pending_edges": 2,
    "declared_unverified_edges": 12,
    "recommendations": [
      {
        "level": "info",
        "code": "declared_unverified",
        "message": "有工具只是节点声明可用，还没有运行时验证。建议在能力验证中心执行 Probe。"
      }
    ]
  },
  "tools": [],
  "workbenches": [],
  "recent_probes": []
}
```

Use Probe Center when an AI client needs one machine-readable view of:

- which tools exist;
- which workbenches and nodes can run them;
- which tool-node edges are verified, pending, failed, unsupported, or only declared;
- what the scheduler should trust.

Trust-aware scheduling rules:

- If any eligible node is `verified` for the required tool, Hub applies a
  verified-only gate and chooses only from verified candidates.
- `failed` tool-node edges are skipped for normal scheduling.
- `declared_unverified` and `expired` edges are allowed only when no verified
  edge exists, and receive a score penalty.
- Verified records expire after 24 hours and are automatically re-probed.
- Failed records remain untrusted, but Hub automatically retries them after a
  cooldown window so fixed nodes can regain verified status.
- Pending probe records are deduplicated.

Remediation Center:

```bash
agentgrid tools remediation-center
agentgrid tools remediation-runbook --id rem_docker_run_jia_node
agentgrid tools remediation-action --id rem_docker_run_jia_node
```

API:

```text
GET /api/tools/remediation-center
GET /api/tools/remediations/{remediation_id}/runbook
POST /api/tools/remediations/{remediation_id}/actions
```

Use it after Probe Center reports `attention_required`. It returns structured
repair items with `tool_id`, `node_id`, `probe_state`, normalized diagnosis,
recommended action, repair steps, and the exact CLI/API command to re-probe.
It is intentionally read-only in v1: policy changes, Docker enablement, and
plugin installation still require an explicit operator action.

`remediation-action` turns a remediation item into an auditable action. In v1,
Hub supports safe actions such as `probe_again` and `check_dependency`; higher
risk actions create review tasks instead of mutating Worker policy directly.

Remediation Result Classifier:

- Read `spec.diagnosis.code` for the latest product-level conclusion.
- Read `spec.diagnosis.next_action` for the recommended next step.
- Read `status.last_action_task` to open the task that produced the diagnosis.

Common diagnosis codes:

- `not_checked`: no remediation task has run yet.
- `check_running`: remediation task is waiting or running.
- `dependency_missing`: dependency or command is missing.
- `policy_blocked`: Worker policy blocked execution.
- `service_not_running`: dependency exists but service is stopped.
- `path_missing`: file or directory path is unavailable.
- `probe_ready`: dependency check passed; run Probe again.
- `check_failed`: check failed without a more specific class.
- `unknown`: insufficient evidence.

Remediation Runbook:

- Read `spec.runbook` from each remediation item.
- Use `agentgrid tools remediation-runbook --id rem_xxx` to print one runbook.
- `spec.runbook.current_step_id` tells the current step.
- `spec.runbook.requires_operator=true` means Hub must not mutate the machine
  silently; an operator or explicitly authorized AI action must perform the
  state-changing step.
- `spec.runbook.steps[]` contains normalized phases: diagnostic,
  operator_action, and verification.
- `spec.runbook.commands.probe_again` and `spec.runbook.api.probe_again` are
  the standard verification entrypoints after repair.

Probe states:

- `declared_unverified`: node heartbeat says it supports the capability, but Hub has not verified it.
- `pending`: Probe task has been created and is waiting/running.
- `verified`: Probe task completed successfully.
- `failed`: Probe task failed.
- `unsupported`: no lightweight Probe is defined for this tool.
- `expired`: reserved for future automatic expiry handling.

Example result:

```json
{
  "kind": "ToolProbe",
  "metadata": {
    "tool_id": "command.run",
    "node_id": "linux-worker-01",
    "task_id": "task_xxx"
  },
  "spec": {
    "support_basis": "runtime_probe"
  },
  "status": {
    "state": "verified",
    "completed_at": "2026-05-31T13:30:00Z",
    "expires_at": "2026-06-01T13:30:00Z",
    "result": {
      "type": "command_result",
      "exit_code": 0,
      "stdout": "hanfeihu\n"
    }
  }
}
```

## 32. Agent Runtime API v1

Agent Runtime API is the stable AI-facing integration layer. External AI clients should use this layer instead of hand-building raw AgentTask objects.

Runtime manifest:

```http
GET /api/agent-runtime/manifest
```

The manifest returns:

- Runtime identity and version
- Supported protocols
- Feature capabilities
- ToolContract list
- Task submit schema
- Result schema
- AI-ready examples

Submit a tool task:

```http
POST /api/agent-runtime/tasks
Content-Type: application/json
```

```json
{
  "tool_id": "command.run",
  "title": "hostname",
  "payload": {
    "type": "command",
    "program": "hostname",
    "args": [],
    "working_dir": null,
    "timeout_seconds": 30
  },
  "verify": {
    "presets": ["command.exit_zero"]
  }
}
```

If `verify` is omitted, the Hub uses the selected ToolContract default verification rule.

Read task snapshot:

```http
GET /api/agent-runtime/tasks/{task_id}
```

Watch task events:

```http
GET /api/agent-runtime/tasks/{task_id}/events
Accept: text/event-stream
```

CLI:

```bash
agentgrid runtime manifest
```

```bash
agentgrid runtime submit \
  --tool command.run \
  --payload '{"type":"command","program":"hostname","args":[],"working_dir":null,"timeout_seconds":30}' \
  --wait
```

```bash
agentgrid runtime get --id task_xxx
```

ToolContract fields:

- `contract_version`: current value is `agentgrid.tool/v1`
- `input_schema`: JSON Schema-style payload shape
- `output_schema`: expected structured result shape
- `default_verify`: Hub-side result verification used when clients do not override it
- `standard_outputs`: normalized output names for AI planning
- `agent_runtime_submit_example`: ready-to-submit request example

AI client guidance:

- Always call `/api/agent-runtime/manifest` first.
- Select a `tool_id` from the returned ToolContract list.
- Build `payload` according to `input_schema`.
- Use `node_id`, `os`, `group`, `prefer_node_id`, or `avoid_node_id` only when placement matters.
- Prefer default verification unless the task has a specific success condition.
- Read `item.result.verification` from Runtime snapshots.
- Subscribe to SSE when the task may run longer than a few seconds.

Tool Registry responses include per-node Probe status:

```json
{
  "id": "linux-worker-01",
  "verification_status": "verified",
  "support_basis": "runtime_probe",
  "probe": {
    "kind": "ToolProbe"
  }
}
```

## 33. Trust-Aware Scheduler v1

The Hub scheduler uses Tool Probe state when selecting nodes. Resource score is still important, but the final decision is adjusted by trust.

Trust multipliers:

- `verified`: `0.72`, preferred.
- `pending`: `1.12`, slightly penalized.
- `declared_unverified`: `1.35`, allowed but penalized.
- `expired`: `1.45`, allowed but strongly penalized.
- `unsupported`: `1.75`, heavily penalized.
- `failed`: skipped unless the task explicitly requires that node.

Example:

```bash
agentgrid tasks explain --id task_xxx
```

The response includes:

```json
{
  "candidates": [
    {
      "node_id": "linux-worker-01",
      "score": 8.3,
      "base_resource_score": 11.5,
      "trust": {
        "tool_id": "command.run",
        "state": "verified",
        "support_basis": "runtime_probe",
        "multiplier": 0.72,
        "reason": "command.run runtime probe verified"
      },
      "reasons": [
        "满足任务要求，可参与调度",
        "可信调度：command.run runtime probe verified"
      ]
    }
  ]
}
```

This answers the production question: "Why is AgentGrid confident this node can run this task?"

## 34. AgentGrid MCP Server v1

MCP Server lets AI clients call AgentGrid through a standard tool interface over stdio.

Binary:

```bash
agentgrid-mcp --hub https://hub.example.com/agentgrid
```

Default Hub URL is already compiled in, so most clients can use:

```bash
agentgrid-mcp
```

MCP tools:

- `agentgrid_runtime_manifest`: read AI Runtime manifest and ToolContracts.
- `agentgrid_list_tools`: list tool registry.
- `agentgrid_list_nodes`: list cluster nodes and resource state.
- `agentgrid_list_task_templates`: list task template store.
- `agentgrid_start_task_template`: start a template by `template_id` and parameters.
- `agentgrid_submit_task`: submit any Runtime task by `tool_id` and `payload`.
- `agentgrid_get_task`: read task snapshot by `task_id`.
- `agentgrid_run_command`: submit a command task.
- `agentgrid_run_plugin`: submit a Worker plugin task.
- `agentgrid_list_webhooks`: list callback subscriptions.
- `agentgrid_create_webhook`: create callback subscription.

Minimal MCP request example:

```json
{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}
```

## 35. AgentGrid SDK v1

SDKs are thin wrappers around the same Agent Runtime API. They should not invent a second protocol.

Rust:

```rust
use agentgrid_sdk::AgentGridClient;

let client = AgentGridClient::default_hub();
let result = client.submit_command("hostname", vec![], Some("hub-node".into()), None, None)?;
```

Node:

```js
import { AgentGridClient } from './sdk/node/index.js';

const client = new AgentGridClient();
const workbenches = await client.workbenches();
const result = await client.runCommand({ program: 'hostname', workbenchId: 'sha256:...' });
```

Python:

```python
from agentgrid_sdk import AgentGridClient

client = AgentGridClient()
workbenches = client.workbenches()
result = client.run_command('hostname', workbench_id='sha256:...')
```

Mobile console clients:

- iOS Swift Package: `sdk/mobile/ios/agentgrid-mobile-sdk-swift`
- Android Kotlin module: `sdk/mobile/android/agentgrid-mobile-sdk-kotlin`

Mobile SDKs are console clients only. They can view the cluster, submit
structured tasks, poll task status, read execution records, and display
artifacts. They are not Workers and do not execute tasks on the phone.

iOS:

```swift
import AgentGridMobileSDK

let client = AgentGridMobileClient()
let workbenches = try await client.workbenches()
let timeline = try await client.workbenchTimeline("sha256:...")
let record = try await client.executionRecord(taskID: "task_xxx")
```

Android:

```kotlin
val client = AgentGridMobileClient()
val workbenches = client.workbenches()
val timeline = client.workbenchTimeline("sha256:...")
val record = client.executionRecord("task_xxx")
```

Common SDK methods:

- `runtime_manifest()` / `runtimeManifest()`
- `tools()`
- `nodes()`
- `workbenches()` / `workbench()` / `workbench_timeline()` / `workbenchTimeline()`
- `submit_task()` / `submitTask()`
- `get_task()` / `getTask()`
- `run_command()` / `runCommand()`
- `run_plugin()` / `runPlugin()`
- `task_templates()` / `taskTemplates()`
- `start_task_template()` / `startTaskTemplate()`
- `webhooks()`
- `create_webhook()` / `createWebhook()`
- `webhook_deliveries()` / `webhookDeliveries()`

Mobile SDK methods:

- `health()`
- `runtimeStandard()`
- `mobileSdkStandard()`
- `workbenches()`
- `workbench(workbenchID/workbenchId)`
- `workbenchTimeline(workbenchID/workbenchId)`
- `devices()`
- `evidenceStandard()`
- `nodes()`
- `tools()`
- `submitTask(request)`
- `runCommand(program, args, nodeID/nodeId, workbenchID/workbenchId, title)`
- `runPlugin(pluginID/pluginId, action, input, nodeID/nodeId, workbenchID/workbenchId, title)`
- `getTask(taskId)`
- `taskEvents(taskId)`
- `executionRecord(taskId)`
- `artifacts()`
- `artifactDownloadUrl(artifactId)`
- `taskTemplates()`
- `startTaskTemplate(templateId, request)`
- `localServices()`
- `createBridgeSession(nodeId, serviceId)`
- `bridgeWebSocketUrl(sessionId, token)`
- `listPortBridges()`
- `createPortBridge(sourceNodeId, targetNodeId, targetPort, ...)`
- `getPortBridge(portBridgeId)`
- `closePortBridge(portBridgeId)`

## 36. Task Template Store v1

Task templates are reusable, parameterized Runtime task definitions stored in Hub.

List templates:

```bash
agentgrid task-templates list
```

Get one template:

```bash
agentgrid task-templates get --id server.hostname
```

Start a template:

```bash
agentgrid task-templates start \
  --id http.healthcheck \
  --param url=https://hub.example.com/agentgrid/api/health \
  --wait
```

REST API:

```http
GET /api/task-templates
GET /api/task-templates/{id}
POST /api/task-templates/{id}/start
```

Start request:

```json
{
  "title": "检查中心服务器健康",
  "parameters": {
    "url": "https://hub.example.com/agentgrid/api/health"
  },
  "node_id": "hub-node",
  "created_by": "agent-runtime"
}
```

Template object fields:

- `metadata.id`: stable template id, for example `server.hostname`.
- `spec.name`: human readable name.
- `spec.category`: server, network, browser, source_control, collaboration.
- `spec.tool_id`: ToolContract id.
- `spec.payload`: Runtime payload with `{{parameter}}` placeholders.
- `spec.parameters`: parameter schema for humans and AI.
- `spec.verify`: default result verification.

## 37. Worker Plugin System v1

Worker plugins extend node capabilities without hardcoding every task type into Worker.

Plugin task payload:

```json
{
  "type": "plugin",
  "plugin_id": "hello-plugin",
  "action": "run",
  "input": { "name": "AgentGrid" },
  "timeout_seconds": 60
}
```

CLI:

```bash
agentgrid submit-plugin \
  --plugin-id hello-plugin \
  --action run \
  --input '{"name":"AgentGrid"}' \
  --node hub-node
```

Runtime API:

```json
{
  "tool_id": "plugin.run",
  "title": "hello plugin",
  "payload": {
    "type": "plugin",
    "plugin_id": "hello-plugin",
    "action": "run",
    "input": { "name": "AgentGrid" },
    "timeout_seconds": 60
  }
}
```

Worker lookup rule:

- Read plugin directory from `AGENTGRID_PLUGIN_DIR`.
- Default plugin directory is `/opt/agentgrid/plugins`.
- Executable path is `{plugin_dir}/{plugin_id}`.
- Worker passes `action` as the first command argument.
- Worker writes a JSON request to plugin stdin.
- Plugin must write JSON to stdout.

Plugin stdin contract:

```json
{
  "plugin_id": "hello-plugin",
  "action": "run",
  "input": { "name": "AgentGrid" }
}
```

Plugin stdout contract:

```json
{
  "ok": true,
  "message": "hello AgentGrid",
  "data": {}
}
```

Task result shape:

```json
{
  "type": "plugin_result",
  "plugin_id": "hello-plugin",
  "action": "run",
  "output": {},
  "duration_ms": 12
}
```

## 38. Webhook Callback v1

Webhooks notify external systems when tasks finish or fail.

Create webhook:

```bash
agentgrid webhooks create \
  --name ci-callback \
  --url https://example.com/agentgrid/webhook \
  --event task.completed \
  --event task.failed
```

List webhooks:

```bash
agentgrid webhooks list
```

Delivery records:

```bash
agentgrid webhooks deliveries
```

REST API:

```http
GET /api/webhooks
POST /api/webhooks
DELETE /api/webhooks/{id}
GET /api/webhooks/deliveries
```

Create request:

```json
{
  "name": "CI callback",
  "url": "https://example.com/agentgrid/webhook",
  "events": ["task.completed", "task.failed"],
  "secret": "optional-shared-secret",
  "enabled": true
}
```

Delivery payload:

```json
{
  "api_version": "agentmessage.io/v1",
  "kind": "WebhookEvent",
  "delivery_id": "whdel_xxx",
  "event_type": "task.completed",
  "subject_id": "task_xxx",
  "created_at": "2026-05-31T00:00:00Z",
  "payload": {
    "task_id": "task_xxx",
    "node_id": "hub-node",
    "result": {}
  }
}
```

Delivery headers:

- `content-type: application/json`
- `x-agentgrid-event`: event type.
- `x-agentgrid-delivery`: unique delivery id.
- `x-agentgrid-signature`: `sha256=<hmac>` when `secret` is configured.

Signature standard:

```text
hex(HMAC_SHA256(secret, raw_request_body))
```

The Webhook API does not return the secret. It only returns `has_secret`.

## 39. Workflow Orchestrator v2

Workflow v2 lets AgentGrid run a directed acyclic graph of tasks. Each node declares its payload, dependencies, routing labels, and failure policy.

Submit a workflow:

```bash
agentgrid workflows submit --file workflow.json --wait --timeout-seconds 900
```

Read a workflow:

```bash
agentgrid workflows get --id workflow_xxx
```

Minimal workflow:

```json
{
  "name": "check server and report",
  "summary": "Run hostname, then run uptime.",
  "nodes": [
    {
      "id": "hostname",
      "title": "hostname",
      "payload": {
        "type": "command",
        "program": "hostname",
        "args": [],
        "timeout_seconds": 30
      },
      "labels": ["compute", "command"]
    },
    {
      "id": "uptime",
      "title": "uptime",
      "depends_on": ["hostname"],
      "payload": {
        "type": "command",
        "program": "uptime",
        "args": [],
        "timeout_seconds": 30
      },
      "labels": ["compute", "command"]
    }
  ]
}
```

Node failure policy:

- `on_failure: "fail_workflow"` is the default. If the node fails, the workflow fails.
- `on_failure: "continue"` marks the failed node run as `skipped` and releases downstream nodes.
- `optional: true` is equivalent to allowing the workflow to continue when that node fails.
- A dependency is satisfied when its upstream run is `done` or `skipped`.

Optional node example:

```json
{
  "id": "best_effort_probe",
  "title": "best effort probe",
  "payload": {
    "type": "command",
    "program": "sh",
    "args": ["-lc", "exit 2"],
    "timeout_seconds": 30
  },
  "labels": ["compute", "command"],
  "optional": true,
  "on_failure": "continue"
}
```

Workflow terminal states:

- `done`: every node run is `done` or `skipped`.
- `failed`: at least one required node failed, stopped, or was cancelled.
- `cancelled`: workflow was cancelled by operator.

The workflow result includes ordered steps, `done_count`, and `skipped_count`.

## 40. Execution Record Standard v1

Execution Record is the complete archive for one task or workflow. It is designed for humans, AI agents, audits, callbacks, and later replay/debugging.

Task record:

```bash
agentgrid records task --id task_xxx
```

Workflow record:

```bash
agentgrid records workflow --id workflow_xxx
```

REST API:

```http
GET /api/execution-records/tasks/{task_id}
GET /api/execution-records/workflows/{workflow_id}
```

Task record shape:

```json
{
  "api_version": "agentmessage.io/v1",
  "kind": "ExecutionRecord",
  "record_type": "task",
  "task_id": "task_xxx",
  "generated_at": "2026-05-31T00:00:00Z",
  "summary": {},
  "input": {
    "raw": [],
    "payloads": [],
    "labels": [],
    "acceptance_criteria": [],
    "verify": null
  },
  "schedule": {},
  "execution": {
    "result": {},
    "error": null,
    "verification": null,
    "logs": [],
    "artifacts": []
  },
  "notifications": {
    "webhook_deliveries": []
  },
  "audit": [],
  "raw": {}
}
```

Workflow record shape:

```json
{
  "api_version": "agentmessage.io/v1",
  "kind": "ExecutionRecord",
  "record_type": "workflow",
  "workflow_id": "workflow_xxx",
  "summary": {
    "state": "done",
    "progress": 100,
    "done_count": 2,
    "skipped_count": 1,
    "failed_count": 0
  },
  "definition": {
    "inputs": {},
    "nodes": []
  },
  "runs": [],
  "tasks": [],
  "result": {},
  "error": null,
  "notifications": {
    "webhook_deliveries": []
  },
  "audit": [],
  "raw": {}
}
```

The Web console exposes the same archive under `执行档案`.

## 41. AgentGrid Runtime Standard v1

AgentGrid Runtime Standard v1 is the machine-readable contract for AI clients, CLI clients, SDKs, and human operators.

Boundary:

- AgentGrid accepts structured JSON tasks.
- AgentGrid validates contracts, schedules resources, executes through Workers, records audit, and returns structured results.
- AgentGrid does not perform natural language understanding.
- AgentGrid does not decide model reasoning strategy.
- Authorization design is intentionally outside this standard version.

Main endpoint:

```http
GET /api/runtime-standard
```

Sub-standards:

```http
GET /api/runtime-standard/tool-contracts
GET /api/runtime-standard/capabilities
GET /api/runtime-standard/state-machine
GET /api/runtime-standard/workflow-template
GET /api/runtime-standard/result-report
GET /api/runtime-standard/workbench
GET /api/runtime-standard/devices
GET /api/runtime-standard/evidence
GET /api/runtime-standard/runbook
GET /api/runtime-standard/mobile-sdk
GET /api/runtime-standard/plugin-runtime
GET /api/runtime-standard/capability-graph
GET /api/runtime-standard/execution-contract
GET /api/runtime-standard/evidence-pipeline
GET /api/runtime-standard/probe-engine
GET /api/runtime-standard/placement-engine
GET /api/runtime-standard/task-intent
GET /api/runtime-standard/artifact-store
GET /api/runtime-standard/event-timeline
```

CLI:

```bash
agentgrid standard
agentgrid standard tool-contracts
agentgrid standard capabilities
agentgrid standard state-machine
agentgrid standard workflow-template
agentgrid standard result-report
agentgrid standard workbench
agentgrid standard devices
agentgrid standard evidence
agentgrid standard runbook
agentgrid standard mobile-sdk
agentgrid standard plugin-runtime
agentgrid standard capability-graph
agentgrid standard execution-contract
agentgrid standard evidence-pipeline
agentgrid standard probe-engine
agentgrid standard placement-engine
agentgrid standard task-intent
agentgrid standard artifact-store
agentgrid standard event-timeline
```

Runtime Standard includes:

- `ToolContract`: machine-readable input/output schema for every callable tool.
- `CapabilityRegistry`: cluster capability discovery mapped to online nodes.
- `WorkbenchStandard`: real machines and stations AI can operate.
- `DeviceStandard`: desktop, browser, filesystem, serial, flasher, test rig, plugin runtime, and future devices.
- `EvidenceStandard`: screenshot, log, serial output, file artifact, report, and operation timeline rules.
- `RunbookStandard`: structured procedures for hardware bench, desktop bench, and probe-and-use flows.
- `MobileSdkStandard`: iOS/Android console-client contract.
- `PluginRuntimeStandard`: node-local plugin declaration, installation, version, dependency, health-check, risk, and result contract.
- `CapabilityGraphStandard`: nodes, devices, tools, plugins, evidence, and task-intent relationships.
- `ExecutionContractStandard`: normalized input, output, error, timeout, retry, recovery, artifact, and audit contract.
- `EvidencePipelineStandard`: screenshot, log, file, report, serial, and timeline evidence processing.
- `NodeCapabilityProbeEngineStandard`: regular/manual validation of declared node tools and capabilities.
- `PlacementEngineStandard`: hard/soft scheduling constraints and decision record rules.
- `TaskIntentSchemaStandard`: structured JSON intent produced by AI clients before AgentGrid scheduling.
- `ArtifactStoreStandard`: artifact metadata, preview, hash, retention, and download contract.
- `EventTimelineStandard`: shared event stream for Web, Mobile, Webhook, MCP, and audit consumers.
- `CapabilityMarketplaceStandard`: AI-facing catalog of currently callable node capabilities.
- `TaskStateMachine`: legal task states and transitions.
- `WorkflowTemplateStandard`: reusable DAG template contract.
- `ResultReportStandard`: structured report rules for task, workflow, and cluster diagnostics.
- `ExecutionRecordStandard`: complete audit/archive contract.

AI client rule:

```text
Natural language -> AI client/model -> structured AgentGrid JSON -> AgentGrid Runtime
```

AgentGrid only receives the structured JSON side of that pipeline.

Example:

```bash
agentgrid runtime submit \
  --tool command.run \
  --payload '{"type":"command","program":"hostname","args":[],"working_dir":null,"timeout_seconds":30}' \
  --wait
```

The AI client should first read:

```bash
agentgrid standard tool-contracts
```

Then select a `tool_id`, produce a payload matching `input_schema`, and submit through Runtime.

Core standard quick commands:

```bash
agentgrid standard capability-graph
agentgrid standard execution-contract
agentgrid standard evidence-pipeline
agentgrid standard probe-engine
agentgrid standard placement-engine
agentgrid standard task-intent
agentgrid standard artifact-store
agentgrid standard event-timeline
agentgrid standard plugin-runtime
```

Plugin Runtime quick check:

```bash
agentgrid standard plugin-runtime
```

Plugin Runtime v1 is the standard for node-local tool extension packages. It
separates the implementation package from the AI-facing tool contract:

- `plugin_id`: the installed package, for example `agentgrid-plugin-document-parser`.
- `tool_id`: the callable capability, for example `document.parse`.
- `executor`: always shaped as `plugin:<plugin_id>` for plugin-backed tools.
- `manifest`: package identity, version, supported platforms, dependencies, entrypoint, tools, probe, and risk.
- `probe`: a real health-check task the Hub can run before trusting the tool.
- `risk`: `low`, `medium`, or `high`; the scheduler uses it with Probe and Capability Graph scoring.

Scheduler rule:

```text
eligible nodes -> resource score -> probe trust -> plugin risk -> capability graph fit -> selected node
```

Artifact Store v2 rule:

```text
task result -> normalized artifact -> sha256/content type/preview/retention/tool id -> Web/Mobile/MCP display
```

Task Intent example:

```json
{
  "intent_type": "document.parse",
  "title": "Parse uploaded contract",
  "tool_id": "document.parse",
  "placement": {
    "required_capabilities": ["plugin"]
  },
  "payload": {
    "type": "document",
    "file_artifact_id": "artifact_xxx",
    "extract_mode": "text"
  },
  "evidence": ["file_artifact", "test_report"]
}
```

AgentGrid does not parse natural language. An AI client may translate natural
language into Task Intent JSON, then AgentGrid validates contracts, chooses
placement, executes through Workers, records evidence, and emits timeline
events.

## 42. Worker Install

Linux, macOS, and Windows nodes run `agentgrid-worker` as a background service. The CLI is only for submitting tasks; the Worker is what keeps a node online.

Windows one-line install:

Run PowerShell as Administrator:

```powershell
irm "https://hub.example.com/agentgrid/install/windows.ps1" | iex
```

Optional node identity:

```powershell
$env:AG_NODE_ID="office-pc-01"
$env:AG_NODE_NAME="Office PC 01"
$env:AG_MAX_JOBS="4"
$env:AGENTGRID_JOIN_TOKEN="agj_xxx"
irm "https://hub.example.com/agentgrid/install/windows.ps1" | iex
```

After first heartbeat, the node appears as `pending` in the Web console. A Hub
super admin must approve it once before it can receive command, file, desktop,
plugin, or job tasks.

The bootstrap script downloads:

```text
https://hub.example.com/agentgrid/api/worker/download/windows-x86_64
```

It installs `agentgrid-worker.exe` under:

```text
C:\Program Files\AgentGridWorker
```

It also creates a machine-level tool directory and adds it to the machine `PATH`:

```text
C:\Program Files\AgentGridTools\bin
```

Windows workers should install shared command-line tools into machine-visible paths. For example, if a tool such as `codex`, `node`, `git`, or `python` is installed only under an interactive user's profile, the Worker may not see it because the Worker runs as `SYSTEM`.

The Windows bootstrap creates an automatic Scheduled Task named `AgentGridWorker`:

```powershell
Get-ScheduledTask AgentGridWorker
```

Runtime account:

```text
NT AUTHORITY\SYSTEM
RunLevel: Highest
```

This is the highest local service identity on Windows. If a command is still not found, it is usually a machine `PATH` / install-location issue, not a lack-of-permission issue.

Logs are written under:

```text
C:\Program Files\AgentGridWorker\logs
```

### Windows Worker vs Desktop Helper Boundary

Windows services run in a non-interactive `SYSTEM` session. That is good for background commands, file operations, software install, and long-running jobs, but it cannot reliably capture or control the real logged-in user's desktop.

Use this boundary:

| Node | Runtime | Use For | Do Not Use For |
| --- | --- | --- | --- |
| Normal Worker `<node_id>` | Background service / `NT AUTHORITY\SYSTEM` | command, file, service, software install, Git, Docker, session, plugin | visible desktop screenshot/click/type |
| Desktop Helper `<node_id>-desktop` | Logged-in Windows user session | screenshot, click, type text, key press, foreground app control | service install, background maintenance |

Product view:

- Treat `<node_id>` and `<node_id>-desktop` as two channels of one physical computer.
- The Hub scheduler enforces the channel boundary: background tasks require the Worker channel; visible desktop tasks require the Desktop Helper channel.
- Web and mobile consoles should show one physical computer with channel states, not two unrelated machines.

For screenshot, click, type, and key desktop control, install the optional Desktop Helper from an elevated PowerShell while the target Windows user is logged in:

```powershell
$env:AG_DESKTOP_HELPER="1"
irm "https://hub.example.com/agentgrid/install/windows.ps1" | iex
```

This keeps the normal background Worker as:

```text
<node_id>
Run account: NT AUTHORITY\SYSTEM
Capabilities: http, command, file, git, docker, browser, session, agentmessage, plugin
```

And starts a second login-session Worker:

```text
<node_id>-desktop
Run account: the logged-in Windows user
Capabilities: desktop
```

Submit a screenshot task to the desktop helper node:

```bash
agentgrid submit-desktop-screenshot \
  --node ZZH0610-windows-desktop \
  --wait
```

Submit a screenshot task to a physical computer/workbench. Hub chooses the
`desktop` channel automatically:

```bash
agentgrid submit-desktop-screenshot \
  --workbench sha256:091c56d7a82c696b759efbf8836b6f8e82cf507deeda35e170fc42134a2caece \
  --wait
```

Optional output path on the Windows node:

```bash
agentgrid submit-desktop-screenshot \
  --node ZZH0610-windows-desktop \
  --path "C:\\Users\\Public\\Pictures\\agentgrid-screen.png" \
  --wait
```

Click the visible desktop:

```bash
agentgrid submit-desktop-click \
  --node ZZH0610-windows-desktop \
  --x 100 \
  --y 100 \
  --button left \
  --wait
```

Type text into the current foreground window:

```bash
agentgrid submit-desktop-type-text \
  --node ZZH0610-windows-desktop \
  --text "hello from AgentGrid" \
  --wait
```

Send a key or shortcut:

```bash
agentgrid submit-desktop-key \
  --node ZZH0610-windows-desktop \
  --key ESC \
  --wait
```

```bash
agentgrid submit-desktop-key \
  --node ZZH0610-windows-desktop \
  --modifier ctrl \
  --key L \
  --wait
```

If the task is sent to the normal `SYSTEM` node, Windows may report a screen but fail during capture/control because that session is not the user's interactive desktop.

Desktop task detail in the Hub console should show:

- who submitted the task
- why Hub scheduled it to that node
- the structured desktop operation
- screenshot artifacts and operation timeline
- failure reason when the operation fails

## 43. Job Runtime v1

Job Runtime is for long-running or recoverable work. It adds Job, Attempt, and Checkpoint above normal AgentTask execution.

Core model:

- `Job`: user intent and recovery policy.
- `JobAttempt`: one execution try on one node through a normal AgentTask.
- `JobCheckpoint`: recoverable progress reported by Worker or client.
- `Lease`: existing task lease mechanism.
- `Event Ingress`: structured events pushed into Hub for a job or node.

Job request fields:

| Field | Required | Description |
| --- | --- | --- |
| `title` | yes | Human-readable Job title. |
| `tool_id` | yes | Tool contract id, such as `command.run` or `http.request`. |
| `payload` | yes | Tool payload. Templates are allowed in string values. |
| `placement` | no | Node placement constraints, such as `node_id` or `os`. |
| `strategy` | no | `single` or `sharded` execution strategy. |
| `partition` | no | `none`, `items`, or `range` partition plan. |
| `reduce` | no | Final aggregation strategy. |
| `retry_policy` | no | Retry and reschedule rules. |
| `checkpoint_policy` | no | Checkpoint behavior. |
| `idempotency` | no | External idempotency metadata. |

Retry and reschedule contract:

```json
{
  "retry_policy": {
    "max_attempts": 3,
    "on_node_lost": "reschedule",
    "on_process_failed": "reschedule_if_idempotent"
  },
  "checkpoint_policy": {
    "enabled": true,
    "mode": "worker_reported"
  },
  "idempotency": {
    "key": "stable-external-key",
    "mode": "idempotent"
  }
}
```

Decision rules:

| Failure reason | Policy | Reschedule condition |
| --- | --- | --- |
| `node_lost` | `reschedule` | Reschedule while `attempts_so_far < max_attempts`. Does not require idempotency because the previous attempt is treated as lost. |
| `node_lost` | `fail` | Do not reschedule. Mark Job or shard failed. |
| `process_failed` | `reschedule_if_idempotent` | Reschedule only when `idempotency.key` exists or `idempotency.mode` is `idempotent` / `external_exactly_once`. |
| `process_failed` | `fail` | Do not reschedule. Mark Job or shard failed. |

Machine-readable contract:

```http
GET /api/jobs/reliability
```

The response includes `retry_reschedule_contract`.

Job dry-run also returns request-specific `retry_reschedule_contract`:

```http
POST /api/jobs/plan
```

Important:

- `max_attempts` is clamped to `1..20`.
- `process_failed` without idempotency does not auto-retry under the default policy.
- `node_lost` is safe to reschedule by default, but the next attempt is still at-least-once.
- Checkpoint-aware tools should write richer checkpoints so the next attempt can resume instead of restarting from zero.

Strategy JSON:

```json
{
  "type": "sharded",
  "shard_count": 4,
  "max_parallelism": 2,
  "payload_mode": "inject_shard"
}
```

Items partition JSON:

```json
{
  "type": "items",
  "items": ["a", "b", "c", "d"]
}
```

Range partition JSON:

```json
{
  "type": "range",
  "start": 0,
  "end": 100,
  "step": 1
}
```

Reduce JSON:

```json
{
  "type": "json_array"
}
```

Create Job:

```http
POST /api/jobs
Content-Type: application/json
```

```json
{
  "title": "hostname recoverable job",
  "tool_id": "command.run",
  "payload": {
    "type": "command",
    "program": "hostname",
    "args": [],
    "timeout_seconds": 30
  },
  "placement": {
    "os": "linux",
    "workbench_id": "optional-physical-machine-id"
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
  "idempotency": {
    "key": "hostname-demo",
    "mode": "at_least_once"
  }
}
```

CLI:

```bash
agentgrid jobs submit \
  --tool command.run \
  --title "hostname recoverable job" \
  --payload '{"type":"command","program":"hostname","args":[],"timeout_seconds":30}' \
  --max-attempts 3 \
  --wait
```

Sharded Job:

```bash
agentgrid jobs submit \
  --tool command.run \
  --title "sharded hostname job" \
  --payload '{"type":"command","program":"hostname","args":[],"timeout_seconds":30}' \
  --os linux \
  --shards 4 \
  --max-parallelism 2 \
  --reduce stdout_concat \
  --max-attempts 3 \
  --wait
```

Items Partition Job:

```bash
agentgrid jobs submit \
  --tool command.run \
  --title "partitioned echo job" \
  --payload '{"type":"command","program":"echo","args":["partition"],"timeout_seconds":30}' \
  --os linux \
  --shards 2 \
  --max-parallelism 2 \
  --partition-items '["a","b","c","d"]' \
  --reduce json_array \
  --wait
```

Range Partition Job:

```bash
agentgrid jobs submit \
  --tool command.run \
  --title "range partition job" \
  --payload '{"type":"command","program":"echo","args":["range"],"timeout_seconds":30}' \
  --os linux \
  --shards 4 \
  --partition-range start=0,end=100,step=1 \
  --reduce summary \
  --wait
```

Hub injects shard metadata into the task payload:

```json
{
  "shard": {
    "index": 0,
    "count": 4,
    "first": true,
    "last": false
  }
}
```

Hub also injects partition metadata into every shard task payload.

Items shard payload:

```json
{
  "partition": {
    "type": "items",
    "items": ["a", "b"],
    "item_count": 2,
    "total_items": 4
  }
}
```

Range shard payload:

```json
{
  "partition": {
    "type": "range",
    "start": 0,
    "end": 25,
    "step": 1,
    "total_units": 100
  }
}
```

Payload Template Engine:

Hub renders payload templates after injecting shard and partition metadata. Supported examples:

| Template | Meaning |
| --- | --- |
| `${shard.index}` | Current shard index. |
| `${shard.count}` | Total shard count. |
| `${partition.items[0]}` | First item assigned to this shard. |
| `${partition.range.start}` | Current shard range start. |
| `${partition.range.end}` | Current shard range end. |

Batch HTTP example:

```bash
agentgrid jobs submit \
  --tool http.request \
  --title "batch fetch urls" \
  --payload '{"type":"http_request","method":"GET","url":"${partition.items[0]}","headers":[],"body":null,"timeout_seconds":30,"max_response_bytes":65536}' \
  --os linux \
  --shards 2 \
  --max-parallelism 2 \
  --partition-items '["https://example.com","https://httpbin.org/get"]' \
  --reduce json_array \
  --wait
```

Reduce strategies:

| Strategy | Purpose |
| --- | --- |
| `summary` | Count shard success/failure and keep every shard result. |
| `stdout_concat` | Concatenate `stdout` from shard results in shard order. |
| `json_array` | Return shard result objects as an array. |

After all shards finish, Hub creates an auditable reducer task with tool label `job.reduce` and writes the final Job result:

```json
{
  "type": "job_reduce_result",
  "summary": {
    "state": "passed",
    "shard_count": 4,
    "success_count": 4,
    "failed_count": 0
  },
  "reducer_result": {
    "type": "stdout_concat",
    "stdout": "node-a\nnode-b\n"
  },
  "shards": [],
  "artifacts": []
}
```

When a retry uses a checkpoint, Hub injects resume metadata into the task payload:

```json
{
  "resume_from": {
    "checkpoint_id": "checkpoint_xxx",
    "sequence": 12,
    "progress": 42,
    "resume_token": { "cursor": "page-42" },
    "artifacts": []
  }
}
```

Read Job:

```bash
agentgrid jobs
agentgrid jobs get --id job_xxx
```

Idempotent submit:

When `idempotency.key` is provided, Hub stores it in the Job index. If the same key is submitted again, Hub returns the existing Job instead of creating another Job or another Attempt.

```bash
agentgrid jobs submit \
  --tool command.run \
  --title "hostname once" \
  --payload '{"type":"command","program":"hostname","args":[],"timeout_seconds":30}' \
  --os linux \
  --idempotency-key hostname-once
```

Duplicate submit response:

```json
{
  "ok": true,
  "reused": true,
  "item": {
    "metadata": {
      "id": "job_xxx"
    },
    "spec": {
      "idempotency_key": "hostname-once"
    },
    "status": {
      "idempotency_reused": true
    }
  }
}
```

Rules:

- Use a stable `idempotency.key` for client retries.
- Use a different key for intentionally different work.
- Hub-level idempotency prevents duplicate Job creation; exactly-once side effects still require the selected tool/external system to honor idempotency.

Plan Job without creating it:

```http
POST /api/jobs/plan
Content-Type: application/json
```

```json
{
  "title": "hostname recoverable job",
  "tool_id": "command.run",
  "payload": {
    "type": "command",
    "program": "hostname",
    "args": [],
    "timeout_seconds": 30
  },
  "placement": {
    "os": "linux"
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
  "idempotency": {
    "key": "hostname-demo",
    "mode": "idempotent"
  }
}
```

Plan response:

```json
{
  "ok": true,
  "item": {
    "api_version": "agentgrid.job-plan/v1",
    "kind": "JobPlan",
    "valid_payload": true,
    "can_run": true,
    "tool_id": "command.run",
    "selected_node_id": "hub-node",
    "eligible_nodes": [],
    "rejected_nodes": [],
    "execution_shape": {
      "strategy": { "type": "single" },
      "estimated_attempts": 1,
      "max_parallelism": 1
    },
    "reliability": {
      "delivery": "at_least_once",
      "max_attempts": 3,
      "on_node_lost": "reschedule",
      "checkpoint_enabled": true,
      "idempotency_key": "hostname-demo",
      "safe_for_retry": true
    },
    "retry_reschedule_contract": {
      "api_version": "agentgrid.retry-reschedule/v1",
      "kind": "RetryReschedulePlan",
      "max_attempts": 3,
      "safe_for_retry": true,
      "decisions": {
        "node_lost": {
          "should_reschedule": true,
          "reason": "retry_policy_allows_reschedule"
        },
        "process_failed": {
          "should_reschedule": true,
          "reason": "retry_policy_allows_reschedule"
        }
      }
    },
    "warnings": [],
    "normalized_job": {}
  }
}
```

CLI:

```bash
agentgrid jobs plan \
  --tool command.run \
  --title "hostname recoverable job" \
  --payload '{"type":"command","program":"hostname","args":[],"timeout_seconds":30}' \
  --os linux \
  --max-attempts 3 \
  --idempotency-key hostname-demo
```

Read reliability status:

```http
GET /api/jobs/reliability
```

The response exposes:

- Current runtime guarantee.
- Lease settings.
- Queued/running/done/failed Job counts.
- Lost/failed Attempt counts.
- Total checkpoints.
- Standard endpoints used by AI clients.

Read Job execution view:

```http
GET /api/jobs/{id}/execution
```

CLI:

```bash
agentgrid jobs execution --id job_xxx
```

Response shape:

```json
{
  "ok": true,
  "item": {
    "api_version": "agentgrid.job-execution/v1",
    "kind": "JobExecutionView",
    "job_id": "job_xxx",
    "summary": {
      "attempts": {
        "total": 1,
        "queued": 0,
        "running": 0,
        "done": 1,
        "failed": 0,
        "lost": 0
      },
      "checkpoints": {
        "total": 2,
        "latest": {}
      },
      "events": {
        "total": 8,
        "latest": {}
      }
    },
    "recovery": {
      "state": "done",
      "failure_reason": "none",
      "latest_attempt_id": "attempt_xxx",
      "latest_task_id": "task_xxx",
      "latest_checkpoint": {},
      "retry_decision": null,
      "contract": {
        "api_version": "agentgrid.retry-reschedule/v1",
        "kind": "RetryReschedulePlan"
      }
    },
    "timeline": [
      {
        "time": "2026-06-01T00:00:00Z",
        "type": "job.created",
        "summary": "Job created"
      },
      {
        "time": "2026-06-01T00:00:01Z",
        "type": "job.attempt.created",
        "summary": "Attempt 1 created"
      }
    ],
    "attempts": [],
    "checkpoints": [],
    "events": []
  }
}
```

Use this endpoint when an AI client needs to explain a Job result, decide whether a retry is safe, or show a user-facing execution timeline.

Run recovery scan:

```http
POST /api/jobs/recovery/scan
```

CLI:

```bash
agentgrid jobs recovery-scan
```

Response shape:

```json
{
  "ok": true,
  "item": {
    "api_version": "agentgrid.recovery/v1",
    "kind": "JobRecoveryScan",
    "trigger": "manual",
    "status": "completed",
    "started_at": "2026-06-01T00:00:00Z",
    "completed_at": "2026-06-01T00:00:01Z",
    "inputs": {
      "expired_leases_before": 1,
      "running_jobs_before": 2,
      "queued_jobs_before": 3
    },
    "outputs": {
      "rescheduled_attempts": 1,
      "stopped_attempts": 0,
      "recovered_items": [
        {
          "job_id": "job_xxx",
          "attempt_id": "attempt_xxx",
          "task_id": "task_xxx",
          "node_id": "hub-node",
          "shard_id": null,
          "outcome": "rescheduled",
          "next_attempt_id": "attempt_yyy",
          "error": {
            "code": "job_attempt_lost"
          },
          "retry_decision": {
            "failure_reason": "node_lost",
            "should_reschedule": true,
            "reason": "retry_policy_allows_reschedule"
          }
        }
      ],
      "expired_leases_after": 0,
      "running_jobs_after": 1,
      "queued_jobs_after": 4
    },
    "recovery_loop_seconds": 15,
    "contract": {
      "api_version": "agentgrid.retry-reschedule/v1",
      "kind": "RetryRescheduleContract"
    }
  }
}
```

The Hub also runs the same recovery scan automatically every 15 seconds. The manual endpoint exists so an AI client, operator, or console can force a scan and receive a structured result.

`recovered_items` is the per-attempt recovery report. `outcome=rescheduled` means Hub created a replacement attempt. `outcome=failed` means retry policy or `max_attempts` stopped recovery and the Job or shard was marked failed.

Report checkpoint:

```bash
agentgrid jobs checkpoint \
  --id job_xxx \
  --attempt attempt_xxx \
  --task task_xxx \
  --node hub-node \
  --sequence 12 \
  --progress 42 \
  --resume-token '{"cursor":"page-42"}'
```

Worker automatic checkpoints:

Worker reports coarse-grained checkpoints for Job Attempts automatically:

| Sequence | Progress | Stage | When |
| --- | --- | --- | --- |
| `1` | `1` | `started` | Worker starts executing a Job Attempt. |
| `100` | `100` | `completed` | Worker finished execution and is about to report task completion. |

Automatic checkpoint `resume_token`:

```json
{
  "stage": "completed",
  "task_id": "task_xxx",
  "attempt_id": "attempt_xxx",
  "shard_id": "job_xxx_shard_0000",
  "result_type": "command_result"
}
```

Notes:

- Automatic checkpoints are best-effort; failure to report a checkpoint must not fail the task.
- Fine-grained resume, such as file cursor or page cursor, still requires the tool or AI client to call `agentgrid jobs checkpoint` with a richer `resume_token`.

Event ingress:

```http
POST /api/events/ingress
```

```json
{
  "source": "external-system",
  "type": "job.signal",
  "target": {
    "job_id": "job_xxx",
    "node_id": "hub-node"
  },
  "idempotency_key": "evt_xxx",
  "payload": {
    "signal": "pause"
  },
  "ttl_seconds": 300
}
```

CLI:

```bash
agentgrid jobs event \
  --source external-system \
  --type job.signal \
  --job job_xxx \
  --node hub-node \
  --payload '{"signal":"pause"}'
```

Recovery behavior:

- Hub creates the first JobAttempt as a normal task.
- If the task completes, Job becomes `done`.
- If the task fails, Hub marks the Attempt failed and creates another Attempt until `max_attempts`.
- If a node goes offline or a Job task lease expires, Hub marks the Attempt `lost` and creates a new Attempt.
- The next Attempt references the latest checkpoint through `resume_from`.

Worker lease renewal:

Worker must keep a running task lease alive while the task is still executing. The Hub is the canonical lease owner. Worker renews the lease by calling:

```http
POST /api/worker/tasks/{task_id}/renew
Content-Type: application/json
```

```json
{
  "node_id": "hub-node",
  "lease_seconds": 120
}
```

Success response:

```json
{
  "ok": true,
  "api_version": "agentgrid.worker-lease/v1",
  "kind": "WorkerLeaseRenewal",
  "task_id": "task_xxx",
  "node_id": "hub-node",
  "lease_seconds": 120,
  "lease_expires_at": "2026-06-01T00:00:00Z"
}
```

Rules:

- Hub only renews tasks in `in_progress`.
- `node_id` must match `status.leased_by_node_id`.
- `lease_seconds` is clamped to `10..600`.
- Worker renew failure is best-effort and does not immediately fail the task.
- If Worker stops renewing and the node is offline or the lease expires, Hub may mark the Job Attempt `lost` and reschedule it.

Worker Execution Journal:

Worker writes a local JSONL journal for every leased task. This is node-side execution memory. Hub remains the canonical state store, but the journal helps recovery and incident analysis when a Worker restarts or loses network.

Default path:

```text
$AGENTGRID_WORKER_HOME/worker/journal/{node_id}.jsonl
```

Fallback path:

```text
$HOME/.agentgrid/worker/journal/{node_id}.jsonl
```

Configure path:

```bash
agentgrid-worker \
  --hub https://hub.example.com/agentgrid \
  --id hub-node \
  --journal-path /var/lib/agentgrid/worker/journal/hub-node.jsonl
```

Disable journal:

```bash
agentgrid-worker --no-journal
```

Journal record shape:

```json
{
  "api_version": "agentgrid.worker-journal/v1",
  "kind": "WorkerExecutionJournalRecord",
  "time": "1710000000.000000000Z",
  "node_id": "hub-node",
  "event": "leased",
  "task_id": "task_xxx",
  "job_id": "job_xxx",
  "job_attempt_id": "attempt_xxx",
  "job_shard_id": "shard_xxx",
  "lease_expires_at": "2026-06-01T00:00:00Z",
  "detail": {}
}
```

Events:

| Event | Meaning |
| --- | --- |
| `leased` | Hub granted the task lease to this Worker. |
| `started` | Worker thread started executing the task. |
| `lease_renewed` | Worker successfully renewed the running task lease. |
| `lease_renew_failed` | Worker attempted to renew the lease, but Hub or network returned an error. |
| `reported` | Worker successfully reported completion or failure to Hub. |
| `report_failed` | Worker executed or tried to fail the task, but reporting to Hub failed. |

Worker Reconcile:

When Worker starts, it reads recent journal records and sends them to Hub:

```http
POST /api/worker/reconcile
Content-Type: application/json
```

```json
{
  "node_id": "hub-node",
  "records": [
    {
      "api_version": "agentgrid.worker-journal/v1",
      "event": "started",
      "task_id": "task_xxx",
      "job_id": "job_xxx",
      "job_attempt_id": "attempt_xxx"
    }
  ]
}
```

Hub response:

```json
{
  "ok": true,
  "api_version": "agentgrid.worker-reconcile/v2",
  "kind": "WorkerReconcileResult",
  "node_id": "hub-node",
  "checked": 1,
  "summary": {
    "total": 1,
    "needs_attention": 1,
    "by_action": {
      "worker_should_confirm_running_or_finish": 1
    },
    "by_severity": {
      "info": 1
    }
  },
  "needs_attention": [
    {
      "task_id": "task_xxx",
      "journal_event": "started",
      "hub_state": "in_progress",
      "leased_by_node_id": "hub-node",
      "action": "worker_should_confirm_running_or_finish",
      "severity": "info",
      "recovery": {
        "severity": "info",
        "automation": "wait_for_worker_report",
        "retryable": false,
        "is_job_attempt": true,
        "lease_expired": false,
        "recommendation": "Worker journal and Hub both show the task is running. Worker should keep renewing the lease and eventually report completion or failure.",
        "operator_action": "Monitor lease renewal and task logs."
      },
      "hub_snapshot": {
        "exists": true,
        "state": "in_progress",
        "leased_by_node_id": "hub-node",
        "lease_expires_at": "2026-06-01T00:00:00Z",
        "job_id": "job_xxx",
        "job_attempt_id": "attempt_xxx"
      }
    }
  ]
}
```

Reconcile v2 fields:

| Field | Meaning |
| --- | --- |
| `summary` | Hub-side counts by action and severity. |
| `severity` | `info`, `warning`, or `critical`. Used by dashboards and AI operators to decide urgency. |
| `recovery.automation` | The standard recovery hint. It is a recommendation, not automatic mutation. |
| `recovery.retryable` | Whether the case can be retried/rescheduled when the task is a Job Attempt and the tool is idempotent/checkpoint-aware. |
| `hub_snapshot` | Canonical Hub state for the task at reconcile time. |

Reconcile actions:

| Action | Meaning |
| --- | --- |
| `none` | Hub and Worker journal do not need action. |
| `worker_should_confirm_running_or_finish` | Journal says Worker started; Hub still has lease in progress. Worker should eventually finish, fail, or let lease expire. |
| `hub_missing_task` | Worker journal references a task missing from Hub. |
| `hub_does_not_know_worker_started` | Journal says started, but Hub state is still assigned/todo. |
| `worker_report_failed` | Worker could not report its execution result to Hub. |

Recovery automation values:

| Automation | Meaning |
| --- | --- |
| `none` | No recovery required. |
| `wait_for_worker_report` | Hub and Worker agree the task is running. Continue monitoring lease renewal and final report. |
| `eligible_for_job_reschedule` | Lease is expired and the record belongs to a Job Attempt; Hub recovery loop can reschedule according to retry policy. |
| `manual_inspection_required` | Non-Job task or ambiguous side effects require human or AI operator inspection. |
| `manual_audit_required` | Hub is missing the task referenced by Worker journal; preserve journal evidence and inspect side effects. |
| `manual_state_repair_required` | Worker claims started while Hub still shows queued/assigned; avoid duplicate work until operator confirms state. |
| `recover_result_or_reschedule` | Worker could not report result for a Job Attempt; recover result from Worker logs or reschedule if safe. |
| `manual_result_recovery` | Worker could not report result for a non-Job task; inspect Worker logs/artifacts manually. |

Guarantee:

```text
Job Runtime v1 is at-least-once. Exactly-once requires idempotent tools and checkpoint-aware executors.
```
