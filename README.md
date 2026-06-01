# AI Task Scheduler

A cross-platform Rust task scheduling system for AI clients.

## Targets

- Desktop app: macOS, Linux, Windows
- Command-line app: macOS, Linux, Windows
- Shared scheduler core for reuse by AI clients

## Project Layout

```text
ai-task-scheduler/
├── apps/
│   ├── cli/        # Command-line interface
│   ├── daemon/     # Local scheduler daemon
│   └── desktop/    # Desktop application
├── crates/
│   └── scheduler-core/ # Shared scheduling engine
└── docs/           # Design notes and API drafts
```

## Core MVP

The first core loop is intentionally small:

1. Create a one-time command task with the CLI.
2. Persist it in SQLite.
3. Run `ai-taskd` as the local scheduler process.
4. Execute due tasks.
5. Store task run results, stdout, stderr, and exit code.

Example:

```bash
cargo run -p ai-task-scheduler-cli -- add \
  --name "Say hello" \
  --at "2026-06-01T09:00:00Z" \
  --program echo \
  --arg "hello"

cargo run -p ai-task-scheduler-daemon -- run
```

## Early Design Goals

- Reliable local task scheduling
- AI-client-friendly API surface
- Human-readable configuration
- Cross-platform background execution
- Clear separation between core logic, desktop UI, and CLI
