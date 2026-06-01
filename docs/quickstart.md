# 5-Minute Quick Start

This guide gets one Hub, one Worker, and one command task running on a local machine.

## Option A: Use a Release Build

Linux and macOS:

```bash
curl -fsSL https://raw.githubusercontent.com/hanfeihu/agentgrid/main/scripts/install.sh | bash
```

Windows PowerShell, run as Administrator:

```powershell
irm "https://raw.githubusercontent.com/hanfeihu/agentgrid/main/scripts/install.ps1" | iex
```

Start a local Hub:

```bash
agentgrid-hub \
  --host 127.0.0.1 \
  --port 20181 \
  --db /opt/agentgrid/agentgrid-hub.db \
  --web-dir /opt/agentgrid/web
```

On Windows, use the install path printed by the installer, usually:

```powershell
agentgrid-hub --host 127.0.0.1 --port 20181 --db "$env:ProgramFiles\AgentGrid\agentgrid-hub.db" --web-dir "$env:ProgramFiles\AgentGrid\web"
```

Open the console:

```text
http://127.0.0.1:20181
```

Start a local Worker:

```bash
agentgrid-worker \
  --hub http://127.0.0.1:20181 \
  --id local-worker \
  --name "Local Worker" \
  --capability command \
  --capability file \
  --capability http
```

Submit a command task:

```bash
agentgrid submit-command \
  --program hostname \
  --wait
```

## Option B: Build From Source

```bash
git clone https://github.com/hanfeihu/agentgrid.git
cd agentgrid
cargo build --release -p agentgrid-hub -p agentgrid-worker-app -p agentgrid-cli -p agentgrid-mcp
npm --prefix apps/agentgrid-web ci
npm --prefix apps/agentgrid-web run build
```

Run the Hub:

```bash
target/release/agentgrid-hub \
  --host 127.0.0.1 \
  --port 20181 \
  --db data/agentgrid-hub.db \
  --web-dir apps/agentgrid-web/dist
```

Run the Worker in another terminal:

```bash
target/release/agentgrid-worker \
  --hub http://127.0.0.1:20181 \
  --id local-worker \
  --name "Local Worker" \
  --capability command \
  --capability file \
  --capability http
```

Submit a task:

```bash
target/release/agentgrid submit-command \
  --program hostname \
  --wait
```

## What Good Looks Like

- The Hub health endpoint returns `ok: true`.
- The console shows `local-worker` online.
- The command task reaches `done`.
- The task result contains `stdout`, `stderr`, exit code, runtime, and audit events.

## Next Steps

- Install Worker as a service with `AGENTGRID_INSTALL_WORKER_SERVICE=1`.
- Add a real join token before connecting remote nodes.
- Read [Node Join Standard](node-join-standard.md) before opening a Hub to other machines.
- Read [CLI](cli.md) for task submission commands.
