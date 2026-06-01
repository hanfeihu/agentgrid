# Vision

AgentGrid exists to make real machines programmable by AI agents through open, structured, auditable standards.

AI models can reason, plan, and write code. But many valuable tasks still happen outside the model: a Windows desktop app, a Linux build server, an internal browser, a hardware bench, a serial console, a flashing tool, a test rig, or a machine inside a private network.

AgentGrid is the scheduling and evidence layer for that world.

## The North Star

Build an open capability grid where AI clients can:

- discover which machines, devices, desktops, tools, plugins, and workbenches exist
- submit structured tasks without natural-language ambiguity
- schedule work to the right node by capability, resource load, probe status, and risk
- collect screenshots, logs, files, reports, DOM snapshots, serial output, and metrics
- recover jobs when nodes disappear
- let humans audit what happened
- let community plugins and templates expand the ecosystem

## Ecosystem Position

AgentGrid should work with, not replace, AI clients and skill systems.

An AI client should decide what to do. AgentGrid should decide where and how a structured task can safely run, then return evidence.

This makes AgentGrid a natural companion for:

- Codex-like coding agents
- Claude/Cursor-style developer tools
- MCP clients
- local-first skill systems
- hardware test automation
- desktop automation workbenches
- design and browser automation platforms

## Open Design Integration Direction

[Open Design](https://open-design.ai/zh/) is a strong example of a local-first, open, skill-oriented AI design environment. AgentGrid and systems like Open Design can fit together cleanly:

- Open Design Skills can become AgentGrid Tools.
- Open Design workflows can call AgentGrid through MCP or SDKs.
- AgentGrid Workers can run design, browser, file, desktop, and local tool operations on real machines.
- AgentGrid can return screenshots, files, logs, and reports as evidence.
- AgentGrid can schedule different skills to different machines based on capability and current health.

The boundary is simple:

- Open Design owns design workflow, skill UX, and model interaction.
- AgentGrid owns machine capability discovery, placement, execution, evidence, artifacts, and audit.

## What Makes AgentGrid Valuable

The long-term value is not "remote command execution." That market is crowded.

The long-term value is **AI operation of real workbenches**:

- hardware test benches
- Windows desktop workstations
- browser/SDK stations
- CI/build clusters behind private networks
- device farms and lab machines
- enterprise machines that cannot expose inbound ports

## Ecosystem Surfaces

AgentGrid should invite ecosystem work in these areas:

- Worker plugins
- Tool manifests
- Capability graph extensions
- Task templates
- Job templates
- Workbench runbooks
- SDKs
- MCP tools
- Evidence viewers
- deployment packages
- integrations with skill systems

## Design Principles

- Structured payloads over natural language.
- Evidence over blind success flags.
- Capabilities differ by node; do not force uniform workers.
- Workers connect outward; private networks are first-class.
- Humans should be able to audit every AI action.
- Standards should be machine-readable: OpenAPI, JSON Schema, and versioned contracts.
- The core runtime should remain useful even when UI and AI clients change.

