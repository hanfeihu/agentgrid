# 愿景

AgentGrid 的愿景是：让 AI Agent 通过开放、结构化、可审计的标准操作真实机器。

大模型会推理、规划、写代码，但很多有价值的动作仍然发生在模型之外：Windows 桌面软件、Linux 构建机、内网浏览器、硬件测试台、串口、烧录器、测试治具、私有网络里的电脑。

AgentGrid 就是为这些真实环境准备的调度和证据层。

## 北极星

我们希望构建一个开放的能力网格，让 AI 客户端可以：

- 发现有哪些机器、设备、桌面、工具、插件和工位
- 提交结构化任务，避免自然语言歧义
- 按能力、资源、Probe 状态、历史成功率和风险调度到正确节点
- 拿回截图、日志、文件、报告、DOM、串口输出和指标
- 在节点掉线后恢复 Job
- 让人类追溯 AI 做过什么
- 让社区通过插件、模板、SDK、MCP 扩展生态

## 生态定位

AgentGrid 应该和 AI 客户端、Skill 系统协作，而不是替代它们。

AI 客户端负责决定“做什么”。AgentGrid 负责决定结构化任务“在哪里、如何运行”，并返回证据。

这让 AgentGrid 很适合连接：

- Codex 类 coding agent
- Claude/Cursor 类开发工具
- MCP 客户端
- 本地优先 Skill 系统
- 硬件测试自动化
- 桌面自动化工位
- 设计和浏览器自动化平台

## 和 Open Design 的结合

[Open Design](https://open-design.ai/zh/) 是一个本地优先、开源、Skill 体系、BYOK 的 AI 设计环境。AgentGrid 和这类系统可以自然结合：

- Open Design Skill 可以注册成 AgentGrid Tool。
- Open Design 工作流可以通过 MCP 或 SDK 调用 AgentGrid。
- AgentGrid Worker 可以在真实机器上执行设计、浏览器、文件、桌面、本地工具操作。
- AgentGrid 可以回传截图、文件、日志、报告作为证据。
- AgentGrid 可以把不同 Skill 调度到不同机器上执行。

边界很清楚：

- Open Design 负责设计工作流、Skill 体验和模型交互。
- AgentGrid 负责机器能力发现、调度、执行、证据、产物和审计。

## 真正的价值

AgentGrid 的长期价值不是“远程执行命令”。这个方向已经很拥挤。

真正的价值是 **AI 操作真实工位**：

- 硬件测试工位
- Windows 桌面工位
- 浏览器/SDK 工位
- 私有网络里的 CI/构建集群
- 设备农场和实验室机器
- 不能暴露公网入口的企业电脑

## 生态接口

AgentGrid 希望吸引这些方向的贡献者：

- Worker 插件
- Tool Manifest
- 能力图谱扩展
- 任务模板
- Job 模板
- 工位 Runbook
- SDK
- MCP 工具
- 证据查看器
- 部署包
- Skill 系统集成

## 设计原则

- 用结构化 payload，而不是自然语言指令。
- 用证据，而不是只看成功/失败。
- 每个节点能力不同，不强求统一。
- Worker 主动连接 Hub，私有网络是一等场景。
- 人类必须能审计 AI 的每一步操作。
- 标准必须机器可读：OpenAPI、JSON Schema、版本化契约。
- 核心运行时不绑定某个 UI 或某个 AI 客户端。

