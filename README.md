# AgentGrid

[中文文档](README.zh-CN.md) | [Vision](docs/vision.md) | [Install](docs/install.md) | [Deploy](docs/deployment.md) | [CLI](docs/cli.md) | [Artifacts](docs/artifacts.md)

AgentGrid is an open-source scheduling layer for AI agents that need to operate real machines, desktops, tools, devices, jobs, and evidence.

It is not a natural-language automation product. AI clients do the reasoning. AgentGrid provides the structured runtime underneath: capability discovery, placement, worker execution, artifacts, audit trails, and recoverable jobs across private and cloud machines.

![AgentGrid console overview](docs/assets/screenshots/agentgrid-console-overview.svg)

## Why AgentGrid

AI agents are getting good at planning, coding, and using tools, but many real workflows still require access to actual computers:

- Windows workstations with desktop-only software
- Linux build nodes and private servers
- browser stations, SDK stations, and internal tooling
- hardware benches, serial ports, flashers, and test rigs
- evidence such as screenshots, logs, files, reports, DOM snapshots, and test results

AgentGrid turns those machines into a discoverable, schedulable, auditable capability grid.

## Core Ideas

- **Hub**: the control plane for users, organizations, nodes, tasks, jobs, tools, artifacts, events, and the web console.
- **Worker**: a cross-platform Rust agent running on Linux, macOS, and Windows nodes.
- **Capability Graph**: node -> device -> tool -> plugin -> probe -> evidence -> suitable task.
- **Placement Engine**: schedules by hard constraints, soft scores, resource load, probe status, history, and risk.
- **Evidence Pipeline**: every execution can return structured output, artifacts, logs, screenshots, and timelines.
- **Job Runtime**: retries, leases, checkpoints, shard execution, recovery, and reducers.
- **AgentMessage**: structured AI-to-AI collaboration messages.
- **MCP + SDKs**: standard surfaces for AI clients and humans.

## Ecosystem Vision

AgentGrid is designed as infrastructure for an ecosystem:

- worker plugins that add new tool families
- reusable task templates and workbench runbooks
- SDKs for Rust, Node, Python, iOS, Android, and future clients
- MCP servers for AI clients
- integrations with skill-based systems, local-first design tools, and agent workbenches

Projects such as [Open Design](https://open-design.ai/zh/) can fit naturally above AgentGrid: their skills and design workflows can become AgentGrid tools or plugins, while AgentGrid supplies real-machine scheduling, desktop execution, artifacts, and audit evidence.

## Features

- Hub web console built with Ant Design Pro
- node status, resources, CPU cores, memory, disk, OS, IP, and heartbeat
- node join authorization with machine fingerprint and join token
- command, HTTP, file, Git, Docker, browser, desktop, plugin, and AgentMessage tasks
- Windows Desktop Helper for screenshot, click, type, and key operations
- task result details, logs, artifacts, and screenshots
- task queue, priorities, target node/OS placement, and schedule explanations
- Job Runtime with dry-run plans, shards, checkpoints, recovery scan, and reducers
- Tool Registry, Node Tools, Tool Probe, and capability manifests
- Webhooks, Event Bus, execution records, and audit logs
- MCP server and SDKs
- OpenAPI and JSON Schema contracts

## Quick Start

Requirements:

- Rust stable toolchain
- Node.js 20+ for the web console

Build and check:

```bash
cargo check -p agentgrid-hub -p agentgrid-worker -p agentgrid-cli -p agentgrid-mcp
npm --prefix apps/agentgrid-web install
npm --prefix apps/agentgrid-web run build
```

Run the Hub:

```bash
cargo run -p agentgrid-hub -- \
  --host 127.0.0.1 \
  --port 20181 \
  --db data/agentgrid-hub.db \
  --web-dir apps/agentgrid-web/dist
```

Run a Worker:

```bash
cargo run -p agentgrid-worker -- \
  --hub http://127.0.0.1:20181 \
  --id local-worker \
  --name "Local Worker" \
  --capability command \
  --capability file \
  --capability http
```

Open the console:

```text
http://127.0.0.1:20181
```

Submit a command task:

```bash
cargo run -p agentgrid-cli -- submit-command \
  --program hostname \
  --wait
```

## Documentation

| Topic | English | Chinese |
| --- | --- | --- |
| Vision and ecosystem | [docs/vision.md](docs/vision.md) | [docs/zh-CN/vision.md](docs/zh-CN/vision.md) |
| Architecture | [docs/architecture.md](docs/architecture.md) | [docs/zh-CN/architecture.md](docs/zh-CN/architecture.md) |
| Install | [docs/install.md](docs/install.md) | [docs/zh-CN/install.md](docs/zh-CN/install.md) |
| Deployment | [docs/deployment.md](docs/deployment.md) | [docs/zh-CN/deployment.md](docs/zh-CN/deployment.md) |
| CLI | [docs/cli.md](docs/cli.md) | [docs/zh-CN/cli.md](docs/zh-CN/cli.md) |
| Node join | [docs/node-join-standard.md](docs/node-join-standard.md) | [docs/zh-CN/node-join.md](docs/zh-CN/node-join.md) |
| Artifacts and releases | [docs/artifacts.md](docs/artifacts.md) | [docs/zh-CN/artifacts.md](docs/zh-CN/artifacts.md) |
| OpenAPI | [docs/openapi/agentgrid-openapi.yaml](docs/openapi/agentgrid-openapi.yaml) | Same |
| Command reference | [docs/agentgrid-command-reference.md](docs/agentgrid-command-reference.md) | Same |

## Project Layout

```text
agentgrid/
├── apps/
│   ├── agentgrid-hub/       # Rust Hub server
│   ├── agentgrid-worker/    # Cross-platform Worker
│   ├── agentgrid-cli/       # CLI for AI clients and humans
│   ├── agentgrid-mcp/       # MCP server
│   └── agentgrid-web/       # Web console
├── crates/
│   ├── agentgrid-protocol/
│   ├── agentgrid-scheduler/
│   └── agentgrid-sdk/
├── sdk/
│   ├── node/
│   ├── python/
│   └── mobile/
├── schemas/
├── docs/
└── examples/
```

## Security Note

AgentGrid can execute commands and operate desktops on machines you own or administer. Do not expose a Hub publicly without authentication, network controls, and operational policy. Production secrets, SMTP codes, SSH passwords, private keys, and server inventories must stay outside git.

## License

AgentGrid is released under the Apache License, Version 2.0. See [LICENSE](LICENSE), [NOTICE](NOTICE), and [OPEN_SOURCE.md](OPEN_SOURCE.md).

