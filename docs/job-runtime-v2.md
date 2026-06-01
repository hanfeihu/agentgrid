# Job Runtime v2 Standard

Job Runtime v2 defines the recoverable execution contract for long-running,
batch, sharded, checkpointed, or reducer-based AgentGrid work.

A Job is not a replacement for a task. It is a reliability layer above normal
AgentGrid task execution. Hub still schedules Worker tasks, leases them, records
results, and captures evidence. Job Runtime adds attempts, shards, checkpoints,
retry decisions, reducer tasks, and a user-facing execution view.

Schema: `schemas/job-contract.schema.json`

## 1. Goals

- Let AI clients submit recoverable work with structured retry rules.
- Make at-least-once execution explicit.
- Support sharded jobs, item partitions, range partitions, and reducers.
- Preserve checkpoints so replacement attempts can resume when tools support it.
- Give humans and AI a clear execution timeline and recovery explanation.

## 2. Non-Goals

- Job Runtime does not guarantee exactly-once side effects.
- Job Runtime does not make non-idempotent tools safe to retry.
- Job Runtime does not bypass normal placement, policy, or capability checks.
- Job Runtime does not parse natural language.

## 3. Core Model

- `Job`: durable request, placement, reliability policy, and final result.
- `JobShard`: one partition of a sharded job.
- `JobAttempt`: one execution try for a job or shard through a normal task.
- `JobCheckpoint`: recoverable progress reported by Worker, tool, plugin, or
  client.
- `Reducer`: final aggregation task that combines shard results.
- `RecoveryScan`: Hub process that finds lost attempts and applies retry policy.
- `ExecutionView`: AI/human-readable status, timeline, attempts, checkpoints,
  evidence, and retry explanation.

## 4. Job Contract

Minimum request:

```json
{
  "api_version": "agentgrid.job/v2",
  "kind": "JobContract",
  "metadata": {
    "id": "job_01J00000000000000000000000",
    "created_at": "2026-06-01T00:00:00Z",
    "created_by": "architect-agent"
  },
  "spec": {
    "title": "Fetch URLs",
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
      "required_capabilities": ["http"],
      "os": "linux"
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
    "idempotency": {
      "key": "fetch-url-demo",
      "mode": "idempotent"
    },
    "evidence": {
      "required_types": ["structured_result"],
      "retain_attempt_evidence": true
    }
  }
}
```

## 5. Job States

Standard job states:

- `queued`
- `planning`
- `running`
- `reducing`
- `done`
- `failed`
- `cancelled`
- `paused`

Shard states:

- `queued`
- `running`
- `done`
- `failed`
- `lost`
- `skipped`

Attempt states:

- `queued`
- `leased`
- `running`
- `done`
- `failed`
- `lost`
- `cancelled`

Recommended state flow:

```text
queued -> planning -> running -> reducing -> done
queued -> planning -> failed
running -> failed
running -> paused -> running
running -> cancelled
```

## 6. Execution Strategy

Strategies:

- `single`: one attempt produces one result.
- `sharded`: Hub creates multiple shards and attempts.

For sharded jobs:

- `shard_count` must be at least 1.
- `max_parallelism` limits concurrently running shards.
- `payload_mode=inject_shard` means Hub adds shard metadata to each task
  payload.
- `payload_mode=template_only` means Hub renders template variables but does not
  inject a `shard` object.

Injected shard metadata:

```json
{
  "shard": {
    "id": "shard_0000",
    "index": 0,
    "count": 2,
    "first": true,
    "last": false
  }
}
```

## 7. Partitioning

Partition types:

- `none`: no partition.
- `items`: split a list of JSON values across shards.
- `range`: split an integer range across shards.

Items partition:

```json
{
  "type": "items",
  "items": ["a", "b", "c", "d"]
}
```

Range partition:

```json
{
  "type": "range",
  "start": 0,
  "end": 100,
  "step": 1
}
```

Hub injects partition metadata into each shard task payload:

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

## 8. Payload Templates

Hub renders string templates after shard and partition planning.

Supported variables:

- `${job.id}`
- `${job.title}`
- `${shard.id}`
- `${shard.index}`
- `${shard.count}`
- `${partition.items[0]}`
- `${partition.range.start}`
- `${partition.range.end}`
- `${attempt.index}`
- `${resume_from.checkpoint_id}`

Templates are only substitutions inside JSON string values. Hub must not
evaluate code.

## 9. Retry And Reschedule

Default retry policy:

```json
{
  "max_attempts": 1,
  "on_node_lost": "reschedule",
  "on_process_failed": "fail"
}
```

Policy values:

- `reschedule`: create a replacement attempt while attempts remain.
- `reschedule_if_idempotent`: reschedule only when idempotency makes retry safe.
- `fail`: mark attempt, shard, or job failed.

Decision table:

| Failure | Policy | Result |
| --- | --- | --- |
| `node_lost` | `reschedule` | Reschedule while under `max_attempts`. |
| `node_lost` | `fail` | Stop recovery for that job or shard. |
| `process_failed` | `reschedule_if_idempotent` | Retry only with `idempotency.key` or safe idempotency mode. |
| `process_failed` | `fail` | Stop recovery for that job or shard. |

Guarantee:

```text
Job Runtime v2 is at-least-once. Exactly-once requires idempotent external systems and checkpoint-aware tools.
```

## 10. Idempotency

`idempotency.key` prevents duplicate Job creation for client retries. It does
not guarantee exactly-once side effects inside the selected tool.

Modes:

- `none`: no retry safety is claimed.
- `at_least_once`: duplicate side effects are possible.
- `idempotent`: repeated attempts should produce the same external effect.
- `external_exactly_once`: the external system enforces exactly-once by key.

AI clients should provide a stable key for client retries and a different key
for intentionally different work.

## 11. Checkpoints

Checkpoint modes:

- `disabled`
- `worker_reported`
- `tool_reported`
- `client_reported`

Checkpoint shape:

```json
{
  "api_version": "agentgrid.job-checkpoint/v2",
  "kind": "JobCheckpoint",
  "checkpoint_id": "checkpoint_xxx",
  "job_id": "job_xxx",
  "attempt_id": "attempt_xxx",
  "shard_id": "shard_0000",
  "sequence": 12,
  "progress": 42,
  "stage": "fetching",
  "resume_token": {
    "cursor": "page-42"
  },
  "created_at": "2026-06-01T00:00:00Z"
}
```

When retrying from a checkpoint, Hub injects:

```json
{
  "resume_from": {
    "checkpoint_id": "checkpoint_xxx",
    "sequence": 12,
    "progress": 42,
    "resume_token": {
      "cursor": "page-42"
    }
  }
}
```

## 12. Reduce

Reducer types:

- `none`: no reducer; final result is the single task or shard summary.
- `summary`: count successes, failures, lost attempts, retries, and evidence.
- `stdout_concat`: concatenate `stdout` fields in shard order.
- `json_array`: return shard result objects as an array.
- `custom_tool`: run a declared tool as the reducer.

Reducer output:

```json
{
  "type": "job_reduce_result",
  "summary": {
    "state": "done",
    "shard_count": 2,
    "success_count": 2,
    "failed_count": 0
  },
  "reducer_result": {
    "type": "json_array",
    "items": []
  },
  "evidence": []
}
```

Reducer tasks must be audited and may produce Evidence Pipeline v2 items.

## 13. Evidence

Job Runtime should preserve:

- Job-level plan and final result evidence.
- Per-attempt stdout, stderr, structured result, and error evidence.
- Per-shard result evidence.
- Checkpoint evidence when it contains externally useful progress.
- Reducer evidence.

`spec.evidence.required_types` tells the Hub and clients what proof is expected
before the job is considered explainable.

## 14. Execution View

AI and human clients should explain jobs from a structured execution view:

```json
{
  "api_version": "agentgrid.job-execution/v2",
  "kind": "JobExecutionView",
  "job_id": "job_xxx",
  "state": "done",
  "summary": {
    "attempts": {
      "total": 2,
      "done": 2,
      "failed": 0,
      "lost": 0
    },
    "checkpoints": {
      "total": 4
    },
    "evidence": {
      "total": 6
    }
  },
  "recovery": {
    "delivery": "at_least_once",
    "safe_for_retry": true,
    "latest_decision": null
  },
  "timeline": []
}
```

The execution view is the preferred source for answering:

- What ran?
- Where did it run?
- Which attempt failed or was lost?
- Was retry safe?
- Which checkpoint was used?
- Which evidence proves the result?

## 15. Capability Graph Standard

Capability Graph is the relationship map Job Runtime, Plugin Runtime, and AI
clients use before placing work on real nodes.

Schema: `schemas/capability-graph.schema.json`

The graph connects:

- Node -> capability.
- Node -> device.
- Node -> tool.
- Tool -> plugin.
- Tool -> evidence type.
- Tool -> task intent.
- Tool -> dependency.
- Node/tool -> probe status.

Graph example:

```json
{
  "api_version": "agentgrid.capability-graph/v2",
  "kind": "CapabilityGraph",
  "metadata": {
    "generated_at": "2026-06-01T00:00:00Z",
    "hub_id": "agentgrid-hub"
  },
  "nodes": [
    {
      "node_id": "hub-node",
      "name": "Linux Worker",
      "os": "linux",
      "arch": "x86_64",
      "status": "online",
      "capabilities": ["command", "http", "plugin"],
      "tools": ["command.run", "http.request", "document.parse"]
    }
  ],
  "tools": [
    {
      "tool_id": "document.parse",
      "title": "Parse document",
      "executor": "plugin:agentgrid-plugin-document-parser",
      "capabilities": ["plugin", "document"],
      "required_evidence": ["structured_result", "file_artifact"],
      "probe": {
        "state": "verified",
        "last_checked_at": "2026-06-01T00:00:00Z"
      }
    }
  ],
  "edges": [
    {
      "from": "node:hub-node",
      "to": "tool:document.parse",
      "type": "provides_tool"
    }
  ]
}
```

Graph rules:

- A node must not advertise a tool it cannot execute.
- A tool edge must include a probe state when the tool is probeable.
- Job planning must use graph eligibility before scoring resources.
- AI clients should select `tool_id` from the graph, then build payloads from
  the tool contract.
- The graph is a snapshot. Clients should refresh it before submitting work that
  depends on node availability.

## 16. AI Client Rules

AI clients should:

- Use Job Runtime for long-running, sharded, recoverable, or reducer work.
- Run a plan/dry-run when placement or retry safety matters.
- Use idempotency keys for client retries.
- Avoid automatic retry of non-idempotent tools after process failure.
- Explain results from execution view and evidence, not from final status alone.

## 17. Human Operator Rules

Humans should be able to see:

- The original job request.
- The normalized plan.
- Node and tool placement reasons.
- Attempts, retries, checkpoints, and reducer output.
- Evidence records for each attempt and final result.
- Whether the job is safe to rerun.

## 18. Compatibility

Job Runtime v2 can read v1 jobs when Hub normalizes them into `JobContract`.

Migration rules:

- Map v1 `strategy`, `partition`, `reduce`, `retry_policy`,
  `checkpoint_policy`, and `idempotency` into `spec`.
- Preserve v1 attempt ids and task ids in execution view links.
- Treat missing `idempotency.mode` as `at_least_once` when a key exists and
  `none` otherwise.
- Treat missing evidence policy as `required_types=[]` and
  `retain_attempt_evidence=true`.
