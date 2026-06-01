# AgentGrid

AgentGrid is an open-source runtime and scheduling layer for AI-operated real machines, tools, desktops, jobs, artifacts, and worker capabilities.

It is built for AI clients that need to discover machines, choose the right worker node, execute structured tasks, collect evidence, and keep an auditable timeline of what happened.

## What It Does

- Hub: central control plane, task API, job runtime, node registry, artifact store, and web console.
- Worker: cross-platform Rust node agent for Linux, macOS, and Windows.
- CLI: structured task submission for AI clients and humans.
- MCP Server: standard tool surface for AI clients.
- SDKs: Rust, Node, Python, and mobile console SDK standards.
- Standards: AgentMessage, task contracts, capability graph, evidence pipeline, node join, runtime contracts, and plugin/tool contracts.

## Project Layout

```text
agentgrid/
├── apps/
│   ├── agentgrid-hub/     # Rust Hub server
│   ├── agentgrid-worker/  # Cross-platform Worker
│   ├── agentgrid-cli/     # CLI for task/job/node operations
│   ├── agentgrid-mcp/     # MCP server
│   └── agentgrid-web/     # Web control console
├── crates/
│   ├── agentgrid-protocol/
│   ├── agentgrid-scheduler/
│   └── agentgrid-sdk/
├── sdk/
│   ├── node/
│   ├── python/
│   └── mobile/
└── docs/
```

## Quick Start

Run local checks:

```bash
cargo check -p agentgrid-hub -p agentgrid-worker -p agentgrid-cli
npm --prefix apps/agentgrid-web run build
```

Start the Hub:

```bash
cargo run -p agentgrid-hub -- \
  --host 0.0.0.0 \
  --port 20181 \
  --db data/agentgrid-hub.db \
  --web-dir apps/agentgrid-web/dist
```

Start a Worker:

```bash
cargo run -p agentgrid-worker -- \
  --hub http://127.0.0.1:20181 \
  --id local-worker \
  --name "Local Worker"
```

## Open Source License

AgentGrid is released under the Apache License, Version 2.0.
See [LICENSE](LICENSE), [NOTICE](NOTICE), and [OPEN_SOURCE.md](OPEN_SOURCE.md).

## Design Focus

- AI-operated real machines and hardware workbenches.
- Worker capability registration and probing.
- Resource-aware placement and job recovery.
- Evidence-first execution: logs, screenshots, reports, artifacts, and audit events.
- Private-network friendly workers that connect outward to the Hub.
