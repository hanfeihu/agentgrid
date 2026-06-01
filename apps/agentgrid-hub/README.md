# AgentGrid Hub

`agentgrid-hub` is the Rust control plane for AgentGrid.

It owns:

- organizations and Hub users
- node registry and node join authorization
- task and Job APIs
- placement decisions
- artifacts and evidence
- Tool Registry and Node Tools
- event timeline and audit logs
- Web console static hosting
- Worker update manifests and downloads

## Run Locally

Build the Web console first:

```bash
npm --prefix ../agentgrid-web install
npm --prefix ../agentgrid-web run build
```

Run Hub:

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

## API Examples

Health:

```bash
curl http://127.0.0.1:20181/api/health
```

Nodes:

```bash
curl http://127.0.0.1:20181/api/nodes
```

Capabilities:

```bash
curl http://127.0.0.1:20181/api/capabilities/manifest
```

