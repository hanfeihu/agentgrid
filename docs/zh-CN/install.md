# 安装

这份文档用于本地开发安装和 Worker 节点安装。

## 依赖

- Rust stable
- Node.js 20+
- Git
- Linux / macOS / Windows Worker 节点

## 从源码构建

```bash
git clone https://github.com/hanfeihu/agentgrid.git
cd agentgrid
cargo check -p agentgrid-hub -p agentgrid-worker-app -p agentgrid-cli -p agentgrid-mcp
npm --prefix apps/agentgrid-web install
npm --prefix apps/agentgrid-web run build
```

## 从 Release 安装

Linux 和 macOS：

```bash
curl -fsSL https://raw.githubusercontent.com/hanfeihu/agentgrid/main/scripts/install.sh | bash
```

Windows PowerShell，用管理员权限运行：

```powershell
irm "https://raw.githubusercontent.com/hanfeihu/agentgrid/main/scripts/install.ps1" | iex
```

Linux 或 macOS 安装并启动 Worker 服务：

```bash
curl -fsSL https://raw.githubusercontent.com/hanfeihu/agentgrid/main/scripts/install.sh | \
  AGENTGRID_INSTALL_WORKER_SERVICE=1 \
  AGENTGRID_HUB_URL=https://hub.example.com/agentgrid \
  AGENTGRID_JOIN_TOKEN=agj_replace_me \
  bash
```

Windows 安装并启动 Worker 计划任务：

```powershell
irm "https://raw.githubusercontent.com/hanfeihu/agentgrid/main/scripts/install.ps1" -OutFile "$env:TEMP\agentgrid-install.ps1"
& "$env:TEMP\agentgrid-install.ps1" -HubUrl "https://hub.example.com/agentgrid" -JoinToken "agj_replace_me" -InstallWorker
```

## 本地启动 Hub

```bash
cargo run -p agentgrid-hub -- \
  --host 127.0.0.1 \
  --port 20181 \
  --db data/agentgrid-hub.db \
  --web-dir apps/agentgrid-web/dist
```

打开：

```text
http://127.0.0.1:20181
```

如果还没有超级管理员，页面会显示初始化入口。

## 本地启动 Worker

```bash
cargo run -p agentgrid-worker-app -- \
  --hub http://127.0.0.1:20181 \
  --id local-worker \
  --name "Local Worker" \
  --capability http \
  --capability command \
  --capability file \
  --capability git \
  --max-concurrent-jobs 4
```

## Linux systemd 安装 Worker

先构建 Worker：

```bash
cargo build --release -p agentgrid-worker-app
```

安装：

```bash
AGENTGRID_HUB_URL=https://hub.example.com/agentgrid \
AGENTGRID_NODE_ID=linux-worker-01 \
AGENTGRID_NODE_NAME="Linux Worker 01" \
AGENTGRID_JOIN_TOKEN=agj_replace_me \
./scripts/install-linux-systemd.sh target/release/agentgrid-worker
```

## macOS launchd 安装 Worker

```bash
cargo build --release -p agentgrid-worker-app

AGENTGRID_HUB_URL=https://hub.example.com/agentgrid \
AGENTGRID_NODE_ID=mac-worker-01 \
AGENTGRID_NODE_NAME="Mac Worker 01" \
AGENTGRID_JOIN_TOKEN=agj_replace_me \
./scripts/install-macos-launchd.sh target/release/agentgrid-worker
```

## Windows 安装 Worker

用管理员权限打开 PowerShell：

```powershell
irm "https://raw.githubusercontent.com/hanfeihu/agentgrid/main/scripts/install.ps1" -OutFile "$env:TEMP\agentgrid-install.ps1"
& "$env:TEMP\agentgrid-install.ps1" -HubUrl "https://hub.example.com/agentgrid" -JoinToken "agj_replace_me" -InstallWorker
```

如果要安装交互式桌面助手，需要在目标桌面用户已登录时执行：

```powershell
irm "https://raw.githubusercontent.com/hanfeihu/agentgrid/main/scripts/install.ps1" -OutFile "$env:TEMP\agentgrid-install.ps1"
& "$env:TEMP\agentgrid-install.ps1" -HubUrl "https://hub.example.com/agentgrid" -JoinToken "agj_replace_me" -InstallWorker -DesktopHelper
```

## 节点授权

Worker 不登录总控台。

标准流程：

1. Hub 管理员创建节点纳管计划。
2. Hub 生成 join token。
3. Worker 启动时带 `AGENTGRID_JOIN_TOKEN`。
4. Worker 上报 `node_id`、`machine_fingerprint`、资源和能力。
5. 节点进入 `pending`。
6. Hub 管理员确认后授权。
7. 节点变成 `bound`，可以接任务。

见 [节点入网标准](node-join.md)。
