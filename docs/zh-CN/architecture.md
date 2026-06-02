# AgentGrid 架构

AgentGrid 是面向 AI 真实机器操作的 Hub + Worker 运行时。

Hub 负责集群状态、调度决策、Job、任务、产物、用户、组织、工具和审计事件。Worker 运行在可以执行真实操作的机器上。AI 客户端、CLI、SDK、MCP、Web/Mobile 总控台都通过结构化 API 调用 Hub。

## 总体架构

```mermaid
flowchart LR
  AI["AI Client / MCP / SDK / CLI"] --> Hub["AgentGrid Hub"]
  Console["Web / Mobile Console"] --> Hub
  Hub --> DB[("SQLite Store")]
  Hub --> Artifacts["Artifact Store"]
  Hub --> Events["Event Bus / Timeline"]
  Hub --> Placement["Placement Engine"]
  Placement --> Graph["Capability Graph"]
  Placement --> Probes["Tool Probe State"]
  Hub <--> W1["Worker: Linux"]
  Hub <--> W2["Worker: Windows"]
  Hub <--> W3["Worker: macOS"]
  Mobile["Mobile Console"] --> Hub
  Mobile -. "Bridge Session" .-> Hub
  Hub <-. "Node Service Bridge" .-> W3
  W3 -. "127.0.0.1:8390" .-> Codex["codex.local"]
  W2 --> Desktop["Desktop Helper"]
  W1 --> Plugins["Plugins / Tools"]
  W2 --> Plugins
  W3 --> Plugins
  Plugins --> Evidence["Logs / Screenshots / Files / Reports"]
  Evidence --> Hub
```

## 运行流程

1. AI 客户端通过 `GET /api/capabilities/manifest` 发现能力。
2. 客户端提交结构化 Task 或 Job。
3. Hub 验证 payload，并生成调度契约。
4. Placement Engine 先按硬约束过滤节点。
5. 再按资源、并发槽位、Probe 状态、历史成功率、权重和风险评分。
6. Worker 获取或接收任务。
7. Worker 执行内置任务类型或插件工具。
8. 输出、证据、产物、指标和审计事件回写 Hub。
9. Web、CLI、SDK、MCP、Webhook、事件流都读取同一份状态。

## 节点本地服务桥接

AgentGrid 可以把节点本机的指定本地服务，通过 Hub 暴露给已登录的
Web/Mobile 控制台。这个能力不是任意端口转发。Worker 必须先在心跳里声明
本地服务，Hub 校验通过后才会创建短时 Bridge Session。

第一版内置服务是 `codex.local`：

```text
手机/网页客户端 -> Hub Bridge Session -> Worker 桥接 WebSocket -> 该节点的 127.0.0.1:8390
```

规则：

- `codex.local` 必须由 Worker 心跳注册。
- 节点必须在线。
- 服务必须上报 `status: "available"`。
- 允许的地址固定为 `127.0.0.1:8390`。
- 客户端发送结构化 WebSocket 消息，不做裸 TCP 转发。

这样手机或 Web 控制台就能和某台真实工作站上的 Codex 通讯，同时不需要把
工作站的本地端口暴露到公网或内网入口。

## 节点端口桥接

节点端口桥接是 AgentGrid 内核能力，用于把一个节点上的本地端口临时映射到
另一个节点的本地端口。典型场景是：A 节点需要用浏览器访问 B 节点上的 Web
调试页，但 B 节点不暴露公网或内网入口。

```text
A 节点浏览器 -> A Worker 监听 127.0.0.1:18080
  -> Hub PortBridge Session
  -> B Worker
  -> B 节点 127.0.0.1:8080
```

规则：

- Worker 主动连接 Hub，节点不需要开放入站端口。
- v1 只支持 TCP。
- A 节点监听地址固定为 `127.0.0.1`。
- B 节点目标地址允许 `127.0.0.1`、`localhost` 或私有 IP。
- Hub 负责创建会话、下发指令、审计、关闭会话。
- Worker 负责真实监听、连接目标端口和双向转发字节流。

CLI 示例：

```bash
agentgrid bridge-port \
  --source-node a-node \
  --target-node b-node \
  --target-port 8080 \
  --source-port 18080 \
  --purpose "让 A 节点浏览器访问 B 节点 Web 调试页"
```

返回的 `Source URL` 可以在 A 节点本机浏览器访问。

## 核心模块

| 模块 | 职责 |
| --- | --- |
| `apps/agentgrid-hub` | Rust Hub、HTTP API、数据存储、运行时循环、Web 托管 |
| `apps/agentgrid-worker` | 跨平台 Worker、任务执行、心跳、产物回传 |
| `apps/agentgrid-cli` | 给人和 AI 使用的命令行 |
| `apps/agentgrid-mcp` | MCP Server |
| `apps/agentgrid-web` | Ant Design Pro 总控台 |
| `crates/agentgrid-protocol` | 共享协议类型 |
| `crates/agentgrid-scheduler` | 调度评分和节点选择 |
| `crates/agentgrid-sdk` | Rust SDK |
| `sdk/node` | Node.js SDK |
| `sdk/python` | Python SDK |
| `sdk/mobile` | iOS / Android 控制台 SDK 标准 |

## 关键标准

- AgentMessage：AI Agent 之间的结构化协作消息。
- AgentTask：任务契约。
- Capability Graph：节点、设备、工具、插件、Probe、证据关系模型。
- Execution Contract：输入、输出、错误、超时、重试、产物、审计和指标。
- Evidence Pipeline：截图、日志、文件、报告、串口输出、DOM、时间线。
- Node Join：机器码 + join token + Hub 授权。
- Job Runtime：lease、checkpoint、shard、reducer、恢复。

## 不做什么

- 不把自然语言解析成动作。
- 不替代 AI 客户端。
- 不做通用 RDP / Jenkins / Ansible / CI 替代品。
- 不假设每个节点能力一样。

AgentGrid 是这些系统下面的结构化运行时：它知道真实机器能做什么、任务该去哪里、证据是什么、时间线上发生了什么。
