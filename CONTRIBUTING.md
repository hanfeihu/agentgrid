# Contributing To AgentGrid

Thank you for helping build AgentGrid.

AgentGrid is a standard-oriented runtime for AI-operated machines, tools, jobs,
desktop helpers, artifacts, and worker capabilities.

## License

By contributing to this repository, you agree that your contribution is licensed
under the Apache License, Version 2.0.

## Development Checks

Run these before opening a pull request:

```bash
cargo fmt
cargo check -p agentgrid-hub -p agentgrid-worker -p agentgrid-cli
npm --prefix apps/agentgrid-web run build
```

## Contribution Guidelines

- Keep task payloads structured. AgentGrid does not accept natural-language task execution as a core contract.
- Preserve stable API names where possible.
- Add or update docs when changing task contracts, node capabilities, tool contracts, job runtime behavior, or SDK behavior.
- Do not commit secrets, private hostnames, tokens, passwords, screenshots with private data, or production database files.
- Keep Worker behavior cross-platform unless a feature is explicitly OS-specific.

## Standards First

For new capability areas, prefer adding a standard document or schema before adding a narrow implementation. Good candidates include:

- Execution Contract
- Capability Graph
- Evidence Pipeline
- Node Join Standard
- Plugin Runtime
- Tool Probe
- Job Runtime
