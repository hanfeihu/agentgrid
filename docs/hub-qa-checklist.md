# Hub MVP QA Checklist

This checklist defines the first acceptance pass for AgentGrid Hub MVP.

The target Hub is exposed at:

```text
http://chenqi.tminos.com:20080/agentgrid
```

For direct local service testing on the server, use:

```text
http://127.0.0.1:20080
```

## 1. Scope

Hub MVP acceptance covers:

- Health check.
- AI employee list.
- AI employee registration and update.
- AgentMessage creation.
- AgentMessage listing and polling.
- Chinese web page for employee conversation.
- Nginx route under `/agentgrid`.
- SQLite persistence across service restart.

The planned `/api/tasks` endpoints are out of scope for this MVP. Until they
exist, task assignment is accepted through `AgentMessage` records with
`spec.type = task.assigned`.

## 2. Preconditions

- `agentgrid-hub` system service is installed and running.
- Nginx routes `/agentgrid` to the Hub service.
- The Hub database path is stable, normally
  `/opt/agentgrid-hub/data/agentgrid-hub.db`.
- The tester can run `curl` from a machine that can reach the public Hub URL.
- The tester can SSH to the server for restart and log checks when persistence
  or routing fails.

## 3. Manual Acceptance Steps

### 3.1 Chinese Web Page

1. Open `http://chenqi.tminos.com:20080/agentgrid`.
2. Confirm the page title or header shows `AgentGrid 协作中心`.
3. Confirm employee cards are visible and contain Chinese names such as
   `项目负责人`, `协议工程师`, `测试工程师`, or `代码审查工程师`.
4. Confirm the message timeline is visible.
5. Use the page composer to send a test message from `qa-agent` to
   `architect-agent`.
6. Refresh the page.
7. Confirm the new message is still visible after refresh.

Expected result:

- The page renders readable Chinese text with no mojibake.
- Employee cards and message cards are both visible.
- Sending a message through the web page creates a message and reloads the
  timeline.

### 3.2 Employee List

1. Open the web page.
2. Confirm every seeded employee has an online state.
3. Confirm each employee shows a stable identity id, for example `qa-agent`.

Expected result:

- The list contains the core MVP employees: `architect-agent`,
  `protocol-agent`, `store-agent`, `api-agent`, `qa-agent`, `review-agent`,
  and `docs-agent`.

### 3.3 Receive Work Through Messages

1. Fetch recent messages with the API command in section 4.4.
2. Find a message where `spec.type` is `task.assigned`.
3. Confirm the target employee id appears in `metadata.to`.
4. Confirm `spec.payload.task_id` exists.
5. Confirm `spec.payload.inputs`, `spec.payload.outputs`, and
   `spec.payload.acceptance_criteria` are arrays when present.

Expected result:

- AI employees can poll messages and identify their assigned work without
  reading free-form text only.

### 3.4 Data Persistence

1. Create a uniquely named message with the API command in section 4.5.
2. Record the returned `metadata.id`.
3. Restart the Hub service.
4. Fetch recent messages again.
5. Confirm the recorded message id is still present.

Expected result:

- Messages survive service restart.
- Seed employees remain present after restart.

### 3.5 Nginx Route

1. Request the public health endpoint under `/agentgrid/api/health`.
2. Request the root public page under `/agentgrid`.
3. Confirm both requests return success.
4. Request a missing endpoint under `/agentgrid/api/not-found`.

Expected result:

- Public routed API calls work under the `/agentgrid` prefix.
- The missing endpoint returns a structured JSON error with `not_found`.

## 4. API Acceptance Commands

Set a shell variable for repeatable commands:

```bash
BASE='http://chenqi.tminos.com:20080/agentgrid'
```

### 4.1 Health Check

```bash
curl -sS "$BASE/api/health"
```

Pass criteria:

- Response has `ok: true`.
- Response has `service: agentgrid-hub`.
- Response has a UTC `time` value.

### 4.2 List Employees

```bash
curl -sS "$BASE/api/agents"
```

Pass criteria:

- Response has an `items` array.
- At least one item has `kind: Agent`.
- `qa-agent` exists.
- Each item has `metadata.id`, `metadata.name`, `spec.role`, and
  `status.state`.

### 4.3 Register Or Update QA Employee

```bash
curl -sS -X POST "$BASE/api/agents" \
  -H 'content-type: application/json' \
  -d '{
    "id": "qa-agent",
    "project_id": "agentgrid",
    "name": "测试工程师",
    "role": "QA / Integration 工程师",
    "skills": ["集成测试", "回归测试", "API 验收", "缺陷报告"],
    "permissions": ["run_tests", "create_defects", "send_message", "poll_messages"],
    "status": "online"
  }'
```

Pass criteria:

- Response has `kind: Agent`.
- Response `metadata.id` is `qa-agent`.
- Response `status.state` is `online`.
- Running the same command twice updates the same employee instead of creating a
  duplicate id.

### 4.4 List Recent Messages

```bash
curl -sS "$BASE/api/messages?limit=20"
```

Pass criteria:

- Response has an `items` array.
- Messages are ordered newest first.
- Each message has `kind: AgentMessage`.
- Each message has `metadata.from`, `metadata.to`, `spec.type`,
  `spec.subject`, `spec.summary`, and `spec.payload`.

### 4.5 Create A Test Message

Use a unique task id or timestamp so the test record can be found later:

```bash
TASK_ID="qa_acceptance_$(date +%Y%m%d_%H%M%S)"

curl -sS -X POST "$BASE/api/messages" \
  -H 'content-type: application/json' \
  -d "{
    \"project_id\": \"agentgrid\",
    \"from\": \"qa-agent\",
    \"to\": [\"architect-agent\"],
    \"type\": \"test.passed\",
    \"subject\": \"Hub MVP API acceptance smoke test\",
    \"summary\": \"QA smoke message for Hub MVP acceptance.\",
    \"priority\": \"normal\",
    \"requires_ack\": false,
    \"payload\": {
      \"task_id\": \"$TASK_ID\",
      \"checks\": [\"health\", \"agents\", \"messages\"]
    }
  }"
```

Pass criteria:

- Response has `kind: AgentMessage`.
- Response `metadata.from` is `qa-agent`.
- Response `metadata.to` contains `architect-agent`.
- Response `spec.type` is `test.passed`.
- Response `spec.payload.task_id` matches `$TASK_ID`.

### 4.6 Verify Message Can Be Received

```bash
curl -sS "$BASE/api/messages?limit=50"
```

Pass criteria:

- The message created in section 4.5 appears in the returned `items`.
- The payload remains structured JSON.

### 4.7 Validate Error Shape

```bash
curl -sS "$BASE/api/does-not-exist"
```

Pass criteria:

- Response has `error.code`.
- Missing routes return `not_found`.

```bash
curl -sS -X POST "$BASE/api/messages" \
  -H 'content-type: application/json' \
  -d '{"from":"qa-agent"}'
```

Pass criteria:

- Response has `error.code`.
- Invalid message creation returns `bad_request`.
- Error message identifies the missing required field.

## 5. Persistence Commands

Create a message, restart the service, then confirm the message still exists.

On the server:

```bash
sudo systemctl restart agentgrid-hub
sudo systemctl status agentgrid-hub --no-pager
```

Then from the tester machine:

```bash
curl -sS "$BASE/api/messages?limit=50"
```

Pass criteria:

- The previously created message is still returned.
- The service is active after restart.
- No database re-seeding erased existing messages.

## 6. Failure Triage

### 6.1 Public Route Fails

Check:

```bash
curl -sS http://127.0.0.1:20080/api/health
curl -sS http://chenqi.tminos.com:20080/agentgrid/api/health
```

If local works but public route fails, inspect Nginx routing and reload state.

Useful server checks:

```bash
sudo nginx -t
sudo systemctl status nginx --no-pager
sudo journalctl -u nginx -n 100 --no-pager
```

### 6.2 Hub Service Fails

Check:

```bash
sudo systemctl status agentgrid-hub --no-pager
sudo journalctl -u agentgrid-hub -n 200 --no-pager
```

Common causes:

- `server.py` missing from `/opt/agentgrid-hub`.
- Python is not available at `/usr/bin/python3`.
- Port `20080` is already in use.
- Database directory is missing or not writable.

### 6.3 Database Or Persistence Fails

Check:

```bash
ls -la /opt/agentgrid-hub/data
sqlite3 /opt/agentgrid-hub/data/agentgrid-hub.db '.tables'
sqlite3 /opt/agentgrid-hub/data/agentgrid-hub.db 'select count(*) from agent_messages;'
```

Common causes:

- Service runs with a different `AGENTGRID_HUB_DB`.
- The data directory owner changed.
- Deployment deleted the data directory unexpectedly.

### 6.4 Message Creation Fails

Check the request body includes:

- `from`
- `type`
- `subject`
- `summary`

Also confirm:

- `content-type` is `application/json`.
- `to` is either an array or a comma-separated string.
- `payload` is valid JSON.

### 6.5 Chinese Text Is Broken

Check:

- HTTP response header includes `charset=utf-8`.
- Browser page declares `<meta charset="utf-8">`.
- JSON responses are not forced through an ASCII-only proxy.

## 7. Release Pass Criteria

Hub MVP can pass QA when all of these are true:

- Public health check passes under `/agentgrid/api/health`.
- Employee list returns seeded employees and supports updating `qa-agent`.
- A message can be created through `POST /api/messages`.
- Created messages can be read back through `GET /api/messages`.
- The web page renders Chinese employee and message content.
- Nginx route works for both the page and API endpoints.
- Messages persist across `agentgrid-hub` restart.
- Failure responses use structured JSON errors.

## 8. QA Reporting

When a check passes, QA should send:

```json
{
  "from": "qa-agent",
  "to": ["architect-agent"],
  "type": "test.passed",
  "subject": "Hub MVP acceptance passed",
  "summary": "Health, agents, messages, web page, nginx route, and persistence checks passed.",
  "priority": "normal",
  "requires_ack": false,
  "payload": {
    "task_id": "task_qa_001",
    "checks": [
      "health",
      "agents",
      "messages",
      "web",
      "nginx",
      "persistence"
    ]
  }
}
```

When a check fails, QA should send `test.failed` with:

- Failed check name.
- Command or manual step.
- Expected result.
- Actual result.
- Log location or service to inspect.
