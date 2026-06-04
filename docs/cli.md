# CLI Guide

`agentgrid` is the command-line entry point for humans, scripts, and AI clients.

By default it connects to:

```text
http://chenqi.tminos.com:20080/agentgrid
```

Override with:

```bash
agentgrid --hub https://hub.example.com/agentgrid <command>
```

## Discovery

```bash
agentgrid health
agentgrid nodes
agentgrid workbench list
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

Command on a physical computer/workbench. Hub chooses the background worker
channel automatically:

```bash
agentgrid submit-command \
  --program hostname \
  --workbench sha256:091c56d7a82c696b759efbf8836b6f8e82cf507deeda35e170fc42134a2caece \
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

## Workbench / Computer Operations

Use `workbench` when the operation belongs to a real physical computer or
station. Hub selects the correct channel automatically.

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

Routing:

- Command, file, and runtime tasks use the background `worker` channel.
- Screenshot tasks use the `desktop` channel.
- `workbench timeline` shows the operation history of that physical computer.
- `workbench action` is the standard AI-facing entry point:
  `workbench_id + action + payload`.

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

Desktop screenshot by workbench. Hub chooses the desktop helper channel:

```bash
agentgrid submit-desktop-screenshot \
  --workbench sha256:091c56d7a82c696b759efbf8836b6f8e82cf507deeda35e170fc42134a2caece \
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

For Windows PowerShell, CI scripts, or AI clients that should not fight shell
quoting, use structured JSON inputs from files, stdin, or base64:

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
agentgrid tools probe-center
agentgrid tools remediation-center
agentgrid tools remediation-action --id rem_docker_run_linux_worker_01
agentgrid tools probes
agentgrid tools probe --id command.run --node linux-worker-01
agentgrid node-tools
agentgrid node-tools get --id demo.hello
agentgrid node-tools node --node linux-worker-01
agentgrid node-tools register --node linux-worker-01 --file node-tool.json
agentgrid node-tools probe --id demo.hello --node linux-worker-01
```

`tools probe-center` is the capability verification center. Use it before scheduling to see which tool-node edges are runtime verified. `tools remediation-center` turns failed or incomplete verification into repair guidance. `node-tools` manages dynamic plugin tool registration.

Example: inspect and verify the `jia-node` TTS voice clone tool:

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

Stable Runtime payload inputs:

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

The same `--payload`, `--payload-file`, `--payload-stdin`, and
`--payload-base64` standard is available on `agentgrid jobs plan` and
`agentgrid jobs submit`.

`--workbench` targets a physical computer or station. `--node` targets one exact
AgentGrid channel. Prefer `--workbench` for product-facing and AI-facing calls.

## Worker / Desktop Boundary

A physical computer can expose two AgentGrid channels:

| Channel | Node example | Use for |
| --- | --- | --- |
| Background Worker | `CHENGCHONG-windows` | commands, files, software install, Git, Docker, sessions, plugins, Hub API calls |
| Desktop Helper | `CHENGCHONG-windows-desktop` | screenshots, clicks, text input, key presses, foreground desktop control |

Do not submit command/file/software/plugin work to Desktop Helper nodes. Do not
submit visible desktop operations to background Worker nodes. The Hub scheduler
enforces this channel boundary.

Submit a voice clone TTS task to `jia-node`:

```bash
agentgrid runtime submit \
  --tool audio.tts.clone \
  --node jia-node \
  --title "Generate cloned voice audio" \
  --payload '{"text":"AgentGrid can schedule voice synthesis on jia-node.","timeout_seconds":600}' \
  --wait \
  --wait-timeout-seconds 900
```

Optional payload fields include `reference_audio_path`, `reference_audio_base64`,
`emotion_mode`, `emotion_audio_path`, `emotion_audio_base64`,
`emotion_weight`, `happy`, `angry`, `sad`, `afraid`, `disgusted`,
`melancholic`, `surprised`, `calm`, `output_dir`, and generation parameters
such as `top_p`, `top_k`, `temperature`, and `max_mel_tokens`.

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
