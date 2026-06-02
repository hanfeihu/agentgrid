# 成熟度说明

AgentGrid 目前还是 alpha 阶段。这个文档用来区分“稳定核心”和“早期实现”，避免把路线图语言误认为生产承诺。

## 稳定核心

下面这些能力已经实现，后续改动要保持兼容意识：

- Hub REST API：健康检查、节点、任务、Job、产物、工具、节点工具、用户、设置、事件。
- Worker 主动拉取任务和心跳上报模型。
- 节点入网授权：join token、机器指纹、pending 状态、管理员确认。
- 资源感知调度：在线节点、硬指定节点、操作系统、分组、标签、能力、避让节点、首选节点、并发槽位。
- 用户登录、注册、session token 哈希、Argon2 密码存储，以及旧密码哈希自动迁移。
- 管理端会话校验：用户管理、系统设置、节点授权、节点配置、节点删除、手动 Probe、节点纳管计划需要 `admin` 或 `super_admin`。
- 组织隔离 v1：用户、节点、任务、消息、审计事件、产物、节点工具、Tool Probe 等核心记录都带 `organization_id`；核心列表、租约、调度、审计、产物读取按组织隔离。
- Worker 自动更新签名验证：支持 Ed25519 签名元数据、Worker 侧强制签名模式，以及 Hub 侧强制签名清单校验。
- 任务结果和产物的结构化 JSON 记录。
- 给 AI 客户端和人使用的常用 CLI 提交命令。
- OpenAPI 和 JSON Schema 作为机器可读集成契约。

## V1 实现

下面这些能力可以跑，但稳定版之前契约还可能调整：

- Job Runtime 分片、checkpoint、reducer、恢复扫描。
- Tool Probe 和可信调度。
- Worker 插件执行。
- Desktop Helper 截图、点击、输入、按键。
- Evidence Pipeline 和 Artifact Store v2 元数据。
- MCP Server 和 SDK API。
- Web 总控台中除任务、节点、用户、产物之外的扩展页面。

## 已知缺口

- Hub 存储和业务逻辑仍有大量代码集中在 `apps/agentgrid-hub/src/main.rs`，后续要继续拆成聚焦模块。
- Web 总控台功能可用，但 `apps/agentgrid-web/src/main.jsx` 仍然过大，高频页面要继续拆到 page/component 模块。
- Hub、Worker、CLI、浏览器控制台的端到端测试还不够；`scripts/e2e-hub-worker-cli.sh` 已覆盖第一条 Hub + Worker + CLI 命令链路，`scripts/e2e-codex-bridge.sh` 已覆盖 `codex.local` 的节点本地服务桥接链路。
- Worker 更新签名机制已具备，但生产密钥轮换和 CI 强制签名还需要继续硬化。
- 完整浏览器自动化和长期交互式终端通道还需要更强的运行时契约。
- 完整多组织产品流还需要继续硬化：组织切换、组织级管理员角色、组织级节点 ID 唯一键，以及完整 Job/Workflow 租户隔离还没有稳定。

## 发布规则

如果一个能力被标为稳定，改动时必须包含：

- 协议或 API 兼容性说明
- 覆盖该规则的测试
- 必要的数据迁移行为
- 操作方式变化时同步更新中英文文档
