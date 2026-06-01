# Security Policy

AgentGrid can operate real machines. Treat every Hub, Worker, plugin, and task
as part of a sensitive automation system.

## Supported Versions

AgentGrid is currently pre-1.0. Security fixes are applied to the main branch.

## Reporting Security Issues

Please do not open public issues for vulnerabilities.

Until a public security contact is established, report privately to the project
maintainers through the repository owner.

## Secret Handling

Never commit:

- SMTP authorization codes
- SSH passwords or private keys
- Hub session tokens
- Worker join tokens
- Production SQLite databases
- Private server inventories
- Screenshots containing private customer or machine data

Use environment variables or ignored private files such as:

```text
docs/private/
private/
*.private.md
```

## Operational Safety

- New nodes should enter `pending` and require Hub approval before receiving tasks.
- Headless nodes should use AgentGrid Node Join Standard v1.
- Workers should connect outward to the Hub instead of exposing public inbound ports.
- Plugin and task runtime isolation is an active area and should be considered before running untrusted workloads.
