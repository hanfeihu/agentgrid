# AgentGrid Server Assets

Last updated: 2026-06-01

This file records AgentGrid infrastructure assets for operators and AI agents.
This is a private repository asset register. Some service passwords may be
recorded here only when HR explicitly authorizes it.

## Public Entrypoints

| Name | URL | Purpose | Notes |
| --- | --- | --- | --- |
| AgentGrid Console | http://chenqi.tminos.com:20080/agentgrid/ | Web control console | Nginx proxies `/agentgrid/` to Hub |
| AgentGrid API | http://chenqi.tminos.com:20080/agentgrid/api | Hub REST API | Workers also use this URL |

## Hub Administration

- Organization concept: Hub has one default organization at bootstrap.
- Super administrator: Hub allows exactly one `super_admin`.
- Bootstrap rule: when no super administrator exists, the Web console shows an initialization page. HR must create and save the first admin account.
- Password rule: after login, the administrator can change password in System Settings.
- Hub public URL: configurable in System Settings. Current default is `http://chenqi.tminos.com:20080/agentgrid`.

## Mail Service

SMTP is used for email verification registration.

| Field | Value |
| --- | --- |
| SMTP host | `smtp.qq.com` |
| SMTP port | `465` |
| Mail account | `1668217900@qq.com` |
| Authorization code | `oebnbqxrirmybacd` |
| From | `1668217900@qq.com` |

## Code Hosting

| Name | URL | Account | Password | Purpose |
| --- | --- | --- | --- | --- |
| GitLab | `https://gitlab.zhuzhux.com/` | `hanfeihu` | `hanfeihu` | AgentGrid private source repository and deployment assets |

## Servers

| Asset ID | SSH | Role | Node ID | OS | Status | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| chenqi | `ssh chenqi.tminos.com` | Center server, Hub, Worker node | `chenqi-center` | Ubuntu Linux x86_64 | Online | Hub listens on `0.0.0.0:20181`; nginx container proxies public entry |
| jia | `ssh jia.zhuzhux.com` | Worker node | `jia-node` | Ubuntu Linux x86_64 | Online | No inbound app port required; Worker connects to Hub |
| local-mac | local machine | Worker node, development machine | `local-mac` | macOS Darwin aarch64 | Online | Local development and build machine |
| huarui | `ssh root@huarui.zhuzhux.com` | Worker node | `huarui-node` | Alibaba Cloud Linux 3 x86_64 | Online | glibc 2.32; Worker is compiled locally on this host |

## Huarui Setup Notes

- Hostname: `ruiju`
- Private IPv4 observed by Worker: `172.16.0.184`
- Worker install directory: `/opt/agentgrid-worker`
- Source build directory: `/opt/agentgrid-src`
- Service: `agentgrid-worker.service`
- Capabilities: `http`, `command`, `file`, `git`, `docker`, `browser`, `session`, `agentmessage`
- Max concurrent jobs: `4`
- Worker auto-update: disabled with `--no-auto-update` because the published Linux package requires newer glibc than Alibaba Cloud Linux 3 provides.

Useful checks:

```bash
systemctl status agentgrid-worker.service --no-pager -l
journalctl -u agentgrid-worker.service -n 80 --no-pager
```

## Credential Policy

- SSH credentials are not stored in this repository.
- `huarui` root password was provided by HR during setup and should be kept in the operator password manager or secure local notes.
- Prefer SSH keys for future operations.
- If an AI agent needs to operate a server, provide credentials only for the current session or configure key-based access first.

## Verification History

| Time | Asset | Verification |
| --- | --- | --- |
| 2026-05-31 18:08 CST | huarui | `huarui-node` came online in Hub with 4 CPU cores and 7481 MB memory |
| 2026-05-31 18:09 CST | huarui | Command task `hostname` ran on `huarui-node` and returned `ruiju` |
| 2026-05-31 18:13 CST | huarui | Worker remained online after stability wait; command task returned `ruiju` with `--no-auto-update` |

## Provisioning Standard

New nodes should be added through the Hub provisioning plan API:

```http
POST /api/node-provisioning/plans
```

The plan records node ID, host, OS, architecture, Hub URL, install steps, and
authorization hint.

Node join authorization rule:

1. Worker starts with Hub URL, node id, node name, machine fingerprint, and join token.
2. Hub records the node as `pending`.
3. Super administrator approves the node in Web console.
4. Hub binds the node id to the machine fingerprint and token hash.
5. Only `bound` or legacy-compatible nodes can lease tasks.

Worker heartbeat must report:

- Worker version
- Worker target
- glibc version on Linux
- Machine fingerprint
- Join token during registration/lease
- auto-update enabled or disabled
- CPU cores, memory total/used, disk total/free, running jobs, max concurrent jobs
