# Architecture Notes

## Components

- `scheduler-core`: shared scheduling types and execution logic.
- `apps/cli`: command-line interface for scripting and AI-client control.
- `apps/daemon`: local process that scans due tasks and executes them.
- `apps/desktop`: graphical desktop application.

## Current Core Boundary

The core does not depend on a UI layer. It owns:

- Task model
- One-time schedule calculation
- SQLite persistence
- Due-task lookup
- Command execution
- Run records

The CLI can create and inspect tasks. The daemon is responsible for repeatedly running due tasks.

## AI Client Integration

Planned integration options:

- Local command-line calls
- Local HTTP or IPC API
- JSON task definitions
- Event stream for task status updates

## Cross-Platform Notes

The scheduler should avoid OS-specific behavior in the core crate. Platform-specific background execution can live behind adapter traits or separate crates later.
