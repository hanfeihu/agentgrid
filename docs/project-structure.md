# AgentGrid Project Structure

This document describes the recommended module and directory structure for AgentGrid.

The project should be organized around one principle:

Only the runtime core owns job state, scheduling, worker coordination, execution, policy decisions, and results. CLI, desktop UI, SDKs, and integrations are clients of that runtime.

## 1. Top-Level Layout

Recommended final layout:

```text
agentgrid/
├── Cargo.toml
├── README.md
├── crates/
│   ├── agentgrid-protocol/
│   ├── agentmessage-protocol/
│   ├── agentgrid-core/
│   ├── agentgrid-store/
│   ├── agentgrid-scheduler/
│   ├── agentgrid-executor/
│   ├── agentgrid-policy/
│   ├── agentgrid-control/
│   ├── agentgrid-worker/
│   ├── agentgrid-api/
│   ├── agentgrid-events/
│   ├── agentgrid-secrets/
│   ├── agentgrid-hub/
│   ├── agentgrid-platform/
│   └── agentgrid-telemetry/
├── apps/
│   ├── agentgrid-cli/
│   ├── agentgrid-control/
│   ├── agentgrid-worker/
│   ├── agentgrid-hub/
│   └── agentgrid-desktop/
├── sdk/
│   ├── rust/
│   ├── typescript/
│   └── python/
├── schemas/
│   └── v1/
├── docs/
│   ├── requirements.md
│   ├── project-structure.md
│   ├── protocol.md
│   ├── api.md
│   ├── security.md
│   ├── agent-message.md
│   ├── hub.md
│   └── operations.md
├── examples/
│   ├── jobs/
│   ├── policies/
│   ├── clients/
│   └── cluster/
├── tests/
│   ├── integration/
│   └── fixtures/
└── packaging/
    ├── macos/
    ├── linux/
    └── windows/
```

## 2. Crates

### `agentgrid-protocol`

Owns the public protocol objects.

This crate should be stable, small, and easy for SDKs and external clients to depend on.

Responsibilities:

- `Job`
- `JobSpec`
- `JobStatus`
- `JobResult`
- `Node`
- `NodeStatus`
- `Client`
- `Policy`
- `Event`
- `ErrorResponse`
- API request/response DTOs
- Versioned protocol types, such as `agentgrid.io/v1`

Should not contain:

- Database logic
- Scheduling logic
- Execution logic
- Platform-specific code

Example internal layout:

```text
agentgrid-protocol/
└── src/
    ├── lib.rs
    ├── version.rs
    ├── job.rs
    ├── node.rs
    ├── client.rs
    ├── policy.rs
    ├── result.rs
    ├── event.rs
    └── error.rs
```

### `agentmessage-protocol`

Owns AI-to-AI communication protocol objects.

AgentMessage is used by AgentGrid Hub for structured agent collaboration. It should stay independent from the compute runtime where possible, but it may reference AgentGrid job IDs when collaboration tasks trigger compute jobs.

Responsibilities:

- `Agent`
- `AgentTask`
- `AgentMessage`
- `AgentEvent`
- `Contract`
- `ContractChange`
- `Review`
- `ReviewComment`
- `DecisionRecord`
- Message priorities
- Acknowledgement rules
- Collaboration error objects

Should not contain:

- Compute scheduling logic
- Worker execution logic
- UI code
- Database implementation details

Example internal layout:

```text
agentmessage-protocol/
└── src/
    ├── lib.rs
    ├── version.rs
    ├── agent.rs
    ├── task.rs
    ├── message.rs
    ├── event.rs
    ├── contract.rs
    ├── review.rs
    ├── decision.rs
    └── error.rs
```

### `agentgrid-core`

Owns domain logic that is independent of storage, networking, UI, and operating system details.

Responsibilities:

- Job state machine
- Attempt state machine
- Lease rules
- Retry rules
- Requirement matching primitives
- Domain errors
- Shared traits

Should not contain:

- SQLite code
- HTTP server code
- Actual process execution
- UI logic

Example internal layout:

```text
agentgrid-core/
└── src/
    ├── lib.rs
    ├── state_machine.rs
    ├── retry.rs
    ├── lease.rs
    ├── requirements.rs
    └── error.rs
```

### `agentgrid-store`

Owns persistence.

The first implementation should use SQLite. The crate should expose traits where useful, but avoid premature multi-database abstraction until needed.

Responsibilities:

- SQLite migrations
- Job storage
- Node storage
- Attempt storage
- Lease storage
- Event storage
- Result storage
- Policy storage
- Transaction helpers

Example internal layout:

```text
agentgrid-store/
└── src/
    ├── lib.rs
    ├── sqlite.rs
    ├── migrations.rs
    ├── jobs.rs
    ├── nodes.rs
    ├── attempts.rs
    ├── leases.rs
    ├── results.rs
    └── events.rs
```

### `agentgrid-scheduler`

Owns node selection and queue decisions.

Responsibilities:

- Select eligible nodes
- Match job requirements to node capabilities
- Apply priority ordering
- Apply basic fairness
- Choose least-loaded node
- Respect node states such as `draining` and `disabled`
- Explain scheduling decisions

Initial scheduler strategy:

```text
Filter online nodes
  -> Filter by capability
  -> Filter by OS/architecture/tags/resources
  -> Filter by policy
  -> Sort by load and running job count
  -> Select best node
```

Example internal layout:

```text
agentgrid-scheduler/
└── src/
    ├── lib.rs
    ├── strategy.rs
    ├── matcher.rs
    ├── score.rs
    └── decision.rs
```

### `agentgrid-executor`

Owns actual job execution on worker nodes.

Responsibilities:

- Execute HTTP request jobs
- Execute command jobs
- Enforce timeout
- Limit output size
- Capture stdout/stderr
- Produce structured results
- Normalize executor errors

Initial executors:

- HTTP executor
- Command executor

Future executors:

- Script executor
- File operation executor
- Browser executor
- Model inference executor
- Container executor
- MCP tool executor

Example internal layout:

```text
agentgrid-executor/
└── src/
    ├── lib.rs
    ├── executor.rs
    ├── http.rs
    ├── command.rs
    ├── output.rs
    └── error.rs
```

### `agentgrid-policy`

Owns permission and governance decisions.

Responsibilities:

- Decide `allow`, `deny`, or `ask_user`
- Check HTTP host allowlists/blocklists
- Check command allowlists
- Check client trust levels
- Check resource limits
- Check secret access
- Produce explainable policy decisions

Policy decisions must happen in the runtime, not only in the UI.

Example internal layout:

```text
agentgrid-policy/
└── src/
    ├── lib.rs
    ├── engine.rs
    ├── decision.rs
    ├── http_rules.rs
    ├── command_rules.rs
    ├── resource_rules.rs
    └── secret_rules.rs
```

### `agentgrid-control`

Owns the control plane service logic.

Responsibilities:

- Accept submitted jobs
- Store jobs
- Register nodes
- Receive heartbeats
- Run scheduling ticks
- Assign jobs
- Track leases
- Detect lost jobs
- Process job completion
- Emit events

This crate should hold the orchestration logic but should not own the HTTP routing details. HTTP routing belongs in `agentgrid-api`.

Example internal layout:

```text
agentgrid-control/
└── src/
    ├── lib.rs
    ├── service.rs
    ├── submit.rs
    ├── nodes.rs
    ├── scheduling.rs
    ├── leases.rs
    └── completion.rs
```

### `agentgrid-worker`

Owns worker node behavior.

Responsibilities:

- Register with control plane
- Send heartbeat
- Report resources
- Poll for jobs
- Execute assigned jobs
- Renew leases
- Upload results
- Upload logs
- Handle cancellation

Example internal layout:

```text
agentgrid-worker/
└── src/
    ├── lib.rs
    ├── service.rs
    ├── registration.rs
    ├── heartbeat.rs
    ├── resources.rs
    ├── polling.rs
    ├── runner.rs
    └── cancellation.rs
```

### `agentgrid-api`

Owns local and cluster API routing.

Responsibilities:

- HTTP server
- API routes
- Request authentication
- JSON serialization
- Error mapping
- API versioning

Recommended stack:

- Axum
- Tokio
- Serde

Example internal layout:

```text
agentgrid-api/
└── src/
    ├── lib.rs
    ├── server.rs
    ├── auth.rs
    ├── routes/
    │   ├── health.rs
    │   ├── jobs.rs
    │   ├── nodes.rs
    │   ├── worker.rs
    │   └── events.rs
    └── error.rs
```

### `agentgrid-events`

Owns event definitions and event publishing.

Responsibilities:

- Build structured events
- Persist events
- Expose event polling
- Later support streaming

Example events:

- `job.created`
- `job.scheduled`
- `job.started`
- `job.succeeded`
- `job.failed`
- `job.lost`
- `node.online`
- `node.offline`
- `policy.denied`

Example internal layout:

```text
agentgrid-events/
└── src/
    ├── lib.rs
    ├── event_bus.rs
    ├── publisher.rs
    └── stream.rs
```

### `agentgrid-hub`

Owns the online AI collaboration service.

AgentGrid Hub coordinates AI agents through tasks, messages, contracts, reviews, decisions, and shared memory.

Responsibilities:

- Agent registry
- Agent task board
- AgentMessage persistence and routing
- Contract center
- Contract change requests
- Review workflow
- Review comments
- Decision records
- Project memory
- Link collaboration tasks to AgentGrid compute jobs

This crate should use `agentmessage-protocol` for collaboration objects and AgentGrid compute APIs for execution.

Example internal layout:

```text
agentgrid-hub/
└── src/
    ├── lib.rs
    ├── service.rs
    ├── agents.rs
    ├── tasks.rs
    ├── messages.rs
    ├── contracts.rs
    ├── reviews.rs
    ├── decisions.rs
    ├── memory.rs
    └── compute_links.rs
```

### `agentgrid-secrets`

Owns secret references and secret injection.

Responsibilities:

- Store or reference secrets
- Resolve `secret_ref`
- Redact logs
- Restrict secret access by client and node

First version can be minimal. Do not overbuild this before policy and job execution are stable.

Example internal layout:

```text
agentgrid-secrets/
└── src/
    ├── lib.rs
    ├── reference.rs
    ├── resolver.rs
    └── redaction.rs
```

### `agentgrid-platform`

Owns OS-specific behavior.

Responsibilities:

- Data/config/cache directory discovery
- Service installation later
- Resource detection helpers
- Notification helpers later
- Process handling differences

Supported platforms:

- macOS
- Linux
- Windows

Example internal layout:

```text
agentgrid-platform/
└── src/
    ├── lib.rs
    ├── dirs.rs
    ├── service.rs
    ├── resources.rs
    ├── macos.rs
    ├── linux.rs
    └── windows.rs
```

### `agentgrid-telemetry`

Owns internal runtime observability.

Responsibilities:

- Logging setup
- Metrics primitives
- Tracing spans
- Diagnostic snapshots

This should stay lightweight in early versions.

Example internal layout:

```text
agentgrid-telemetry/
└── src/
    ├── lib.rs
    ├── logging.rs
    ├── metrics.rs
    └── diagnostics.rs
```

## 3. Apps

### `apps/agentgrid-cli`

User and AI friendly command-line interface.

Responsibilities:

- Submit jobs
- List jobs
- Query status
- Fetch results
- Cancel jobs
- List nodes
- Start local control plane for development
- Start local worker for development

Commands:

```text
agentgrid submit job.json
agentgrid jobs
agentgrid status <job_id>
agentgrid result <job_id>
agentgrid logs <job_id>
agentgrid cancel <job_id>
agentgrid nodes
agentgrid control run
agentgrid worker run
```

All commands should support:

```text
--json
```

### `apps/agentgrid-control`

Control plane binary.

Binary name:

```text
agentgrid-control
```

Responsibilities:

- Run HTTP API server
- Run scheduling loop
- Manage jobs and nodes
- Detect lease expiration

### `apps/agentgrid-worker`

Worker binary.

Binary name:

```text
agentgrid-worker
```

Responsibilities:

- Register node
- Send heartbeat
- Poll for jobs
- Execute jobs
- Return results

### `apps/agentgrid-desktop`

Desktop application.

This is not part of the first core runtime. It should come after CLI, control plane, worker, and policy are functional.

Responsibilities later:

- Show jobs
- Show nodes
- Approve or deny sensitive jobs
- Manage policies
- Manage clients
- Show logs and events

Recommended stack later:

- Tauri
- React or another frontend framework

### `apps/agentgrid-hub`

Online collaboration platform for AI agent teams.

Responsibilities:

- Run Hub web/API server
- Manage projects
- Manage agents
- Show task board
- Show AgentMessage stream
- Show contracts and contract changes
- Show reviews and comments
- Show decisions and shared memory
- Link collaboration tasks to compute jobs

This app should come after the compute MVP has a stable protocol and API.

## 4. SDKs

SDKs should wrap the AgentGrid API and protocol objects.

Initial SDKs:

```text
sdk/rust
sdk/typescript
sdk/python
```

SDK responsibilities:

- Submit jobs
- Query jobs
- Read results
- Cancel jobs
- Register clients
- Subscribe to events later

SDKs should depend on the public protocol, not internal runtime crates.

## 5. Schemas

The `schemas/` directory stores public JSON schemas.

Recommended layout:

```text
schemas/
└── v1/
    ├── job.schema.json
    ├── node.schema.json
    ├── result.schema.json
    ├── policy.schema.json
    ├── event.schema.json
    └── error.schema.json
```

These schemas are part of the standard. Keep them stable and versioned.

## 6. Docs

Recommended docs:

```text
docs/
├── requirements.md
├── project-structure.md
├── protocol.md
├── agent-message.md
├── hub.md
├── api.md
├── security.md
├── scheduling.md
├── worker.md
├── operations.md
└── roadmap.md
```

Document responsibilities:

- `requirements.md`: product and system requirements
- `project-structure.md`: codebase layout and module boundaries
- `protocol.md`: public protocol objects
- `agent-message.md`: AI-to-AI communication protocol
- `hub.md`: online collaboration platform requirements
- `api.md`: HTTP API
- `security.md`: policy, secrets, audit, risk model
- `scheduling.md`: node selection and resource model
- `worker.md`: worker lifecycle
- `operations.md`: running local and cluster deployments
- `roadmap.md`: release stages

## 7. Examples

Examples should make the system easy for AI clients and developers to understand.

Recommended layout:

```text
examples/
├── jobs/
│   ├── http-ollama.json
│   ├── http-api.json
│   └── command-cargo-check.json
├── policies/
│   ├── local-safe.json
│   └── command-allowlist.json
├── clients/
│   ├── python-submit-job/
│   └── typescript-submit-job/
└── cluster/
    ├── single-machine.md
    └── lan-cluster.md
```

## 8. Tests

Recommended test structure:

```text
tests/
├── integration/
│   ├── submit_http_job.rs
│   ├── submit_command_job.rs
│   ├── worker_heartbeat.rs
│   ├── scheduling.rs
│   └── lease_expiration.rs
└── fixtures/
    ├── jobs/
    ├── policies/
    └── nodes/
```

Important test areas:

- Job state transitions
- Scheduler decisions
- HTTP job execution
- Command job execution
- Worker heartbeat
- Lease renewal
- Lease expiration
- Lost job recovery
- Policy allow/deny decisions
- Result persistence

## 9. Packaging

Packaging comes after the runtime is stable.

Recommended layout:

```text
packaging/
├── macos/
│   └── launch-agent/
├── linux/
│   └── systemd/
└── windows/
    └── service/
```

Targets:

- macOS LaunchAgent
- Linux systemd user service
- Windows service or startup task

## 10. Dependency Direction

Keep dependency direction clean.

Recommended dependency flow:

```text
protocol
agentmessage-protocol
  ↓
core
  ↓
store / scheduler / policy / executor / platform
  ↓
control / worker
  ↓
api / apps
```

Collaboration dependency flow:

```text
agentmessage-protocol
  ↓
agentgrid-hub
  ↓
apps/agentgrid-hub
```

Hub may call AgentGrid compute APIs, but compute runtime crates should not depend on Hub.

Important rules:

- `agentgrid-protocol` must not depend on runtime crates.
- `agentmessage-protocol` must not depend on runtime crates.
- `agentgrid-core` must not depend on API, store, or executor.
- `agentgrid-store` should not know about HTTP routing.
- `agentgrid-executor` should not own scheduling decisions.
- `agentgrid-policy` should not execute jobs.
- `agentgrid-control` coordinates modules but should not contain low-level executor code.
- `agentgrid-worker` executes jobs but should not decide cluster-wide scheduling.
- Apps should be thin wrappers.
- Compute runtime crates should not depend on Hub.
- Hub may reference compute job IDs, but collaboration state should remain separate from compute execution state.

## 11. MVP Directory

The full structure is useful as a north star, but the first implementation should stay smaller.

Recommended MVP layout:

```text
agentgrid/
├── crates/
│   ├── agentgrid-protocol/
│   ├── agentgrid-core/
│   ├── agentgrid-store/
│   ├── agentgrid-scheduler/
│   ├── agentgrid-executor/
│   ├── agentgrid-control/
│   └── agentgrid-worker/
├── apps/
│   └── agentgrid-cli/
├── schemas/
│   └── v1/
├── docs/
└── examples/
```

MVP can postpone:

- `agentgrid-desktop`
- `agentgrid-secrets`
- `agentgrid-telemetry`
- SDKs
- packaging
- advanced platform service installation

## 12. Migration From Current Prototype

The current prototype is named `ai-task-scheduler` and uses:

```text
crates/scheduler-core
apps/cli
apps/daemon
apps/desktop
```

Recommended migration:

```text
scheduler-core -> agentgrid-core plus agentgrid-protocol and agentgrid-store
apps/cli       -> apps/agentgrid-cli
apps/daemon    -> apps/agentgrid-control and apps/agentgrid-worker
apps/desktop   -> apps/agentgrid-desktop later
```

Concept migration:

```text
Task       -> Job
TaskRun    -> Attempt
Daemon     -> Control Plane plus Worker
Schedule   -> Queue and scheduler policy
Command    -> Executor payload
```

## 13. Naming

Recommended names:

- Product: `AgentGrid`
- CLI: `agentgrid`
- Control plane binary: `agentgrid-control`
- Worker binary: `agentgrid-worker`
- Protocol version: `agentgrid.io/v1`
- Rust crate prefix: `agentgrid-*`
