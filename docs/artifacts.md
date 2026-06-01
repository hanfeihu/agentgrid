# Artifacts and Release Assets

AgentGrid treats execution evidence as first-class data. A task is not only "done"; it should explain what happened and attach useful evidence.

## Evidence Types

Common evidence:

- stdout log
- stderr log
- screenshot
- file artifact
- directory listing
- browser text
- DOM snapshot
- downloaded file
- serial output
- test report
- operation timeline
- plugin result

## Artifact Store v2 Goals

Artifact Store v2 is the standard for storing and previewing task evidence.

It should support:

- content type
- byte size
- SHA-256 hash
- base64 inline content for small files
- external object storage references for large files
- retention policy
- task/job/node linkage
- preview hints
- download URLs
- log slicing for long logs
- artifact bundles

## Web Console

The console should show:

- artifact list
- task relationship
- image preview for screenshots
- text/log preview
- download button
- hash and size
- creation time

## Worker Update Packages

Worker auto-update uses binary artifacts published by the Hub:

```text
web/downloads/<target>/agentgrid-worker
web/downloads/<target>/agentgrid-worker.sha256
```

Windows uses:

```text
web/downloads/windows-x86_64/agentgrid-worker.exe
```

Recommended targets:

- `linux-x86_64`
- `linux-x86_64-legacy`
- `darwin-aarch64`
- `darwin-x86_64`
- `windows-x86_64`

## Release Checklist

Before publishing a GitHub release:

```bash
cargo build --release -p agentgrid-hub -p agentgrid-worker -p agentgrid-cli -p agentgrid-mcp
npm --prefix apps/agentgrid-web run build
node scripts/validate-agentgrid-schemas.js
```

Package:

- Hub binary
- Worker binaries by target
- CLI binary
- MCP binary
- web console `dist`
- checksums
- release notes

Do not include:

- database files
- logs
- private server inventory
- SMTP secrets
- SSH credentials
- screenshots with private data

