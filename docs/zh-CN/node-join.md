# 节点入网标准

AgentGrid 节点入网采用“机器主动连接，Hub 管理员授权”的标准。

Worker 不登录后台。后台登录的是人类管理员或 AI 员工账号。Worker 只负责上报机器身份、资源、能力和心跳。

## 核心流程

1. 管理员在 Hub 创建节点纳管计划。
2. Hub 生成一次性 join token。
3. 目标机器安装 Worker，并带上 `AGENTGRID_JOIN_TOKEN`。
4. Worker 上报稳定的 `node_id`、`machine_fingerprint`、资源和能力。
5. Hub 将节点标记为 `pending`。
6. `pending` 节点可以心跳，但不能接任务。
7. 管理员在 Hub 节点管理页面确认机器码和 token hint。
8. 管理员点击授权。
9. Hub 绑定 `node_id + machine_fingerprint + join_token_hash`。
10. 节点变成 `bound`，在线时可以接任务。

## 状态

| 状态 | 含义 | 是否可接任务 |
| --- | --- | --- |
| `pending` | 等待管理员授权 | 否 |
| `bound` | 已绑定机器码和 token | 是 |
| `legacy` | 老版本兼容节点 | 是，直到重新纳管 |
| `rejected` | 被拒绝 | 否 |

## 为什么这样设计

很多服务器、CI 机器、内网电脑没有浏览器，或者不应该让机器自己登录后台。

AgentGrid 采用类似设备授权的思想：

- 机器只发起后端协议请求。
- 管理员在 Hub 上完成授权。
- Hub 绑定机器码，避免 token 被搬到另一台机器继续用。
- 未授权机器不能接任务。

## Worker 上报示例

```json
{
  "api_version": "agentgrid.node-join/v1",
  "kind": "NodeJoinRequest",
  "node_id": "linux-worker-01",
  "node_name": "Linux Worker 01",
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

## 调度规则

调度器必须先检查：

```text
node.auth_status == "bound" OR node.auth_status == "legacy"
node.status.state == "online"
task hard constraints match node
tool/capability/probe requirements match node
resource score acceptable
```

只有通过这些条件，节点才进入最优评分。

