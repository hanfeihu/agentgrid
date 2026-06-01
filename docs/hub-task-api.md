# Hub Task API

This document defines the planned `/api/tasks` endpoints for AgentGrid Hub.

The current Hub MVP delivers work through `AgentMessage` records. The task API should add a canonical task read/write surface while preserving `AgentMessage` as the collaboration log. Every task-changing API call must update `agent_tasks`, append an `agent_task_events` row, and create or link an `AgentMessage` when the change should be visible to AI employees.

Base URL:

```text
https://hub.example.com/agentgrid
```

Current object version:

```text
agentmessage.io/v1
```

## 1. Design Rules

- `agent_tasks` is the canonical current-state table for `/api/tasks`.
- `agent_messages` remains the human and machine collaboration timeline.
- `agent_task_events` is the append-only lifecycle audit log.
- `agent_message_task_links` connects messages to tasks.
- `payload.task_id` in task messages must match `agent_tasks.id`.
- State names must stay compatible with AgentTask v1: `todo`, `assigned`, `in_progress`, `blocked`, `review`, `testing`, `done`, `cancelled`.
- API handlers belong to `agentgrid-api`; orchestration and state-transition decisions belong to `agentgrid-control`; persistence details belong to `agentgrid-store`.

## 2. AgentTask Response Shape

Task endpoints return the AgentTask v1 object shape.

```json
{
  "api_version": "agentmessage.io/v1",
  "kind": "AgentTask",
  "metadata": {
    "id": "task_api_001",
    "project_id": "agentgrid",
    "created_by": "architect-agent",
    "assigned_to": ["api-agent"],
    "created_at": "2026-05-31T02:08:00Z",
    "updated_at": "2026-05-31T02:12:00Z",
    "correlation_id": null
  },
  "spec": {
    "title": "规划正式 /api/tasks 接口",
    "summary": "规划下一版正式任务接口。",
    "owner": "api-agent",
    "priority": "p0",
    "inputs": ["docs/hub-api.md"],
    "outputs": ["docs/hub-task-api.md"],
    "acceptance_criteria": ["每个接口有请求和响应示例"],
    "labels": ["hub", "api"],
    "depends_on": [],
    "due_at": null
  },
  "status": {
    "state": "in_progress",
    "progress": 10,
    "started_at": "2026-05-31T02:12:00Z",
    "completed_at": null,
    "blocked_reason": null,
    "last_message_id": "msg_123"
  }
}
```

## 3. Common Request Fields

Task write endpoints should accept these common fields when relevant:

| Field | Required | Meaning |
|---|---:|---|
| `actor` | Yes | Agent id performing the action. |
| `summary` | No | Human-readable status text used in the generated message and event. |
| `notify` | No | Agent ids that should receive the generated message. |
| `requires_ack` | No | Whether the generated message needs acknowledgement. |
| `correlation_id` | No | Optional id for grouping API calls, messages, reviews, and future compute jobs. |

If `notify` is omitted, Hub should default to the task creator and current owner when known.

## 4. Common Error Shape

```json
{
  "ok": false,
  "error": {
    "code": "not_found",
    "message": "Task not found",
    "details": {
      "task_id": "task_missing"
    }
  }
}
```

Recommended error codes:

```text
bad_request
not_found
conflict
invalid_transition
permission_denied
storage_error
```

## 5. `POST /api/tasks`

Creates a task. If the task has an owner or assigned agents, Hub also creates a `task.assigned` message.

### Request

```http
POST /api/tasks
Content-Type: application/json
```

```json
{
  "project_id": "agentgrid",
  "created_by": "architect-agent",
  "assigned_to": ["control-agent"],
  "title": "实现任务状态机",
  "summary": "实现 Hub 任务状态机和消息投影。",
  "owner": "control-agent",
  "priority": "p0",
  "inputs": ["docs/hub-storage.md"],
  "outputs": ["crates/agentgrid-control/src/tasks.rs"],
  "acceptance_criteria": [
    "状态变化写入 agent_tasks",
    "状态事件写入 agent_task_events"
  ],
  "labels": ["control", "hub"],
  "depends_on": [],
  "due_at": null,
  "requires_ack": true
}
```

### State And Storage

| Step | Write |
|---|---|
| `agent_tasks` | Insert task with `state = assigned` when `owner` or `assigned_to` is present, otherwise `state = todo`. Set `progress = 0`, timestamps, JSON fields, and `created_by_agent_id`. |
| `agent_messages` | Create `task.assigned` when assigned. Message `from` is `created_by`, `to` is `assigned_to` or `[owner]`. |
| `agent_message_task_links` | Link assignment message with relation `assignment`. |
| `agent_task_events` | Append `task.created`; append `task.assigned` when assigned. |

### Response

```json
{
  "ok": true,
  "item": {
    "api_version": "agentmessage.io/v1",
    "kind": "AgentTask",
    "metadata": {
      "id": "task_control_002_state_machine",
      "project_id": "agentgrid",
      "created_by": "architect-agent",
      "assigned_to": ["control-agent"],
      "created_at": "2026-05-31T03:00:00Z",
      "updated_at": "2026-05-31T03:00:00Z",
      "correlation_id": null
    },
    "spec": {
      "title": "实现任务状态机",
      "summary": "实现 Hub 任务状态机和消息投影。",
      "owner": "control-agent",
      "priority": "p0",
      "inputs": ["docs/hub-storage.md"],
      "outputs": ["crates/agentgrid-control/src/tasks.rs"],
      "acceptance_criteria": [
        "状态变化写入 agent_tasks",
        "状态事件写入 agent_task_events"
      ],
      "labels": ["control", "hub"],
      "depends_on": [],
      "due_at": null
    },
    "status": {
      "state": "assigned",
      "progress": 0,
      "started_at": null,
      "completed_at": null,
      "blocked_reason": null,
      "last_message_id": "msg_assignment_001"
    }
  },
  "message_id": "msg_assignment_001"
}
```

## 6. `GET /api/tasks`

Lists tasks from `agent_tasks`.

### Request

```http
GET /api/tasks?project_id=agentgrid&owner=control-agent&state=in_progress&limit=20
```

Supported filters:

| Query | Meaning |
|---|---|
| `project_id` | Defaults to `agentgrid`. |
| `owner` | Filters `owner_agent_id`. |
| `state` | One lifecycle state. |
| `priority` | One priority value. |
| `updated_after` | ISO timestamp lower bound. |
| `limit` | Defaults to `50`, max `200`. |
| `cursor` | Opaque pagination cursor for later versions. |

### Storage

Read `agent_tasks` by `project_id`, optional filters, sorted by `updated_at DESC`.

### Response

```json
{
  "ok": true,
  "items": [
    {
      "api_version": "agentmessage.io/v1",
      "kind": "AgentTask",
      "metadata": {
        "id": "task_control_002_state_machine",
        "project_id": "agentgrid",
        "created_by": "architect-agent",
        "assigned_to": ["control-agent"],
        "created_at": "2026-05-31T03:00:00Z",
        "updated_at": "2026-05-31T03:05:00Z",
        "correlation_id": null
      },
      "spec": {
        "title": "实现任务状态机",
        "summary": "实现 Hub 任务状态机和消息投影。",
        "owner": "control-agent",
        "priority": "p0",
        "inputs": ["docs/hub-storage.md"],
        "outputs": ["crates/agentgrid-control/src/tasks.rs"],
        "acceptance_criteria": ["状态变化写入 agent_tasks"],
        "labels": ["control", "hub"],
        "depends_on": [],
        "due_at": null
      },
      "status": {
        "state": "in_progress",
        "progress": 25,
        "started_at": "2026-05-31T03:05:00Z",
        "completed_at": null,
        "blocked_reason": null,
        "last_message_id": "msg_started_001"
      }
    }
  ],
  "next_cursor": null
}
```

## 7. `GET /api/tasks/:id`

Gets one task. Optional expansions include lifecycle events and related messages.

### Request

```http
GET /api/tasks/task_control_002_state_machine?include=events,messages
```

### Storage

- Read one row from `agent_tasks`.
- If `events` is requested, read `agent_task_events` by `task_id`.
- If `messages` is requested, read linked messages through `agent_message_task_links`.

### Response

```json
{
  "ok": true,
  "item": {
    "api_version": "agentmessage.io/v1",
    "kind": "AgentTask",
    "metadata": {
      "id": "task_control_002_state_machine",
      "project_id": "agentgrid",
      "created_by": "architect-agent",
      "assigned_to": ["control-agent"],
      "created_at": "2026-05-31T03:00:00Z",
      "updated_at": "2026-05-31T03:05:00Z",
      "correlation_id": null
    },
    "spec": {
      "title": "实现任务状态机",
      "summary": "实现 Hub 任务状态机和消息投影。",
      "owner": "control-agent",
      "priority": "p0",
      "inputs": ["docs/hub-storage.md"],
      "outputs": ["crates/agentgrid-control/src/tasks.rs"],
      "acceptance_criteria": ["状态变化写入 agent_tasks"],
      "labels": ["control", "hub"],
      "depends_on": [],
      "due_at": null
    },
    "status": {
      "state": "in_progress",
      "progress": 25,
      "started_at": "2026-05-31T03:05:00Z",
      "completed_at": null,
      "blocked_reason": null,
      "last_message_id": "msg_started_001"
    }
  },
  "events": [
    {
      "id": "evt_started_001",
      "type": "task.started",
      "from_state": "assigned",
      "to_state": "in_progress",
      "progress": 1,
      "actor": "control-agent",
      "message_id": "msg_started_001",
      "created_at": "2026-05-31T03:05:00Z"
    }
  ],
  "messages": [
    {
      "id": "msg_started_001",
      "from": "control-agent",
      "to": ["architect-agent"],
      "type": "task.started",
      "subject": "开始处理 task_control_002_state_machine",
      "created_at": "2026-05-31T03:05:00Z"
    }
  ]
}
```

## 8. `POST /api/tasks/:id/accept`

Accepts an assigned task and starts work.

### Request

```http
POST /api/tasks/task_control_002_state_machine/accept
Content-Type: application/json
```

```json
{
  "actor": "control-agent",
  "summary": "我已接收任务，开始实现任务状态机。",
  "notify": ["architect-agent"],
  "progress": 1
}
```

### State And Storage

| Step | Write |
|---|---|
| `agent_tasks` | Change `assigned`, `todo`, or `blocked` to `in_progress`; set `started_at` if empty; clear `blocked_reason`; set `progress = max(current, request.progress, 1)`; update `last_message_id`. |
| `agent_messages` | Create `task.started`. |
| `agent_message_task_links` | Link with relation `status_update`. |
| `agent_task_events` | Append `task.started` with previous and next state. |

Invalid transitions:

```text
done -> in_progress
cancelled -> in_progress
```

### Response

```json
{
  "ok": true,
  "item": {
    "api_version": "agentmessage.io/v1",
    "kind": "AgentTask",
    "metadata": {
      "id": "task_control_002_state_machine",
      "project_id": "agentgrid",
      "created_by": "architect-agent",
      "assigned_to": ["control-agent"],
      "created_at": "2026-05-31T03:00:00Z",
      "updated_at": "2026-05-31T03:05:00Z",
      "correlation_id": null
    },
    "spec": {
      "title": "实现任务状态机",
      "summary": "实现 Hub 任务状态机和消息投影。",
      "owner": "control-agent",
      "priority": "p0",
      "inputs": ["docs/hub-storage.md"],
      "outputs": ["crates/agentgrid-control/src/tasks.rs"],
      "acceptance_criteria": ["状态变化写入 agent_tasks"],
      "labels": ["control", "hub"],
      "depends_on": [],
      "due_at": null
    },
    "status": {
      "state": "in_progress",
      "progress": 1,
      "started_at": "2026-05-31T03:05:00Z",
      "completed_at": null,
      "blocked_reason": null,
      "last_message_id": "msg_started_001"
    }
  },
  "message_id": "msg_started_001",
  "event_id": "evt_started_001"
}
```

## 9. `POST /api/tasks/:id/progress`

Reports meaningful progress.

### Request

```http
POST /api/tasks/task_control_002_state_machine/progress
Content-Type: application/json
```

```json
{
  "actor": "control-agent",
  "progress": 60,
  "state": "in_progress",
  "summary": "任务状态机和消息投影逻辑已完成，正在补测试。",
  "files": ["crates/agentgrid-control/src/tasks.rs"],
  "notify": ["architect-agent"]
}
```

### State And Storage

| Step | Write |
|---|---|
| `agent_tasks` | Set `progress`; set `state` when supplied and valid, otherwise keep current state; update `updated_at` and `last_message_id`. |
| `agent_messages` | Create `task.progress`. Payload includes `task_id`, `state`, `progress`, and optional `files`. |
| `agent_message_task_links` | Link with relation `status_update`. |
| `agent_task_events` | Append `task.progress`. |

Validation:

- `progress` must be between `0` and `100`.
- `state` may be `in_progress`, `blocked`, `review`, or `testing`.
- `done` must use `/complete`; `cancelled` should use a later cancel endpoint.

### Response

```json
{
  "ok": true,
  "item": {
    "api_version": "agentmessage.io/v1",
    "kind": "AgentTask",
    "metadata": {
      "id": "task_control_002_state_machine",
      "project_id": "agentgrid",
      "created_by": "architect-agent",
      "assigned_to": ["control-agent"],
      "created_at": "2026-05-31T03:00:00Z",
      "updated_at": "2026-05-31T03:30:00Z",
      "correlation_id": null
    },
    "spec": {
      "title": "实现任务状态机",
      "summary": "实现 Hub 任务状态机和消息投影。",
      "owner": "control-agent",
      "priority": "p0",
      "inputs": ["docs/hub-storage.md"],
      "outputs": ["crates/agentgrid-control/src/tasks.rs"],
      "acceptance_criteria": ["状态变化写入 agent_tasks"],
      "labels": ["control", "hub"],
      "depends_on": [],
      "due_at": null
    },
    "status": {
      "state": "in_progress",
      "progress": 60,
      "started_at": "2026-05-31T03:05:00Z",
      "completed_at": null,
      "blocked_reason": null,
      "last_message_id": "msg_progress_001"
    }
  },
  "message_id": "msg_progress_001",
  "event_id": "evt_progress_001"
}
```

## 10. `POST /api/tasks/:id/block`

Marks work as blocked.

### Request

```http
POST /api/tasks/task_control_002_state_machine/block
Content-Type: application/json
```

```json
{
  "actor": "control-agent",
  "reason": "需要 store-agent 确认 agent_task_events 的 event_type 枚举。",
  "summary": "状态机实现被事件类型枚举阻塞。",
  "progress": 40,
  "notify": ["architect-agent", "store-agent"],
  "needs": ["确认 event_type 是否允许 review.requested"]
}
```

### State And Storage

| Step | Write |
|---|---|
| `agent_tasks` | Set `state = blocked`; set `blocked_reason`; preserve progress unless request includes `progress`; update `updated_at` and `last_message_id`. |
| `agent_messages` | Create `task.blocked`. Payload includes `reason` and optional `needs`. |
| `agent_message_task_links` | Link with relation `blocker`. |
| `agent_task_events` | Append `task.blocked`. |

### Response

```json
{
  "ok": true,
  "item": {
    "api_version": "agentmessage.io/v1",
    "kind": "AgentTask",
    "metadata": {
      "id": "task_control_002_state_machine",
      "project_id": "agentgrid",
      "created_by": "architect-agent",
      "assigned_to": ["control-agent"],
      "created_at": "2026-05-31T03:00:00Z",
      "updated_at": "2026-05-31T03:20:00Z",
      "correlation_id": null
    },
    "spec": {
      "title": "实现任务状态机",
      "summary": "实现 Hub 任务状态机和消息投影。",
      "owner": "control-agent",
      "priority": "p0",
      "inputs": ["docs/hub-storage.md"],
      "outputs": ["crates/agentgrid-control/src/tasks.rs"],
      "acceptance_criteria": ["状态变化写入 agent_tasks"],
      "labels": ["control", "hub"],
      "depends_on": [],
      "due_at": null
    },
    "status": {
      "state": "blocked",
      "progress": 40,
      "started_at": "2026-05-31T03:05:00Z",
      "completed_at": null,
      "blocked_reason": "需要 store-agent 确认 agent_task_events 的 event_type 枚举。",
      "last_message_id": "msg_blocked_001"
    }
  },
  "message_id": "msg_blocked_001",
  "event_id": "evt_blocked_001"
}
```

## 11. `POST /api/tasks/:id/complete`

Marks output as complete. By default, completion moves the task to `review`. It moves directly to `done` only when the caller explicitly marks it accepted.

### Request

```http
POST /api/tasks/task_control_002_state_machine/complete
Content-Type: application/json
```

```json
{
  "actor": "control-agent",
  "summary": "任务状态机已完成，请审查。",
  "outputs": ["crates/agentgrid-control/src/tasks.rs"],
  "checks": ["cargo test -p agentgrid-control tasks 通过"],
  "notify": ["architect-agent", "review-agent"],
  "requires_ack": true,
  "accepted": false
}
```

### State And Storage

| Step | Write |
|---|---|
| `agent_tasks` | If `accepted = true`, set `state = done` and `completed_at = now`; otherwise set `state = review`. Always set `progress = 100`, clear `blocked_reason`, update `updated_at`, update `last_message_id`. |
| `agent_messages` | Create `task.completed`. Payload includes `outputs`, `checks`, `state`, and `progress = 100`. |
| `agent_message_task_links` | Link with relation `completion`. |
| `agent_task_events` | Append `task.completed`. Use `to_state = review` or `done`. |

### Response

```json
{
  "ok": true,
  "item": {
    "api_version": "agentmessage.io/v1",
    "kind": "AgentTask",
    "metadata": {
      "id": "task_control_002_state_machine",
      "project_id": "agentgrid",
      "created_by": "architect-agent",
      "assigned_to": ["control-agent"],
      "created_at": "2026-05-31T03:00:00Z",
      "updated_at": "2026-05-31T04:00:00Z",
      "correlation_id": null
    },
    "spec": {
      "title": "实现任务状态机",
      "summary": "实现 Hub 任务状态机和消息投影。",
      "owner": "control-agent",
      "priority": "p0",
      "inputs": ["docs/hub-storage.md"],
      "outputs": ["crates/agentgrid-control/src/tasks.rs"],
      "acceptance_criteria": ["状态变化写入 agent_tasks"],
      "labels": ["control", "hub"],
      "depends_on": [],
      "due_at": null
    },
    "status": {
      "state": "review",
      "progress": 100,
      "started_at": "2026-05-31T03:05:00Z",
      "completed_at": null,
      "blocked_reason": null,
      "last_message_id": "msg_completed_001"
    }
  },
  "message_id": "msg_completed_001",
  "event_id": "evt_completed_001"
}
```

## 12. `POST /api/tasks/:id/review-request`

This endpoint is optional for the first implementation, but it keeps review transitions explicit when a task is ready for code or document review before completion is accepted.

### Request

```http
POST /api/tasks/task_control_002_state_machine/review-request
Content-Type: application/json
```

```json
{
  "actor": "control-agent",
  "summary": "请审查任务状态机实现。",
  "reviewers": ["review-agent"],
  "review_targets": [
    {
      "files": ["crates/agentgrid-control/src/tasks.rs"]
    }
  ]
}
```

### State And Storage

| Step | Write |
|---|---|
| `agent_tasks` | Set `state = review`; set `updated_at`; update `last_message_id`. |
| `agent_messages` | Create `review.requested`. |
| `agent_message_task_links` | Link with relation `review`. |
| `agent_task_events` | Append `task.review_requested`. |

## 13. Message Type Mapping

| Endpoint | Generated message type | Event type | Default state |
|---|---|---|---|
| `POST /api/tasks` | `task.assigned` when assigned | `task.created`, `task.assigned` | `todo` or `assigned` |
| `POST /api/tasks/:id/accept` | `task.started` | `task.started` | `in_progress` |
| `POST /api/tasks/:id/progress` | `task.progress` | `task.progress` | Keep current or request state |
| `POST /api/tasks/:id/block` | `task.blocked` | `task.blocked` | `blocked` |
| `POST /api/tasks/:id/complete` | `task.completed` | `task.completed` | `review` or `done` |
| `POST /api/tasks/:id/review-request` | `review.requested` | `task.review_requested` | `review` |

## 14. Implementation Split

### `agentgrid-api`

- Parse HTTP requests and query parameters.
- Validate required fields and JSON shape.
- Convert route calls to control-plane commands.
- Return the common response and error shapes.

### `agentgrid-control`

- Own task lifecycle transitions.
- Enforce valid state transitions.
- Decide generated message type, recipients, subject, and summary defaults.
- Ensure each state mutation produces exactly one event.
- Keep AgentTask v1 compatibility during the transition from message-only MVP.

### `agentgrid-store`

- Persist `agent_tasks`, `agent_messages`, `agent_task_events`, and `agent_message_task_links`.
- Provide idempotent create/update operations.
- Backfill tasks and links from historical `AgentMessage` rows.
- Keep raw message payloads unchanged while projecting task fields into `agent_tasks`.

## 15. First Implementation Order

1. Add storage tables and backfill from existing `task.assigned` messages.
2. Add read endpoints: `GET /api/tasks`, then `GET /api/tasks/:id`.
3. Add `POST /api/tasks` and generate `task.assigned`.
4. Add lifecycle endpoints: `accept`, `progress`, `block`, `complete`.
5. Add optional `review-request` after the core lifecycle is stable.
6. Update `docs/hub-api.md` to link to this document once the endpoints exist.
