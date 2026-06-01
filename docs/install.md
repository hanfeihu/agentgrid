# Installation

This guide installs AgentGrid for local development and for worker nodes.

## Requirements

- Rust stable
- Node.js 20+
- Git
- Linux/macOS/Windows for Worker nodes

## Build From Source

```bash
git clone https://github.com/<your-org>/agentgrid.git
cd agentgrid
cargo check -p agentgrid-hub -p agentgrid-worker -p agentgrid-cli -p agentgrid-mcp
npm --prefix apps/agentgrid-web install
npm --prefix apps/agentgrid-web run build
```

## Run Hub Locally

```bash
cargo run -p agentgrid-hub -- \
  --host 127.0.0.1 \
  --port 20181 \
  --db data/agentgrid-hub.db \
  --web-dir apps/agentgrid-web/dist
```

Open:

```text
http://127.0.0.1:20181
```

When no super administrator exists, the console shows the bootstrap page.

## Run Worker Locally

```bash
cargo run -p agentgrid-worker -- \
  --hub http://127.0.0.1:20181 \
  --id local-worker \
  --name "Local Worker" \
  --capability http \
  --capability command \
  --capability file \
  --capability git \
  --max-concurrent-jobs 4
```

## Install Linux Worker With systemd

Build the Worker first:

```bash
cargo build --release -p agentgrid-worker
```

Install:

```bash
AGENTGRID_HUB_URL=https://hub.example.com/agentgrid \
AGENTGRID_NODE_ID=linux-worker-01 \
AGENTGRID_NODE_NAME="Linux Worker 01" \
AGENTGRID_JOIN_TOKEN=agj_replace_me \
./scripts/install-linux-systemd.sh target/release/agentgrid-worker
```

## Install macOS Worker With launchd

```bash
cargo build --release -p agentgrid-worker

AGENTGRID_HUB_URL=https://hub.example.com/agentgrid \
AGENTGRID_NODE_ID=mac-worker-01 \
AGENTGRID_NODE_NAME="Mac Worker 01" \
AGENTGRID_JOIN_TOKEN=agj_replace_me \
./scripts/install-macos-launchd.sh target/release/agentgrid-worker
```

## Install Windows Worker

Run an elevated PowerShell session:

```powershell
$env:AGENTGRID_JOIN_TOKEN="agj_replace_me"
irm "https://hub.example.com/agentgrid/install/windows.ps1" | iex
```

For interactive desktop automation, install the Desktop Helper while the target desktop user is logged in:

```powershell
$env:AG_DESKTOP_HELPER="1"
$env:AGENTGRID_JOIN_TOKEN="agj_replace_me"
irm "https://hub.example.com/agentgrid/install/windows.ps1" | iex
```

## Node Authorization

Workers do not log in to the Web console.

The standard flow is:

1. A Hub administrator creates a node provisioning plan.
2. The Hub generates a join token.
3. The Worker starts with `AGENTGRID_JOIN_TOKEN`.
4. The Worker reports `node_id`, `machine_fingerprint`, resources, and capabilities.
5. The node appears as `pending`.
6. The Hub administrator approves the node.
7. The node becomes `bound` and can receive tasks.

See [Node Join Standard](node-join-standard.md).

