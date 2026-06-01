# CLI 命令说明

`agentgrid` 是给人、脚本和 AI 客户端使用的命令行入口。

默认连接：

```text
http://127.0.0.1:20181
```

指定 Hub：

```bash
agentgrid --hub https://hub.example.com/agentgrid <command>
```

## 发现能力

```bash
agentgrid health
agentgrid nodes
agentgrid capabilities
agentgrid tools
agentgrid node-tools
agentgrid standard all
```

## 提交简单任务

HTTP：

```bash
agentgrid submit-http \
  --url https://example.com \
  --method GET \
  --wait
```

命令：

```bash
agentgrid submit-command \
  --program hostname \
  --os linux \
  --wait
```

文件列表：

```bash
agentgrid submit-file \
  --operation list \
  --path /tmp \
  --os linux \
  --max-entries 100 \
  --wait
```

Git：

```bash
agentgrid submit-git \
  --operation status \
  --repo-dir /path/to/repo \
  --node linux-worker-01 \
  --wait
```

Docker：

```bash
agentgrid submit-docker \
  --operation run \
  --image alpine:latest \
  --arg echo \
  --arg hello \
  --wait
```

浏览器：

```bash
agentgrid submit-browser \
  --url https://example.com \
  --selector title \
  --wait
```

桌面截图：

```bash
agentgrid submit-desktop-screenshot \
  --node windows-worker-01-desktop \
  --wait
```

桌面点击：

```bash
agentgrid submit-desktop-click \
  --node windows-worker-01-desktop \
  --x 400 \
  --y 320 \
  --wait
```

桌面输入：

```bash
agentgrid submit-desktop-type-text \
  --node windows-worker-01-desktop \
  --text "hello from AgentGrid" \
  --wait
```

桌面按键：

```bash
agentgrid submit-desktop-key \
  --node windows-worker-01-desktop \
  --key Enter \
  --wait
```

AgentMessage：

```bash
agentgrid submit-agent-message \
  --from architect-agent \
  --to review-agent \
  --type task.assigned \
  --subject "Review task" \
  --summary "Please review the task result." \
  --wait
```

插件：

```bash
agentgrid submit-plugin \
  --plugin-id demo.hello \
  --input '{"name":"AgentGrid"}' \
  --wait
```

## 查看任务

```bash
agentgrid tasks
agentgrid tasks get --id task_xxx
agentgrid tasks logs --id task_xxx
agentgrid tasks watch --id task_xxx
agentgrid tasks explain --id task_xxx
```

## Job Runtime

预检：

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

提交：

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

查看：

```bash
agentgrid jobs
agentgrid jobs get --id job_xxx
agentgrid jobs execution --id job_xxx
agentgrid jobs recovery-scan
```

## 节点工具

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

## Webhook

```bash
agentgrid webhooks list
agentgrid webhooks create \
  --name ci-hook \
  --url https://example.com/webhook \
  --event task.completed \
  --event task.failed
agentgrid webhooks deliveries
```

