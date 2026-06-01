# AgentGrid Node Join Standard v1

AgentGrid 节点入网采用“机器主动连接，Hub 管理员授权”的标准。
这个标准解决 Linux 服务器、CI 机器、内网电脑没有浏览器或不方便登录的问题。

## Core Idea

- Worker 不需要打开浏览器。
- Worker 启动时提交稳定的 `node_id`、`machine_fingerprint`、`join_token`。
- Hub 先把节点标记为 `pending`。
- `pending` 节点可以上报心跳和资源，但不能接任务。
- 超级管理员在 Hub 页面确认后，节点变成 `bound`。
- `bound` 节点在线且满足调度条件时才能接任务。

## Industry Mapping

这个流程参考 OAuth 2.0 Device Authorization Grant 的思想：

- 设备本身不负责完整登录体验。
- 用户在另一台有浏览器的设备上完成授权。
- 设备通过后端协议等待授权结果。

AgentGrid 不把它做成完整 OAuth，因为 AgentGrid 需要绑定机器指纹、节点能力、资源状态和任务调度权限。

## Roles

| Role | Responsibility |
| --- | --- |
| Hub | 保存组织、用户、节点、授权状态、任务调度状态 |
| Super Admin | 有且只有一个，负责首次初始化和节点授权 |
| Worker | 安装在 Linux/Windows/macOS 节点上，主动连 Hub |
| Operator | 执行安装命令的人，可以不是 Hub 管理员 |

## States

| State | Meaning | Can Lease Tasks |
| --- | --- | --- |
| `pending` | 节点已申请入网，等待 Hub 授权 | No |
| `bound` | 节点已绑定 token 和机器指纹 | Yes |
| `legacy` | 老版本节点兼容状态 | Yes, until re-enrolled |

## Join Request

```json
{
  "api_version": "agentgrid.node-join/v1",
  "kind": "NodeJoinRequest",
  "node_id": "linux-worker-02",
  "node_name": "huarui 子节点",
  "machine_fingerprint": "sha256:stable-machine-id",
  "join_token": "agj_xxx",
  "capabilities": ["command", "file", "git"],
  "system": {
    "os": "linux",
    "arch": "x86_64",
    "cpu_cores": 4,
    "memory_mb": 8192
  }
}
```

## Headless Linux Flow

1. 管理员在 Hub 创建节点纳管计划。
2. Hub 生成一次性 `agj_...` join token。
3. Hub 生成 systemd 服务模板，写入 `AGENTGRID_JOIN_TOKEN`。
4. 运维人员在 Linux 终端执行安装命令。
5. Worker 主动向 Hub 心跳，节点进入 `pending`。
6. 管理员在自己的浏览器打开 Hub，检查节点 ID、机器指纹、token hint。
7. 管理员点击授权。
8. Hub 绑定 `node_id + machine_fingerprint + join_token_hash`。

## Security Boundary

- Hub 不保存明文 join token，只保存 hash 和 hint。
- 节点机器指纹变更后，Hub 拒绝原 token 请求。
- `pending` 节点不能租约任务，避免陌生机器直接拿任务。
- 子节点不需要暴露公网入口，只主动连接 Hub。

## Standard Install Command Shape

```bash
sudo mkdir -p /opt/agentgrid-worker
curl -fsSL https://hub.example.com/agentgrid/api/worker/download/linux-x86_64 \
  -o /opt/agentgrid-worker/agentgrid-worker
sudo chmod +x /opt/agentgrid-worker/agentgrid-worker
```

systemd:

```ini
[Service]
Environment=AGENTGRID_JOIN_TOKEN=agj_xxx
ExecStart=/opt/agentgrid-worker/agentgrid-worker \
  --hub https://hub.example.com/agentgrid/api \
  --id linux-worker-02 \
  --name "huarui 子节点" \
  --max-concurrent-jobs 4
```

## Scheduling Rule

调度器必须先检查：

```text
node.auth_status == "bound" OR node.auth_status == "legacy"
node.status.state == "online"
task hard constraints match node
tool/capability/probe requirements match node
resource score acceptable
```

只有通过这些条件后，节点才进入最优调度评分。
