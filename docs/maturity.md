# Maturity

AgentGrid is an alpha platform. This document separates stable core behavior from early implementations so contributors do not mistake roadmap language for production guarantees.

## Stable Core

These areas are implemented and should be treated as compatibility-sensitive:

- Hub REST API shape for health, nodes, tasks, jobs, artifacts, tools, node tools, users, settings, and events.
- Worker pull-based execution and heartbeat model.
- Node join authorization with join token, machine fingerprint, pending state, and administrator approval.
- Resource-aware scheduling rules for online nodes, hard node placement, OS, groups, tags, capabilities, avoid nodes, preferred nodes, and concurrent slot capacity.
- User login, registration, session token hashing, and Argon2 password storage with legacy hash migration.
- Admin-session enforcement for user management, system settings, node approval, node configuration, node deletion, manual probe creation, and node provisioning plans.
- Organization isolation v1 for core Hub records: users, nodes, tasks, messages, audit events, artifacts, node tools, and tool probes carry `organization_id`; core list, lease, placement, audit, and artifact reads are organization-scoped.
- Worker auto-update signature verification with Ed25519 metadata, Worker-side required-signature mode, and Hub-side required-signature manifest enforcement.
- Task result and artifact records with structured JSON output.
- CLI commands used by AI clients and humans for common task submission.
- OpenAPI and JSON Schema files as the source of machine-readable integration contracts.

## V1 Implementations

These features work, but their contracts may still change before a stable release:

- Job Runtime shards, checkpoints, reducers, and recovery scan.
- Tool Probe and trust-aware scheduling.
- Plugin execution through the Worker runtime.
- Desktop Helper screenshot, click, type, and key operations.
- Evidence Pipeline and Artifact Store v2 metadata.
- MCP server and SDK APIs.
- Web console pages beyond core task, node, user, and artifact views.

## Known Gaps

- Hub storage is still concentrated in `apps/agentgrid-hub/src/main.rs`; new work should continue extracting focused modules.
- Web console is functional but still too centralized in `apps/agentgrid-web/src/main.jsx`; high-change pages should move into page/component modules.
- End-to-end tests for Hub, Worker, CLI, and browser console flows are still limited; `scripts/e2e-hub-worker-cli.sh` covers the first Hub + Worker + CLI command path, and `scripts/e2e-codex-bridge.sh` covers the Node Service Bridge path for `codex.local`.
- Worker update signing exists, but production key rotation and CI enforcement still need hardening.
- Full browser automation and long-lived interactive terminal channels need stronger runtime contracts.
- Full multi-organization product flows still need hardening: organization switching, organization-scoped admin roles, organization-scoped node-id uniqueness, and complete Job/Workflow tenant isolation are not yet stable.

## Release Rule

When a feature is documented as stable, changes must include:

- protocol or API compatibility notes
- tests for the changed rule
- migration behavior for existing Hub data when needed
- documentation updates in English and Chinese when operator behavior changes
