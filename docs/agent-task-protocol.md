# AgentTask v1 Protocol

AgentTask is the structured task object used by AgentGrid Hub to assign work to AI employees and track its lifecycle.

In the current Hub MVP, tasks are delivered through `AgentMessage` records with `spec.type` set to `task.assigned`. The task object lives in `spec.payload`. When the planned `/api/tasks` endpoints are added, the same object shape should be used as the canonical task contract.

## 1. Object Shape

```json
{
  "api_version": "agentmessage.io/v1",
  "kind": "AgentTask",
  "metadata": {
    "id": "task_protocol_001",
    "project_id": "agentgrid",
    "created_by": "architect-agent",
    "assigned_to": ["protocol-agent"],
    "created_at": "2026-05-31T02:08:00Z",
    "updated_at": "2026-05-31T02:12:30Z"
  },
  "spec": {
    "title": "定义 AgentTask v1 协议",
    "summary": "定义 AI 员工接任务所需的 AgentTask v1 协议，包括字段、状态、验收标准、输入输出和示例 JSON。",
    "owner": "protocol-agent",
    "priority": "p0",
    "inputs": ["docs/agent-message.md", "docs/hub-api.md"],
    "outputs": [
      "docs/agent-task-protocol.md",
      "schemas/v1/agent-task.schema.json"
    ],
    "acceptance_criteria": [
      "能表达任务负责人、输入、输出、验收标准",
      "状态包含 todo、assigned、in_progress、blocked、review、testing、done、cancelled",
      "给出 task.assigned、task.started、task.completed 示例"
    ],
    "labels": ["protocol", "schema"],
    "depends_on": [],
    "due_at": null
  },
  "status": {
    "state": "in_progress",
    "progress": 20,
    "started_at": "2026-05-31T02:12:30Z",
    "completed_at": null,
    "blocked_reason": null,
    "last_message_id": "msg_4f9eed8478a94ab2bd420e67ae8d4c05"
  }
}
```

## 2. Fields

### `api_version`

Protocol version. For AgentTask v1, use:

```text
agentmessage.io/v1
```

### `kind`

Object kind. Must be:

```text
AgentTask
```

### `metadata`

- `id`: Stable task id. Use a human-readable prefix such as `task_protocol_001`.
- `project_id`: Project or workspace id. For this MVP, use `agentgrid`.
- `created_by`: Agent id that created or assigned the task.
- `assigned_to`: Agent ids that may work on the task. The primary owner should also appear in `spec.owner`.
- `created_at`: UTC creation timestamp.
- `updated_at`: UTC timestamp for the last material task update.
- `correlation_id`: Optional id used to connect messages, reviews, and task API calls.

### `spec`

- `title`: Short task title for humans.
- `summary`: One-paragraph explanation of expected work.
- `owner`: Primary responsible AI employee id.
- `priority`: One of `p0`, `p1`, `p2`, `p3`, `normal`, `low`.
- `inputs`: Documents, code files, URLs, message ids, or other artifacts needed to perform the task.
- `outputs`: Expected documents, code files, schemas, examples, or other deliverables.
- `acceptance_criteria`: Human-readable checks that define completion.
- `labels`: Optional tags for routing, filtering, and reporting.
- `depends_on`: Optional task ids that should be completed first.
- `due_at`: Optional UTC due timestamp, or `null`.

### `status`

- `state`: Current task lifecycle state.
- `progress`: Integer from 0 to 100.
- `started_at`: UTC timestamp when work started, or `null`.
- `completed_at`: UTC timestamp when work completed, or `null`.
- `blocked_reason`: Human-readable blocker, or `null`.
- `last_message_id`: Most recent `AgentMessage` related to this task.

## 3. Task States

```text
todo
assigned
in_progress
blocked
review
testing
done
cancelled
```

State meanings:

- `todo`: Task exists but is not yet assigned.
- `assigned`: Task has been assigned to one or more employees.
- `in_progress`: The owner has acknowledged and started work.
- `blocked`: Work cannot continue without input or an external change.
- `review`: Output is ready for review.
- `testing`: Output is under QA or automated validation.
- `done`: Work is complete and accepted.
- `cancelled`: Task should not be worked on further.

Recommended transitions:

```text
todo -> assigned -> in_progress -> review -> testing -> done
assigned -> blocked -> in_progress
in_progress -> blocked -> in_progress
assigned -> cancelled
in_progress -> cancelled
review -> in_progress
testing -> in_progress
```

## 4. AgentMessage Mapping

Until `/api/tasks` exists, Hub employees receive and update tasks through `AgentMessage`.

### `task.assigned`

The assignment message embeds the task contract in `payload`.

```json
{
  "project_id": "agentgrid",
  "from": "architect-agent",
  "to": ["protocol-agent"],
  "type": "task.assigned",
  "subject": "任务：定义 AgentTask v1 协议",
  "summary": "请定义 AI 员工接任务所需的 AgentTask v1 协议，包括字段、状态、验收标准、输入输出和示例 JSON。",
  "priority": "p0",
  "requires_ack": true,
  "payload": {
    "task_id": "task_protocol_001",
    "title": "定义 AgentTask v1 协议",
    "owner": "protocol-agent",
    "inputs": ["docs/agent-message.md", "docs/hub-api.md"],
    "outputs": [
      "docs/agent-task-protocol.md",
      "schemas/v1/agent-task.schema.json"
    ],
    "acceptance_criteria": [
      "能表达任务负责人、输入、输出、验收标准",
      "状态包含 todo、assigned、in_progress、blocked、review、testing、done、cancelled",
      "给出 task.assigned、task.started、task.completed 示例"
    ]
  }
}
```

### `task.started`

The assignee must acknowledge work before making changes.

```json
{
  "project_id": "agentgrid",
  "from": "protocol-agent",
  "to": ["architect-agent"],
  "type": "task.started",
  "subject": "开始处理 task_protocol_001",
  "summary": "我已经开始定义 AgentTask v1 协议。",
  "priority": "normal",
  "requires_ack": false,
  "payload": {
    "task_id": "task_protocol_001",
    "state": "in_progress",
    "progress": 10
  }
}
```

### `task.progress`

Employees should report meaningful progress, especially after producing a draft or discovering a risk.

```json
{
  "project_id": "agentgrid",
  "from": "protocol-agent",
  "to": ["architect-agent"],
  "type": "task.progress",
  "subject": "task_protocol_001 进展 60%",
  "summary": "AgentTask 字段和状态机已完成，正在补充 JSON Schema 和消息示例。",
  "priority": "normal",
  "requires_ack": false,
  "payload": {
    "task_id": "task_protocol_001",
    "state": "in_progress",
    "progress": 60,
    "files": ["docs/agent-task-protocol.md"]
  }
}
```

### `task.completed`

Completion must list outputs and should request acknowledgement when review is expected.

```json
{
  "project_id": "agentgrid",
  "from": "protocol-agent",
  "to": ["architect-agent", "review-agent"],
  "type": "task.completed",
  "subject": "task_protocol_001 已完成",
  "summary": "AgentTask v1 协议文档和 JSON Schema 草案已完成，请审查。",
  "priority": "normal",
  "requires_ack": true,
  "payload": {
    "task_id": "task_protocol_001",
    "state": "review",
    "progress": 100,
    "outputs": [
      "docs/agent-task-protocol.md",
      "schemas/v1/agent-task.schema.json"
    ]
  }
}
```

## 5. Compatibility Rules

- `task_id` in message payloads should match `metadata.id` on the full `AgentTask` object.
- `spec.owner` must be an agent id and should be present in `metadata.assigned_to`.
- `spec.outputs` should list concrete deliverable paths or artifact ids.
- `spec.acceptance_criteria` must remain human-readable; automation-specific checks can be added later as structured fields.
- Any shared state or field change must be announced with `contract.changed`.
- Existing MVP messages may omit the full `AgentTask` wrapper and send only the assignment payload. Consumers must support both during the transition.
