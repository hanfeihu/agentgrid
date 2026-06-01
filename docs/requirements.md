# AgentGrid Requirements

## 1. Product Vision

AgentGrid is an open, local-first compute runtime for AI agents.

It lets AI clients submit jobs, schedule them across local or clustered machines, execute them safely, and return structured results.

The project is not only a task scheduler. It is intended to become a lower-level execution and compute scheduling standard for AI clients.

## 2. Positioning

Modern AI clients are becoming more capable, but each client usually owns its own execution model. Large company products can be powerful, but they are often difficult to customize, extend, or govern at the lowest layer.

AgentGrid should provide a common base layer beneath AI clients:

- A job submission protocol
- A local and clustered compute runtime
- A worker/node model for reusing computer resources
- A policy layer for safe AI execution
- A structured result and event model
- An AI-to-AI collaboration protocol called AgentMessage
- An online collaboration platform for agent teams
- A foundation that different AI clients can integrate with

The long-term goal is to define an open standard for AI agents to use local and clustered computer resources.

AgentGrid should eventually have two connected layers:

- AgentGrid Compute: local and clustered job execution
- AgentGrid Hub: online collaboration for AI agent teams

AgentMessage is the communication protocol used by AgentGrid Hub.

## 3. Core Principle

AI clients should not directly control all local resources.

They should submit structured jobs to AgentGrid. AgentGrid decides:

- Whether the job is allowed
- Which node should execute it
- What resources the job can use
- How the job is isolated
- How the result is stored and returned
- What should be audited

## 4. Main Users

### AI Clients

Examples:

- Coding agents
- Desktop AI assistants
- Local agents
- MCP clients
- Browser agents
- Research agents
- Custom automation agents

They need to:

- Submit jobs
- Query job status
- Cancel jobs
- Read results
- Receive events
- Use compute resources across machines

### Local Users

Users need to:

- Control which AI clients are trusted
- Decide what jobs are allowed
- See what has run
- Stop or restrict workers
- Protect secrets, files, network access, and machine resources

### Developers

Developers need:

- CLI
- Local API
- JSON schema
- SDKs
- Logs
- Debug tools
- Extensible executors
- Stable protocol definitions

## 5. System Overview

```text
AI Client
  ↓
AgentGrid API / CLI / MCP
  ↓
Control Plane
  ↓
Job Queue
  ↓
Scheduler
  ↓
Worker Nodes
  ↓
Executors
  ↓
CPU / GPU / Memory / Network / Files / Local Services
```

## 6. Main Components

### Control Plane

The control plane is the brain of the system.

Responsibilities:

- Accept job submissions
- Store job definitions
- Manage job state
- Manage worker nodes
- Receive worker heartbeats
- Schedule jobs to workers
- Track attempts and leases
- Store results
- Enforce policies
- Emit events

The first version can run control plane and worker on the same machine.

### Worker Node

Each machine that contributes compute runs a worker.

Responsibilities:

- Register with the control plane
- Report heartbeat
- Report resources and capabilities
- Poll or receive assigned jobs
- Execute jobs locally
- Stream or upload logs
- Return structured results
- Renew leases while running jobs

### Scheduler

The scheduler decides where jobs should run.

Scheduling inputs:

- Node online status
- CPU availability
- Memory availability
- GPU capability
- OS and architecture
- Node tags
- Worker capabilities
- Job priority
- Client permissions
- Data locality
- Current load
- Battery and idle state

Initial strategy:

```text
Build candidate nodes
  -> Eligibility Gate
       reject offline / unknown / draining / disabled nodes
       reject nodes that do not match hard placement requirements
       reject nodes without required capability or registered tool
       reject avoided nodes and nodes without free slots
       reject nodes over the high-load limit
  -> Optimization Score
       compare CPU, memory, disk, slot pressure, node weight, success rate, preference, and tool trust
       choose the lowest score from eligible nodes only
  -> Lease job attempt to selected node
```

Business rule: placement requirements are hard constraints, not preferences. `node_id`, OS, capability, dynamic tool availability, node state, avoid list, slot availability, and high-load rejection belong to the Eligibility Gate. Resource score, node weight, historical success rate, preferred nodes, and Tool Probe trust belong to Optimization Score. A node rejected by the gate must never be restored by a later scoring pass.

### Executor

Executors perform the actual work on a worker node.

Initial executors:

- HTTP request executor
- Command executor

Future executors:

- Script executor
- File operation executor
- Model inference executor
- Browser executor
- Docker/container executor
- MCP tool executor
- Workflow executor

## 7. Core Objects

AgentGrid should define stable protocol objects.

### Client

Represents the AI client or application submitting jobs.

Fields:

- `id`
- `name`
- `kind`
- `trust_level`
- `created_at`
- `last_seen_at`
- `revoked_at`

### Node

Represents a machine in the compute grid.

Fields:

- `id`
- `name`
- `os`
- `arch`
- `cpu_cores`
- `memory_mb`
- `gpu_info`
- `tags`
- `capabilities`
- `status`
- `last_heartbeat_at`

Example:

```json
{
  "api_version": "agentgrid.io/v1",
  "kind": "Node",
  "metadata": {
    "id": "node_macbook_m3",
    "name": "MacBook M3"
  },
  "status": {
    "online": true,
    "os": "macos",
    "arch": "arm64",
    "cpu_cores": 10,
    "memory_mb": 32768,
    "load": 0.35,
    "tags": ["local", "ollama", "m3"],
    "capabilities": ["http_request", "command"]
  }
}
```

### Job

Represents work submitted by an AI client.

Fields:

- `id`
- `name`
- `client_id`
- `type`
- `priority`
- `requirements`
- `payload`
- `status`
- `created_at`
- `scheduled_at`
- `started_at`
- `finished_at`
- `assigned_node_id`

Example:

```json
{
  "api_version": "agentgrid.io/v1",
  "kind": "Job",
  "metadata": {
    "name": "call-local-ollama",
    "client": "codex"
  },
  "spec": {
    "type": "http_request",
    "requirements": {
      "tags": ["ollama"],
      "cpu_cores": 1,
      "memory_mb": 512
    },
    "payload": {
      "method": "POST",
      "url": "http://localhost:11434/api/generate",
      "body": {
        "model": "llama3",
        "prompt": "hello"
      },
      "timeout_seconds": 120,
      "max_response_bytes": 1048576
    }
  }
}
```

### Attempt

Represents one execution attempt of a job.

Fields:

- `id`
- `job_id`
- `node_id`
- `attempt_no`
- `status`
- `started_at`
- `finished_at`
- `exit_code`
- `error_message`

### Lease

Prevents jobs from being lost when a worker disappears.

Fields:

- `id`
- `job_id`
- `node_id`
- `expires_at`
- `renewed_at`

Workers must renew leases while running jobs. If a lease expires, the control plane can mark the job as lost and reschedule it.

### Result

Represents the structured output of a job.

Example:

```json
{
  "api_version": "agentgrid.io/v1",
  "kind": "JobResult",
  "job_id": "job_01HZ",
  "status": "succeeded",
  "node_id": "node_macbook_m3",
  "result": {
    "type": "http_response",
    "status_code": 200,
    "body": {
      "response": "hello"
    },
    "duration_ms": 842
  }
}
```

### Policy

Defines what clients are allowed to do.

Example:

```json
{
  "api_version": "agentgrid.io/v1",
  "kind": "Policy",
  "spec": {
    "client": "codex",
    "allow": [
      {
        "type": "http_request",
        "hosts": ["localhost", "api.openai.com"]
      },
      {
        "type": "command",
        "programs": ["git", "cargo", "node"]
      }
    ],
    "limits": {
      "max_concurrent_jobs": 3,
      "max_memory_mb": 4096
    },
    "approval": {
      "required_for": ["shell", "file_delete", "secret_access"]
    }
  }
}
```

## 8. Job Types

### HTTP Request Job

Submits an HTTP request to be executed by a selected worker.

Use cases:

- Call local model services such as Ollama
- Call local ComfyUI or Stable Diffusion APIs
- Call internal network services
- Call external APIs through a selected node
- Reuse a node's network position or credentials

Payload fields:

- `method`
- `url`
- `headers`
- `body`
- `timeout_seconds`
- `follow_redirects`
- `max_response_bytes`
- `auth`

Security requirements:

- Require timeout
- Limit response size
- Redact sensitive headers
- Block dangerous hosts by default
- Support host allowlists
- Prefer `secret_ref` over raw secret values

### Command Job

Runs a local executable with arguments.

Payload fields:

- `program`
- `args`
- `working_dir`
- `env`
- `timeout_seconds`
- `max_stdout_bytes`
- `max_stderr_bytes`

Security requirements:

- Prefer `program + args` over raw shell strings
- Shell execution should be high risk
- Support command allowlists
- Restrict working directories
- Capture stdout, stderr, and exit code

### Future Job Types

- `script`
- `file_operation`
- `model_inference`
- `browser_task`
- `container`
- `tool_call`
- `workflow`
- `batch`

## 9. Job States

Required states:

- `queued`
- `scheduled`
- `dispatching`
- `running`
- `succeeded`
- `failed`
- `retrying`
- `cancelled`
- `lost`
- `blocked`

State flow:

```text
queued
  -> scheduled
  -> dispatching
  -> running
  -> succeeded

running
  -> failed
  -> retrying
  -> queued

running
  -> lost
  -> queued

queued
  -> cancelled

queued
  -> blocked
```

## 10. Node States

Required states:

- `online`
- `offline`
- `busy`
- `draining`
- `disabled`
- `untrusted`

`draining` means the node should not accept new jobs, but existing jobs may finish.

## 11. Resource Model

Jobs may request:

- CPU cores
- Memory
- GPU
- GPU memory
- Disk space
- Network access
- OS
- Architecture
- Node tags
- Specific node
- Capabilities

Example:

```json
{
  "requirements": {
    "cpu_cores": 4,
    "memory_mb": 8192,
    "gpu": true,
    "tags": ["stable-diffusion"]
  }
}
```

Workers should report:

- Total CPU cores
- Available CPU estimate
- Total memory
- Available memory
- GPU information
- Running job count
- Load average
- Battery status
- Idle status
- Tags
- Capabilities

## 12. Cluster Modes

### Single-Machine Mode

```text
AI Client -> Local Control Plane -> Local Worker
```

This is the first development target.

### LAN Cluster Mode

```text
AI Client -> Main Machine Control Plane -> Multiple LAN Workers
```

This is the second target.

### Remote Cluster Mode

```text
AI Client -> Control Server -> Remote Workers
```

This is a later target because it requires stronger authentication, encryption, and network design.

## 13. Communication Model

Initial communication should use:

- HTTP API
- Worker polling
- JSON payloads

Worker polling is preferred for the first version because it works across NAT and is simple to debug.

Future options:

- WebSocket
- Server-Sent Events
- gRPC
- QUIC
- MCP server

## 14. API Requirements

Core endpoints:

```text
GET  /health
POST /clients/register
GET  /nodes
POST /nodes/register
POST /nodes/heartbeat
POST /jobs
GET  /jobs
GET  /jobs/:id
POST /jobs/:id/cancel
GET  /jobs/:id/result
GET  /jobs/:id/logs
POST /worker/poll
POST /worker/jobs/:id/start
POST /worker/jobs/:id/finish
POST /worker/jobs/:id/lease
```

## 15. CLI Requirements

The CLI should be AI-friendly.

Commands:

```text
agentgrid control run
agentgrid worker run
agentgrid worker register
agentgrid submit job.json
agentgrid jobs
agentgrid status <job_id>
agentgrid result <job_id>
agentgrid logs <job_id>
agentgrid cancel <job_id>
agentgrid nodes
```

All commands should support structured JSON output:

```text
--json
```

## 16. Storage Requirements

Initial storage:

- SQLite

Core tables:

```text
clients
nodes
jobs
job_attempts
job_logs
leases
policies
secrets
events
```

SQLite is enough for the first local-first version. Other databases can be considered later if remote multi-user control planes become important.

## 17. Events

The system should emit structured events.

Required events:

- `job.created`
- `job.scheduled`
- `job.started`
- `job.progress`
- `job.succeeded`
- `job.failed`
- `job.cancelled`
- `job.lost`
- `node.registered`
- `node.online`
- `node.offline`
- `policy.denied`

First version can store events in SQLite and expose them through polling.

Future versions can support streaming events.

## 18. Security Requirements

AgentGrid must be conservative by default.

Required security controls:

- Client identity
- API tokens
- Local-only default binding
- Host allowlists for HTTP jobs
- Dangerous host blocklist
- Command allowlists
- Working directory restrictions
- Secret references
- Secret redaction in logs
- Maximum job runtime
- Maximum response/log size
- Resource limits
- Audit logs
- Policy decisions: `allow`, `deny`, `ask_user`

Dangerous HTTP targets should be blocked by default, such as:

- Cloud metadata addresses
- Local sensitive admin services
- Non-HTTP schemes
- Broad internal network scans

## 19. Secret Management

AI clients should not pass raw secrets when possible.

Preferred model:

```json
{
  "auth": {
    "type": "secret_ref",
    "name": "openai_api_key"
  }
}
```

Secret requirements:

- Store secrets securely where possible
- Inject secrets only at execution time
- Redact secrets from logs
- Restrict secrets by client and node
- Support rotation later

## 20. Reliability Requirements

Required reliability features:

- Retries
- Retryable error classification
- Lease renewal
- Lost job detection
- Node offline detection
- Attempt history
- Idempotency keys
- Dedupe keys
- Graceful cancellation
- Hard kill fallback

Retry policy example:

```json
{
  "retry": {
    "max_attempts": 3,
    "backoff": "exponential",
    "initial_delay_seconds": 30
  }
}
```

## 21. Scheduling Features

Initial scheduling:

- FIFO within priority
- Priority support
- Node tag matching
- Capability matching
- CPU and memory filtering
- Least-loaded node selection

Future scheduling:

- Data locality
- Cost-aware scheduling
- Energy-aware scheduling
- Idle-machine scheduling
- GPU-aware scheduling
- Fair sharing by client
- Preemption
- Batch splitting

## 22. Idle Compute Reuse

AgentGrid should eventually support using idle computers safely.

Signals:

- User idle time
- CPU load
- Memory pressure
- Battery state
- Charging state
- Screen lock state
- Time window
- Temperature if available

Policy example:

```json
{
  "run_policy": {
    "only_when_idle": true,
    "min_idle_minutes": 10,
    "require_charging": true
  }
}
```

## 23. Observability

The system should expose:

- Queue length
- Running jobs
- Completed jobs
- Failed jobs
- Average runtime
- Failure rate
- Worker heartbeat age
- Node load
- Executor errors
- Scheduler decisions

## 24. Cross-Platform Requirements

Supported platforms:

- macOS
- Linux
- Windows

Rust is the preferred implementation language because it provides:

- Native performance
- Low runtime overhead
- Strong concurrency support
- Memory safety
- Cross-platform binaries
- Good fit for background services and workers

Suggested stack:

- Rust
- Tokio
- Axum
- SQLite
- Serde
- Reqwest
- Clap
- Sysinfo

## 25. Project Modules

Target module layout:

```text
agentgrid/
├── crates/
│   ├── agentgrid-core
│   ├── agentgrid-store
│   ├── agentgrid-control
│   ├── agentgrid-worker
│   ├── agentgrid-scheduler
│   ├── agentgrid-executor
│   ├── agentgrid-policy
│   └── agentgrid-protocol
├── apps/
│   ├── agentgrid-cli
│   ├── agentgrid-control
│   └── agentgrid-worker
└── docs/
```

## 26. MVP Scope

The first MVP should prove the core runtime.

Required:

- Rust workspace
- Job protocol objects
- Node protocol objects
- SQLite store
- Control plane process
- Worker process
- Worker registration
- Worker heartbeat
- Submit job through CLI
- HTTP request job
- Command job
- Basic scheduler
- Job attempt records
- Lease mechanism
- Result retrieval
- JSON output

Not required for MVP:

- Desktop UI
- Cloud sync
- Account system
- Multi-control-plane high availability
- Docker sandbox
- GPU scheduling
- Complex workflows
- Natural language scheduling

## 27. Roadmap

### Phase 1: Protocol and Single-Machine Runtime

- Define `agentgrid.io/v1` JSON objects
- Implement job creation
- Implement local control plane
- Implement local worker
- Execute HTTP jobs
- Execute command jobs
- Store results

### Phase 2: LAN Cluster

- Multi-worker registration
- Heartbeats
- Node tags
- Resource reports
- Scheduler assignment
- Lease renewal
- Lost job recovery

### Phase 3: AI Client Integration

- Local HTTP API
- MCP server
- TypeScript SDK
- Python SDK
- Rust SDK
- Example integrations

### Phase 4: Security and Governance

- Policy engine
- Secret manager
- Audit log
- Approval flow
- URL and command allowlists
- Node trust levels

### Phase 5: Open Standard and Ecosystem

- JSON schema
- Protocol documentation
- Executor plugin spec
- Capability registry
- Example worker implementations
- Community contribution guide

## 28. Differentiation

AgentGrid is different from existing systems because it combines:

- AI-client-first job protocol
- Local-first execution
- Cluster-ready worker model
- Cross-platform desktop machine support
- Rust-native low-level runtime
- HTTP and command execution as first-class job types
- Security and policy for AI actions
- Structured results for AI consumption
- Open protocol ambition

The core thesis:

Large companies may build powerful AI clients, but AI clients need an open, customizable, local-first execution and compute scheduling standard beneath them.

## 29. AgentGrid Hub

AgentGrid Hub is the online collaboration layer for AI agent teams.

It should let multiple AI agents coordinate through structured tasks, messages, events, reviews, contracts, and shared memory.

AgentGrid Hub is not only a chat room. It should behave more like a purpose-built collaboration platform for AI workers:

- Task board
- Agent registry
- Structured message bus
- Contract center
- Review system
- Shared project memory
- Decision records
- Compute job integration

The Hub should use AgentGrid Compute to run tests, execute jobs, inspect code, and process automation tasks.

Example relationship:

```text
AgentGrid Hub
  -> assigns AI development tasks
  -> sends AgentMessage events
  -> submits compute jobs to AgentGrid Compute
  -> receives test/build/run results
  -> updates task and review state
```

## 30. AgentMessage Protocol

AgentMessage is a structured communication protocol for AI agents.

It is used for AI-to-AI collaboration. It should not replace human-readable discussion, but it should provide machine-readable structure around important collaboration events.

AgentMessage should support:

- Task assignment
- Task progress
- Task completion
- Blocking reports
- Contract changes
- Review requests
- Review comments
- Test results
- Decision proposals
- Decision acceptance
- Broadcast announcements

Example:

```json
{
  "api_version": "agentmessage.io/v1",
  "kind": "AgentMessage",
  "metadata": {
    "id": "msg_01",
    "from": "ProtocolAgent",
    "to": ["WorkerAgent", "ApiAgent"],
    "created_at": "2026-05-31T12:00:00Z"
  },
  "spec": {
    "type": "contract.changed",
    "subject": "JobSpec",
    "summary": "Added requirements.capabilities field.",
    "priority": "normal",
    "requires_ack": true,
    "payload": {
      "files": ["schemas/v1/job.schema.json"],
      "breaking": false
    }
  }
}
```

AgentMessage should be a protocol and module inside AgentGrid, not necessarily the name of the entire system.

Recommended naming:

- AgentGrid: overall system and compute runtime
- AgentGrid Hub: online collaboration platform
- AgentMessage: AI-to-AI communication protocol

## 31. Agent Collaboration Requirements

AI agents working together need more than free-form chat.

They need:

- Shared task state
- Clear ownership
- Acceptance criteria
- Structured events
- Contract change notifications
- Review comments
- Test feedback
- Persistent decisions
- Shared memory

Core collaboration objects:

- `Agent`
- `AgentTask`
- `AgentMessage`
- `AgentEvent`
- `Contract`
- `ContractChange`
- `Review`
- `ReviewComment`
- `DecisionRecord`
- `ProjectMemory`

Initial Hub data tables:

```text
projects
agents
agent_tasks
agent_messages
contracts
contract_changes
reviews
review_comments
decisions
project_memory
compute_jobs
events
```

Agent tasks may link to compute jobs:

```text
agent_task -> compute_job
```

Example:

```text
Task: Implement HTTP executor
  -> ExecutorAgent writes code
  -> QAAgent submits test job to AgentGrid Compute
  -> Worker runs tests
  -> Result returns to Hub
  -> ReviewAgent comments or approves
```
