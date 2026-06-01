# 5 分钟快速开始

这份文档帮你在一台本机上跑通一个 Hub、一个 Worker 和一个命令任务。

## 方式 A：使用 Release 包

Linux 和 macOS：

```bash
curl -fsSL https://raw.githubusercontent.com/hanfeihu/agentgrid/main/scripts/install.sh | bash
```

Windows PowerShell，用管理员权限运行：

```powershell
irm "https://raw.githubusercontent.com/hanfeihu/agentgrid/main/scripts/install.ps1" | iex
```

启动本地 Hub：

```bash
agentgrid-hub \
  --host 127.0.0.1 \
  --port 20181 \
  --db /opt/agentgrid/agentgrid-hub.db \
  --web-dir /opt/agentgrid/web
```

Windows 使用安装器打印出来的路径，一般是：

```powershell
agentgrid-hub --host 127.0.0.1 --port 20181 --db "$env:ProgramFiles\AgentGrid\agentgrid-hub.db" --web-dir "$env:ProgramFiles\AgentGrid\web"
```

打开总控台：

```text
http://127.0.0.1:20181
```

启动本地 Worker：

```bash
agentgrid-worker \
  --hub http://127.0.0.1:20181 \
  --id local-worker \
  --name "Local Worker" \
  --capability command \
  --capability file \
  --capability http
```

提交一个命令任务：

```bash
agentgrid submit-command \
  --program hostname \
  --wait
```

## 方式 B：从源码构建

```bash
git clone https://github.com/hanfeihu/agentgrid.git
cd agentgrid
cargo build --release -p agentgrid-hub -p agentgrid-worker-app -p agentgrid-cli -p agentgrid-mcp
npm --prefix apps/agentgrid-web ci
npm --prefix apps/agentgrid-web run build
```

启动 Hub：

```bash
target/release/agentgrid-hub \
  --host 127.0.0.1 \
  --port 20181 \
  --db data/agentgrid-hub.db \
  --web-dir apps/agentgrid-web/dist
```

另开一个终端启动 Worker：

```bash
target/release/agentgrid-worker \
  --hub http://127.0.0.1:20181 \
  --id local-worker \
  --name "Local Worker" \
  --capability command \
  --capability file \
  --capability http
```

提交任务：

```bash
target/release/agentgrid submit-command \
  --program hostname \
  --wait
```

## 跑通后的效果

- Hub health 接口返回 `ok: true`。
- 总控台能看到 `local-worker` 在线。
- 命令任务状态变成 `done`。
- 任务结果里有 `stdout`、`stderr`、退出码、耗时和审计事件。

## 下一步

- 用 `AGENTGRID_INSTALL_WORKER_SERVICE=1` 把 Worker 安装成开机自启服务。
- 远程节点接入前，先配置真实 join token。
- 对外开放 Hub 之前，先阅读 [节点入网标准](node-join.md)。
- 任务提交命令见 [命令文档](cli.md)。
