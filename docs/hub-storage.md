# Hub Storage Design

This document defines the first Hub storage model for the planned `/api/tasks` endpoints.

The current Hub MVP delivers work through `AgentMessage` records. The next version should keep that message stream as the collaboration log, while adding `agent_tasks` as the canonical task state table.

## 1. Goals

- Store task identity, ownership, lifecycle state, progress, inputs, outputs, and acceptance criteria.
- Preserve every collaboration message related to a task.
- Support polling-based task and message APIs with simple SQLite queries.
- Keep task state derivable from messages, but fast to read from `agent_tasks`.
- Avoid a premature multi-database abstraction; SQLite is the first target.

## 2. Core Tables

Recommended first task-related tables:

```text
agent_tasks
agent_task_events
agent_messages
agent_message_task_links
```

`agent_tasks` owns current task state. `agent_messages` owns collaboration messages. `agent_task_events` records state changes as an auditable task log. `agent_message_task_links` connects any message to one or more tasks.

## 3. `agent_tasks`

`agent_tasks` is the canonical read model for `/api/tasks`.

```sql
CREATE TABLE agent_tasks (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL DEFAULT 'agentgrid',
  api_version TEXT NOT NULL DEFAULT 'agentmessage.io/v1',
  kind TEXT NOT NULL DEFAULT 'AgentTask',

  title TEXT NOT NULL,
  summary TEXT NOT NULL DEFAULT '',
  owner_agent_id TEXT NOT NULL,
  created_by_agent_id TEXT NOT NULL,
  priority TEXT NOT NULL DEFAULT 'normal',

  state TEXT NOT NULL DEFAULT 'todo',
  progress INTEGER NOT NULL DEFAULT 0,
  blocked_reason TEXT,

  inputs_json TEXT NOT NULL DEFAULT '[]',
  outputs_json TEXT NOT NULL DEFAULT '[]',
  acceptance_criteria_json TEXT NOT NULL DEFAULT '[]',
  labels_json TEXT NOT NULL DEFAULT '[]',
  depends_on_json TEXT NOT NULL DEFAULT '[]',

  due_at TEXT,
  assigned_at TEXT,
  started_at TEXT,
  completed_at TEXT,
  cancelled_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,

  assignment_message_id TEXT,
  last_message_id TEXT,
  correlation_id TEXT,

  CHECK (state IN (
    'todo',
    'assigned',
    'in_progress',
    'blocked',
    'review',
    'testing',
    'done',
    'cancelled'
  )),
  CHECK (progress >= 0 AND progress <= 100)
);
```

### Field Notes

- `id`: Stable task id, matching `payload.task_id` in task messages.
- `project_id`: Workspace or project namespace. The current MVP uses `agentgrid`.
- `owner_agent_id`: Primary responsible employee, matching `spec.owner`.
- `created_by_agent_id`: Agent that created or assigned the task.
- `priority`: Keep string priorities compatible with AgentMessage: `p0`, `p1`, `p2`, `p3`, `normal`, `low`.
- `state`: Current lifecycle state from `AgentTask v1`.
- `progress`: Current completion estimate, from 0 to 100.
- `inputs_json`, `outputs_json`, `acceptance_criteria_json`: JSON arrays stored as text for SQLite.
- `labels_json`, `depends_on_json`: Optional JSON arrays for filtering and dependency hints.
- `assignment_message_id`: First `task.assigned` message that created or assigned this task.
- `last_message_id`: Most recent related AgentMessage.
- `correlation_id`: Optional id for grouping API calls, messages, reviews, and future compute jobs.

Recommended indexes:

```sql
CREATE INDEX idx_agent_tasks_project_state
  ON agent_tasks (project_id, state, updated_at);

CREATE INDEX idx_agent_tasks_owner_state
  ON agent_tasks (owner_agent_id, state, updated_at);

CREATE INDEX idx_agent_tasks_priority_updated
  ON agent_tasks (priority, updated_at);
```

## 4. `agent_task_events`

`agent_task_events` is an append-only history of task lifecycle changes.

```sql
CREATE TABLE agent_task_events (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL,
  project_id TEXT NOT NULL DEFAULT 'agentgrid',
  event_type TEXT NOT NULL,
  from_state TEXT,
  to_state TEXT,
  progress INTEGER,
  actor_agent_id TEXT NOT NULL,
  message_id TEXT,
  summary TEXT NOT NULL DEFAULT '',
  payload_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,

  FOREIGN KEY (task_id) REFERENCES agent_tasks(id)
);
```

Recommended event types:

```text
task.created
task.assigned
task.started
task.progress
task.blocked
task.review_requested
task.testing
task.completed
task.cancelled
task.reopened
```

Recommended indexes:

```sql
CREATE INDEX idx_agent_task_events_task_created
  ON agent_task_events (task_id, created_at);

CREATE INDEX idx_agent_task_events_project_created
  ON agent_task_events (project_id, created_at);
```

## 5. `agent_messages`

The existing AgentMessage stream should remain the collaboration source of truth.

Recommended shape:

```sql
CREATE TABLE agent_messages (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL DEFAULT 'agentgrid',
  api_version TEXT NOT NULL DEFAULT 'agentmessage.io/v1',
  kind TEXT NOT NULL DEFAULT 'AgentMessage',

  from_agent_id TEXT NOT NULL,
  to_agent_ids_json TEXT NOT NULL DEFAULT '[]',

  type TEXT NOT NULL,
  subject TEXT NOT NULL DEFAULT '',
  summary TEXT NOT NULL DEFAULT '',
  priority TEXT NOT NULL DEFAULT 'normal',
  requires_ack INTEGER NOT NULL DEFAULT 0,
  payload_json TEXT NOT NULL DEFAULT '{}',

  created_at TEXT NOT NULL
);
```

Recommended indexes:

```sql
CREATE INDEX idx_agent_messages_project_created
  ON agent_messages (project_id, created_at);

CREATE INDEX idx_agent_messages_type_created
  ON agent_messages (type, created_at);

CREATE INDEX idx_agent_messages_from_created
  ON agent_messages (from_agent_id, created_at);
```

Because `to_agent_ids_json` is a JSON array, the first SQLite implementation can filter recipients in application code after limiting by project and time. If recipient filtering becomes slow, add a normalized table:

```sql
CREATE TABLE agent_message_recipients (
  message_id TEXT NOT NULL,
  agent_id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (message_id, agent_id),
  FOREIGN KEY (message_id) REFERENCES agent_messages(id)
);

CREATE INDEX idx_agent_message_recipients_agent_created
  ON agent_message_recipients (agent_id, created_at);
```

## 6. `agent_message_task_links`

Messages may relate to one task, multiple tasks, or no task. The link table keeps that relationship explicit.

```sql
CREATE TABLE agent_message_task_links (
  message_id TEXT NOT NULL,
  task_id TEXT NOT NULL,
  relation TEXT NOT NULL DEFAULT 'related',
  created_at TEXT NOT NULL,

  PRIMARY KEY (message_id, task_id, relation),
  FOREIGN KEY (message_id) REFERENCES agent_messages(id),
  FOREIGN KEY (task_id) REFERENCES agent_tasks(id)
);
```

Recommended relation values:

```text
assignment
status_update
completion
blocker
review
contract_change
test_result
related
```

Recommended indexes:

```sql
CREATE INDEX idx_agent_message_task_links_task
  ON agent_message_task_links (task_id, created_at);
```

## 7. Relationship Between Tasks And Messages

Task messages use `payload.task_id` to identify the task.

When Hub receives an `AgentMessage`:

1. Insert the message into `agent_messages`.
2. If `payload.task_id` exists, insert a row into `agent_message_task_links`.
3. If `type = 'task.assigned'`, create or update `agent_tasks`.
4. If `type` is a task status message, update `agent_tasks.state`, `progress`, timestamps, `blocked_reason`, and `last_message_id`.
5. Insert an append-only row into `agent_task_events`.

This preserves the raw collaboration log while keeping `/api/tasks` fast.

## 8. State Persistence Rules

Recommended state mapping from messages:

| Message type | Task state | Progress rule | Timestamp rule |
|---|---|---|---|
| `task.assigned` | `assigned` | `0` unless payload provides progress | Set `assigned_at` if empty |
| `task.started` | `in_progress` | Use payload progress, else at least `1` | Set `started_at` if empty |
| `task.progress` | Payload state or current state | Use payload progress when present | Update `updated_at` |
| `task.blocked` | `blocked` | Preserve or use payload progress | Set `blocked_reason` |
| `review.requested` | `review` | Use payload progress, often `100` | Update `updated_at` |
| `test.failed` | `in_progress` or `blocked` | Preserve progress | Link failure message |
| `test.passed` | `testing` or `done` | Preserve or set `100` | Link passing message |
| `task.completed` | `review` until accepted, then `done` | Set `100` | Set `completed_at` when accepted |

`task.completed` should initially move work to `review` when `requires_ack = true`. A later acceptance message or `/api/tasks/:id/complete` call can move it to `done`.

## 9. Planned Task API Persistence

### `POST /api/tasks`

Create a row in `agent_tasks` with state `todo` or `assigned`.

If `owner_agent_id` or assigned agents are provided, create a matching `task.assigned` AgentMessage and set:

```text
state = assigned
assignment_message_id = created message id
last_message_id = created message id
assigned_at = now
```

### `GET /api/tasks`

Read from `agent_tasks`, filtered by:

```text
project_id
owner_agent_id
state
priority
updated_at
```

### `GET /api/tasks/:id`

Read one `agent_tasks` row and optionally include:

```text
events from agent_task_events
messages via agent_message_task_links
```

### `POST /api/tasks/:id/accept`

Set:

```text
state = in_progress
started_at = now if empty
progress = max(progress, 1)
updated_at = now
```

Also create a `task.started` AgentMessage and a `task.started` event.

### `POST /api/tasks/:id/progress`

Update:

```text
state = payload.state if present, otherwise keep current state
progress = payload.progress
updated_at = now
last_message_id = created message id
```

Also create a `task.progress` AgentMessage and event.

### `POST /api/tasks/:id/block`

Update:

```text
state = blocked
blocked_reason = request reason
updated_at = now
last_message_id = created message id
```

Also create a `task.blocked` AgentMessage and event.

### `POST /api/tasks/:id/complete`

If review is required:

```text
state = review
progress = 100
updated_at = now
```

If the caller explicitly marks the task accepted:

```text
state = done
progress = 100
completed_at = now
updated_at = now
```

Also create a `task.completed` AgentMessage and event.

## 10. Migration Order

Recommended SQLite migration order:

1. Create `agent_tasks`.
2. Create `agent_task_events`.
3. Create or update `agent_messages`.
4. Create `agent_message_task_links`.
5. Optionally create `agent_message_recipients` when recipient queries need indexing.
6. Backfill `agent_tasks` from historical `task.assigned` messages that include `payload.task_id`.
7. Backfill links from all historical messages with `payload.task_id`.

Backfill should be idempotent: re-running it must not duplicate tasks, links, or events.

## 11. Storage Invariants

- Every `agent_tasks.id` must match the task id used in related message payloads.
- Every task state update must update `agent_tasks.updated_at`.
- Every task state update should append exactly one `agent_task_events` row.
- `agent_messages` must preserve the original payload JSON even when task fields are also projected into `agent_tasks`.
- `last_message_id` should always point to the newest task-related message processed by Hub.
- Shared protocol, API, or state changes must still be announced with `contract.changed`.

