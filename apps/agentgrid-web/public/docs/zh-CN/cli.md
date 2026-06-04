# CLI 命令说明

`agentgrid` 是给人、脚本和 AI 客户端使用的命令行入口。

默认连接：

```text
http://chenqi.tminos.com:20080/agentgrid
```

指定 Hub：

```bash
agentgrid --hub https://hub.example.com/agentgrid <command>
```

## 发现能力

```bash
agentgrid health
agentgrid nodes
agentgrid workbench list
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

投递到一台物理电脑/工位。Hub 会自动选择后台 Worker 通道：

```bash
agentgrid submit-command \
  --program hostname \
  --workbench sha256:091c56d7a82c696b759efbf8836b6f8e82cf507deeda35e170fc42134a2caece \
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

## Workbench / 电脑操作

当一次操作属于某台真实电脑或工位时，优先使用 `workbench`。Hub 会自动选择正确的能力通道。

```bash
agentgrid workbench list
agentgrid workbench get --id sha256:091c56d7a82c696b759efbf8836b6f8e82cf507deeda35e170fc42134a2caece
agentgrid workbench timeline --id sha256:091c56d7a82c696b759efbf8836b6f8e82cf507deeda35e170fc42134a2caece
```

```bash
agentgrid workbench command \
  --id sha256:091c56d7a82c696b759efbf8836b6f8e82cf507deeda35e170fc42134a2caece \
  --program hostname \
  --wait
```

```bash
agentgrid workbench screenshot \
  --id sha256:091c56d7a82c696b759efbf8836b6f8e82cf507deeda35e170fc42134a2caece \
  --wait
```

```bash
agentgrid workbench file \
  --id sha256:091c56d7a82c696b759efbf8836b6f8e82cf507deeda35e170fc42134a2caece \
  --operation list \
  --path "C:\\" \
  --max-entries 200 \
  --wait
```

```bash
agentgrid workbench runtime \
  --id sha256:091c56d7a82c696b759efbf8836b6f8e82cf507deeda35e170fc42134a2caece \
  --tool audio.tts.clone \
  --payload-file payload.json \
  --wait
```

```bash
agentgrid workbench action \
  --id sha256:091c56d7a82c696b759efbf8836b6f8e82cf507deeda35e170fc42134a2caece \
  --action command.run \
  --payload '{"program":"hostname","args":[],"timeout_seconds":30}' \
  --wait
```

```bash
agentgrid workbench action \
  --id source_workbench_id \
  --action port_bridge.create \
  --payload '{"target_workbench_id":"target_workbench_id","target_port":8888,"source_bind_port":9999}'
```

路由规则：

- 命令、文件、Runtime 工具任务走后台 `worker` 通道。
- 截屏任务走 `desktop` 通道。
- `workbench timeline` 可以查看这台物理电脑的操作记录。
- `workbench action` 是面向 AI 和 SDK 的标准入口：
  `workbench_id + action + payload`。

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

按工位截图。Hub 会自动选择 Desktop Helper 通道：

```bash
agentgrid submit-desktop-screenshot \
  --workbench sha256:091c56d7a82c696b759efbf8836b6f8e82cf507deeda35e170fc42134a2caece \
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

在 Windows PowerShell、CI 脚本、AI 客户端里，不建议硬拼 JSON 引号。
可以用文件、stdin 或 base64 提交结构化 JSON：

```bash
agentgrid submit-plugin \
  --plugin-id demo.hello \
  --input-file payload.json \
  --wait
```

```bash
printf '%s\n' '{"name":"AgentGrid"}' | agentgrid submit-plugin \
  --plugin-id demo.hello \
  --input-stdin \
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
agentgrid tools probe-center
agentgrid tools remediation-center
agentgrid tools remediation-runbook --id rem_docker_run_linux_worker_01
agentgrid tools remediation-action --id rem_docker_run_linux_worker_01
agentgrid tools probes
agentgrid tools probe --id command.run --node linux-worker-01
agentgrid node-tools
agentgrid node-tools get --id demo.hello
agentgrid node-tools node --node linux-worker-01
agentgrid node-tools register --node linux-worker-01 --file node-tool.json
agentgrid node-tools probe --id demo.hello --node linux-worker-01
```

`tools probe-center` 是能力验证中心，适合 AI 和人先判断“哪个工具在哪台电脑上真的可用”；`tools remediation-center` 会把失败或未完成验证转成修复建议；`node-tools` 是节点动态插件工具注册中心。

修复项会返回 `spec.diagnosis.code`、`spec.diagnosis.next_action` 和
`status.last_action_task`。AI 和人可以不用解析原始 stdout/stderr，就能区分
依赖缺失、策略阻止、服务未运行、路径不可用、依赖检查已通过、检查仍在执行等结论。

使用 `tools remediation-runbook --id rem_xxx` 可以查看某一个修复项的标准
修复流程。Runbook v1 只做诊断和审计流程编排；安装软件、启动服务、修改
Worker 策略等改变机器状态的步骤，必须是显式的人工或授权 AI 动作。

示例：查看并验证 `jia-node` 上的 TTS 语音克隆工具：

```bash
agentgrid node-tools get --id audio.tts.clone
agentgrid node-tools probe --id audio.tts.clone --node jia-node
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

稳定的 Runtime payload 入口：

```bash
agentgrid runtime submit \
  --tool audio.tts.clone \
  --workbench jia-node \
  --payload-file payload.json \
  --wait
```

```bash
cat payload.json | agentgrid runtime submit \
  --tool audio.tts.clone \
  --workbench jia-node \
  --payload-stdin \
  --wait
```

```bash
agentgrid runtime submit \
  --tool audio.tts.clone \
  --workbench jia-node \
  --payload-base64 eyJ0ZXh0IjoiSGVsbG8iLCJ0aW1lb3V0X3NlY29uZHMiOjYwMH0= \
  --wait
```

`agentgrid jobs plan` 和 `agentgrid jobs submit` 也支持同一套
`--payload`、`--payload-file`、`--payload-stdin`、`--payload-base64` 标准。

`--workbench` 表示一台物理电脑/工位，`--node` 表示一个精确的
AgentGrid 通道。面向用户和 AI 的调用优先使用 `--workbench`。

## 普通 Worker / Desktop Helper 边界

一台物理电脑可以暴露两个 AgentGrid 能力通道：

| 通道 | 节点示例 | 用途 |
| --- | --- | --- |
| 后台 Worker | `CHENGCHONG-windows` | 命令、文件、软件安装、Git、Docker、会话、插件、Hub API 调用 |
| Desktop Helper | `CHENGCHONG-windows-desktop` | 截图、点击、输入、按键、前台桌面控制 |

不要把命令、文件、软件安装、插件任务交给 Desktop Helper。不要把可见桌面操作交给普通 Worker。Hub 调度器会强制执行这个通道边界。

向 `jia-node` 提交语音克隆合成任务：

```bash
agentgrid runtime submit \
  --tool audio.tts.clone \
  --node jia-node \
  --title "生成克隆音色语音" \
  --payload '{"text":"AgentGrid 可以调度 jia 节点生成语音。","timeout_seconds":600}' \
  --wait \
  --wait-timeout-seconds 900
```

可选字段包括 `reference_audio_path`、`reference_audio_base64`、
`emotion_mode`、`emotion_audio_path`、`emotion_audio_base64`、
`emotion_weight`、`happy`、`angry`、`sad`、`afraid`、`disgusted`、
`melancholic`、`surprised`、`calm`、`output_dir`，以及 `top_p`、
`top_k`、`temperature`、`max_mel_tokens` 等生成参数。

## 节点端口桥接

把 B 节点的本地端口临时桥接到 A 节点本机端口：

```bash
agentgrid bridge-port \
  --source-node a-node \
  --target-node b-node \
  --target-port 8080 \
  --source-port 18080 \
  --purpose "让 A 节点浏览器访问 B 节点 Web 调试页"
```

等价资源命令：

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

返回的 `Source URL` 需要在 A 节点本机打开。

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
