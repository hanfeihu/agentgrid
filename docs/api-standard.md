# AgentGrid Machine-Readable API Standard v1

This document defines the v1 machine-readable API surface for AgentGrid Hub.
The normative OpenAPI description is `docs/openapi/agentgrid-openapi.yaml`.
The normative JSON Schemas are in `schemas/`.

Base URL:

```text
https://hub.example.com/agentgrid/api
```

## Compatibility

- OpenAPI uses OpenAPI 3.1 and JSON Schema 2020-12 semantics.
- API responses use a common envelope: `ok: true` with `item` or `items`, or
  `ok: false` with `error`.
- Mutating endpoints should emit audit/event-bus records.
- AI clients must send structured payloads. Natural language may appear in
  titles or summaries, but scheduling, placement, tools, and results are
  machine-readable fields.
- Worker-facing endpoints are intentionally separate from AI/client-facing
  endpoints. This standard covers the public AI/client surface plus core worker
  task completion and event ingress contracts.

## Endpoint Families

| Family | Endpoints |
| --- | --- |
| Health | `GET /health` |
| Auth | `POST /auth/login`, `POST /auth/register/request-code`, `POST /auth/register` |
| Nodes | `GET /nodes`, `GET /nodes/{node_id}`, `POST /nodes/{node_id}/approve` |
| Tasks | `GET /tasks`, `POST /tasks`, `GET /tasks/{task_id}`, `GET /tasks/{task_id}/schedule-preview`, `POST /worker/tasks/{task_id}/complete`, `POST /worker/tasks/{task_id}/fail`, `POST /worker/tasks/{task_id}/renew` |
| Jobs | `GET /jobs`, `POST /jobs`, `POST /jobs/plan`, `GET /jobs/reliability`, `GET /jobs/{id}`, `GET /jobs/{id}/execution`, `POST /jobs/recovery/scan` |
| Artifacts | `GET /artifacts`, `GET /artifacts/{artifact_id}`, `GET /artifacts/{artifact_id}/download` |
| Tools | `GET /tools`, `GET /tools/{tool_id}`, `GET /tools/{tool_id}/nodes`, `POST /tools/{tool_id}/nodes/{node_id}/probe` |
| Events | `GET /events`, `GET /events/stream`, `POST /events/ingress` |

## Object Standards

### API Error

`schemas/api-error.schema.json` defines the shared error envelope. Error codes
are stable strings such as `bad_request`, `not_found`, `conflict`,
`invalid_transition`, `permission_denied`, `unauthorized`, `storage_error`,
`scheduler_no_eligible_node`, and `worker_execution_failed`.

### AgentTask

`schemas/agent-task.schema.json` defines the canonical task object returned by
task endpoints and embedded in task lifecycle messages. It is compatible with
the existing AgentTask v1 states:

```text
todo, assigned, in_progress, blocked, review, testing, done, failed, cancelled
```

Executable compute tasks must include a structured payload through
`spec.payload` and routable labels such as `compute`, `command`, `http_request`,
`tool:<tool_id>`, or `node:<node_id>`.

### AgentTaskResult

`schemas/agent-task-result.schema.json` defines the unified task result
envelope. It records success or failure, selected node, tool result, artifacts,
timing, verification, and retryability. Worker completion and failure endpoints
should store this shape or a compatible projection.

### Placement Contract

`schemas/placement-contract.schema.json` defines hard and soft scheduling
constraints plus decision records. Hard constraints include node id, OS,
required capabilities, required tools, online state, and policy allowance. Soft
constraints include preferred/avoided nodes, resource pressure, concurrency
slots, probe verification, historical success rate, node weight, risk, and cost.

## Validation

Run the local schema readability check:

```bash
node scripts/validate-agentgrid-schemas.js
```

The script intentionally performs a minimal self-check: every schema file must
exist and be readable by `JSON.parse`. It does not validate OpenAPI YAML.
