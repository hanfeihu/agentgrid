# CLI Guide

`agentgrid` is the command-line entry point for humans, scripts, and AI clients.

By default it connects to:

```text
http://127.0.0.1:20181
```

Override with:

```bash
agentgrid --hub https://hub.example.com/agentgrid <command>
```

## Discovery

```bash
agentgrid health
agentgrid nodes
agentgrid capabilities
agentgrid tools
agentgrid node-tools
agentgrid standard all
```

## Submit Simple Tasks

HTTP request:

```bash
agentgrid submit-http \
  --url https://example.com \
  --method GET \
  --wait
```

Command:

```bash
agentgrid submit-command \
  --program hostname \
  --os linux \
  --wait
```

File list:

```bash
agentgrid submit-file \
  --operation list \
  --path /tmp \
  --os linux \
  --max-entries 100 \
  --wait
```

Git:

```bash
agentgrid submit-git \
  --operation status \
  --repo-dir /path/to/repo \
  --node linux-worker-01 \
  --wait
```

Docker:

```bash
agentgrid submit-docker \
  --operation run \
  --image alpine:latest \
  --arg echo \
  --arg hello \
  --wait
```

Browser:

```bash
agentgrid submit-browser \
  --url https://example.com \
  --selector title \
  --wait
```

Desktop screenshot:

```bash
agentgrid submit-desktop-screenshot \
  --node windows-worker-01-desktop \
  --wait
```

Desktop click:

```bash
agentgrid submit-desktop-click \
  --node windows-worker-01-desktop \
  --x 400 \
  --y 320 \
  --wait
```

Desktop typing:

```bash
agentgrid submit-desktop-type-text \
  --node windows-worker-01-desktop \
  --text "hello from AgentGrid" \
  --wait
```

Desktop key:

```bash
agentgrid submit-desktop-key \
  --node windows-worker-01-desktop \
  --key Enter \
  --wait
```

AgentMessage:

```bash
agentgrid submit-agent-message \
  --from architect-agent \
  --to review-agent \
  --type task.assigned \
  --subject "Review task" \
  --summary "Please review the task result." \
  --wait
```

Plugin:

```bash
agentgrid submit-plugin \
  --plugin-id demo.hello \
  --input '{"name":"AgentGrid"}' \
  --wait
```

## Inspect Tasks

```bash
agentgrid tasks
agentgrid tasks get --id task_xxx
agentgrid tasks logs --id task_xxx
agentgrid tasks watch --id task_xxx
agentgrid tasks explain --id task_xxx
```

## Job Runtime

Dry run:

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

Submit:

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

Inspect:

```bash
agentgrid jobs
agentgrid jobs get --id job_xxx
agentgrid jobs execution --id job_xxx
agentgrid jobs recovery-scan
```

## Node Tools

```bash
agentgrid node-tools
agentgrid node-tools get --id demo.hello
agentgrid node-tools node --node linux-worker-01
agentgrid node-tools register --node linux-worker-01 --file node-tool.json
agentgrid node-tools probe --id demo.hello --node linux-worker-01
```

## Runtime API

```bash
agentgrid runtime manifest
agentgrid runtime submit \
  --tool command.run \
  --payload '{"type":"command","program":"hostname","args":[],"timeout_seconds":30}' \
  --wait
agentgrid runtime get --id task_xxx
```

## Node Port Bridge

Bridge a local port on node B to a local port on node A:

```bash
agentgrid bridge-port \
  --source-node a-node \
  --target-node b-node \
  --target-port 8080 \
  --source-port 18080 \
  --purpose "let node A browser access node B web debug page"
```

Equivalent resource commands:

```bash
agentgrid port-bridges create \
  --source-node a-node \
  --target-node b-node \
  --target-port 8080 \
  --source-port 18080
agentgrid port-bridges
agentgrid port-bridges get --id pbridge_xxx
agentgrid port-bridges close --id pbridge_xxx
```

Open the returned `Source URL` from node A.

## Webhooks

```bash
agentgrid webhooks list
agentgrid webhooks create \
  --name ci-hook \
  --url https://example.com/webhook \
  --event task.completed \
  --event task.failed
agentgrid webhooks deliveries
```
