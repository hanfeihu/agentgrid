# AgentGrid Deployment Assets Template

This public template documents what operators should record for a deployment.
Do not store plaintext passwords, SMTP authorization codes, SSH passwords,
private keys, or production tokens in this repository.

For private deployments, copy this file to an ignored location such as:

```text
docs/private/server-assets.private.md
```

## Public Entrypoints

| Name | URL | Purpose | Notes |
| --- | --- | --- | --- |
| AgentGrid Console | `https://example.com/agentgrid/` | Web control console | Reverse proxy entry |
| AgentGrid API | `https://example.com/agentgrid/api` | Hub REST API | Workers also use this URL |

## Hub Administration

- Organization concept: Hub has one default organization at bootstrap.
- Super administrator: Hub allows exactly one `super_admin`.
- Bootstrap rule: when no super administrator exists, the Web console shows an initialization page.
- Password rule: after login, the administrator can change password in System Settings.
- Hub public URL: configurable in System Settings.
- Node authorization standard: AgentGrid Node Join Standard v1. Headless Linux nodes do not need a browser; they start with `AGENTGRID_JOIN_TOKEN`, enter `pending`, and are approved by the Hub `super_admin` from the Web console.

## Mail Service

SMTP is used for email verification registration.

| Field | Value |
| --- | --- |
| SMTP host | `smtp.example.com` |
| SMTP port | `465` |
| Mail account | `agentgrid@example.com` |
| Authorization code | store outside git |
| From | `agentgrid@example.com` |

Recommended Hub environment variables:

```bash
AGENTGRID_SMTP_HOST=smtp.example.com
AGENTGRID_SMTP_PORT=465
AGENTGRID_SMTP_USERNAME=agentgrid@example.com
AGENTGRID_SMTP_PASSWORD=replace-me-outside-git
AGENTGRID_SMTP_FROM=agentgrid@example.com
AGENTGRID_SMTP_ENABLED=true
```

## Code Hosting

| Name | URL | Account | Purpose |
| --- | --- | --- | --- |
| Git remote | `https://git.example.com/org/agentgrid.git` | team account | Source repository and deployment assets |

## Servers

| Asset ID | SSH | Role | Node ID | OS | Status | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| hub-01 | `ssh user@hub.example.com` | Hub and optional Worker node | `hub-01` | Linux x86_64 | Planned | Reverse proxy terminates public traffic |
| worker-01 | `ssh user@worker.example.com` | Worker node | `worker-01` | Linux x86_64 | Planned | No inbound app port required |

## Credential Policy

- SSH credentials are not stored in this repository.
- SMTP authorization codes are not stored in this repository.
- Prefer SSH keys or a dedicated secret manager for future operations.
- If an AI agent needs to operate a server, provide credentials only for the current session or configure key-based access first.
