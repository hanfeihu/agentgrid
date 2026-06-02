# AgentGrid Architecture

AgentGrid is a Hub-and-Worker runtime for AI-operated real machines.

The Hub owns cluster state, placement decisions, jobs, tasks, artifacts, users, organizations, tools, and audit events. Workers run on machines that can execute real operations. AI clients, CLIs, SDKs, MCP servers, and web/mobile consoles talk to the Hub through structured APIs.

## High-Level Diagram

```mermaid
flowchart LR
  AI["AI Client / MCP / SDK / CLI"] --> Hub["AgentGrid Hub"]
  Console["Web / Mobile Console"] --> Hub
  Hub --> DB[("SQLite Store")]
  Hub --> Artifacts["Artifact Store"]
  Hub --> Events["Event Bus / Timeline"]
  Hub --> Placement["Placement Engine"]
  Placement --> Graph["Capability Graph"]
  Placement --> Probes["Tool Probe State"]
  Hub <--> W1["Worker: Linux"]
  Hub <--> W2["Worker: Windows"]
  Hub <--> W3["Worker: macOS"]
  Mobile["Mobile Console"] --> Hub
  Mobile -. "Bridge Session" .-> Hub
  Hub <-. "Node Service Bridge" .-> W3
  W3 -. "127.0.0.1:8390" .-> Codex["codex.local"]
  W2 --> Desktop["Desktop Helper"]
  W1 --> Plugins["Plugins / Tools"]
  W2 --> Plugins
  W3 --> Plugins
  Plugins --> Evidence["Logs / Screenshots / Files / Reports"]
  Evidence --> Hub
```

## Runtime Flow

1. An AI client discovers capabilities through `GET /api/capabilities/manifest`.
2. The client submits a structured task or Job.
3. The Hub validates the payload and builds a placement contract.
4. The Placement Engine filters nodes by hard constraints.
5. Eligible nodes are scored by resource load, concurrency, probe status, history, weight, and risk.
6. A Worker leases the task or receives work through the runtime contract.
7. The Worker executes a built-in task type or a plugin tool.
8. Output, evidence, artifacts, metrics, and audit events are written back to the Hub.
9. The Web console, CLI, SDK, MCP server, webhooks, and event streams observe the same state.

## Node Service Bridge

AgentGrid can expose selected node-local services to authenticated console
clients through the Hub. This is not arbitrary port forwarding. A Worker must
declare a local service in its heartbeat, and the Hub validates the service
before creating a short-lived bridge session.

The first built-in service is `codex.local`:

```text
mobile/web client -> Hub Bridge Session -> Worker bridge websocket -> 127.0.0.1:8390 on that node
```

Rules:

- `codex.local` must be registered by the Worker heartbeat.
- The node must be online.
- The service must report `status: "available"`.
- The allowed address is `127.0.0.1:8390`.
- Clients send structured WebSocket messages, not raw TCP.

This lets a phone or web console talk to Codex running on a real workstation
without exposing the workstation's local port to the network.

## Node Port Bridge

Node Port Bridge is an AgentGrid kernel capability for temporarily mapping a
local port on one node to a local port on another node. A typical use case is:
node A needs to open a browser against a web debug page running on node B, while
node B has no exposed inbound port.

```text
browser on node A -> A Worker listens on 127.0.0.1:18080
  -> Hub PortBridge Session
  -> B Worker
  -> B node 127.0.0.1:8080
```

Rules:

- Workers connect outward to the Hub; child nodes do not need inbound ports.
- v1 supports TCP.
- Source bind host is fixed to `127.0.0.1`.
- Target host may be `127.0.0.1`, `localhost`, or a private IP.
- Hub owns sessions, orchestration, audit, and close semantics.
- Workers own local listening, target connection, and bidirectional byte
  forwarding.

CLI example:

```bash
agentgrid bridge-port \
  --source-node a-node \
  --target-node b-node \
  --target-port 8080 \
  --source-port 18080 \
  --purpose "let node A browser access node B web debug page"
```

The returned `Source URL` is opened from node A.

## Core Modules

| Module | Responsibility |
| --- | --- |
| `apps/agentgrid-hub` | Rust HTTP Hub, store, API, console hosting, runtime loops |
| `apps/agentgrid-worker` | Cross-platform Worker, task execution, heartbeats, artifacts |
| `apps/agentgrid-cli` | Human and AI-friendly command line |
| `apps/agentgrid-mcp` | Model Context Protocol server |
| `apps/agentgrid-web` | Ant Design Pro web console |
| `crates/agentgrid-protocol` | Shared protocol types |
| `crates/agentgrid-scheduler` | Scoring and placement helpers |
| `crates/agentgrid-sdk` | Rust SDK client |
| `sdk/node` | Node.js SDK |
| `sdk/python` | Python SDK |
| `sdk/mobile` | iOS and Android console SDK standards |

## Important Standards

- AgentMessage: structured collaboration between AI agents.
- AgentTask: structured task contract.
- Capability Graph: relation model for nodes, tools, devices, plugins, probes, and evidence.
- Execution Contract: input, output, errors, timeout, retryability, artifacts, audit, and metrics.
- Evidence Pipeline: screenshots, logs, files, test reports, serial output, DOM snapshots, and timelines.
- Node Join: machine fingerprint + join token + Hub approval.
- Job Runtime: lease, checkpoint, shard, reducer, and recovery semantics.

## What AgentGrid Does Not Do

- It does not parse natural-language instructions into actions.
- It does not replace AI clients.
- It does not try to be a generic RDP, Jenkins, Ansible, or CI replacement.
- It does not assume every node has the same capabilities.

AgentGrid is the structured runtime under those systems: the part that knows what real machines can do, where a task should run, what evidence came back, and what happened over time.
