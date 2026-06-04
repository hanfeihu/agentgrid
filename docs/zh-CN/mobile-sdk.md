# AgentGrid Mobile SDK

AgentGrid Mobile SDK 是手机端总控台 SDK，支持 iOS 和 Android。它用于
构建手机控制台，不用于把手机变成 Worker 节点。

手机 App 通过 Hub API 查看集群、提交结构化任务、查看执行证据、打开
Codex Bridge 会话，以及管理 Hub 控制的节点端口桥接会话。

## 边界

Mobile SDK 是：

- Hub API 客户端
- 集群和节点查看器
- 结构化任务提交端
- 执行记录和产物查看器
- Codex Bridge 控制端
- Node Port Bridge 控制端

Mobile SDK 不是：

- Worker
- 调度器
- 桌面助手
- 任务执行器
- 自然语言解析器

## 包

| 平台 | 语言 | 路径 | 最低版本 |
| --- | --- | --- | --- |
| iOS | Swift | `sdk/mobile/ios/agentgrid-mobile-sdk-swift` | iOS 15 |
| Android | Kotlin | `sdk/mobile/android/agentgrid-mobile-sdk-kotlin` | Android API 23 |

实际安装到 iPhone 的 AgentGrid App 在：

```text
apps/agentgrid-mobile-ios
```

iOS App 开发、真机构建、安装和 Codex 项目选择经验见：

```text
apps/agentgrid-mobile-ios/README.md
docs/zh-CN/mobile-ios-app-development.md
```

默认 Hub：

```text
http://chenqi.tminos.com:20080/agentgrid
```

两个 SDK 都支持可选 bearer token：

```http
Authorization: Bearer <token>
```

## iOS SDK

iOS SDK 是 Swift Package：

```text
sdk/mobile/ios/agentgrid-mobile-sdk-swift
```

构建：

```bash
cd sdk/mobile/ios/agentgrid-mobile-sdk-swift
swift build
```

基本用法：

```swift
import AgentGridMobileSDK

let client = AgentGridMobileClient()
let health = try await client.health()
let nodes = try await client.nodes()
let tools = try await client.tools()
```

如果 Hub 仍然是 HTTP，iOS App 需要在目标 App 的 `Info.plist` 增加限定域名的
App Transport Security 例外。生产环境建议切换到 HTTPS。

```xml
<key>NSAppTransportSecurity</key>
<dict>
    <key>NSExceptionDomains</key>
    <dict>
        <key>chenqi.tminos.com</key>
        <dict>
            <key>NSExceptionAllowsInsecureHTTPLoads</key>
            <true/>
            <key>NSIncludesSubdomains</key>
            <true/>
        </dict>
    </dict>
</dict>
```

## Android SDK

Android SDK 是独立 Gradle 工程：

```text
sdk/mobile/android
```

模块：

```text
:agentgrid-mobile-sdk-kotlin
```

构建和测试：

```bash
scripts/check-android-mobile-sdk.sh
```

或者直接执行：

```bash
cd sdk/mobile/android
./gradlew :agentgrid-mobile-sdk-kotlin:assembleDebug
./gradlew :agentgrid-mobile-sdk-kotlin:testDebugUnitTest
```

基本用法：

```kotlin
val client = AgentGridMobileClient()
val health = client.health()
val nodes = client.nodes()
val tools = client.tools()
```

模块已声明网络权限：

```xml
<uses-permission android:name="android.permission.INTERNET" />
```

## 通用方法

集群和标准：

- `health()`
- `runtimeStandard()`
- `mobileSdkStandard()`
- `workbenches()`
- `workbench(workbenchID/workbenchId)`
- `workbenchTimeline(workbenchID/workbenchId)`
- `devices()`
- `evidenceStandard()`
- `nodes()`
- `tools()`

任务和证据：

- `submitTask(request)`
- `runCommand(program, args, nodeID/nodeId, workbenchID/workbenchId, title)`
- `runPlugin(pluginID/pluginId, action, input, nodeID/nodeId, workbenchID/workbenchId, title)`
- `getTask(taskID/taskId)`
- `taskEvents(taskID/taskId)`
- `executionRecord(taskID/taskId)`
- `artifacts()`
- `artifactDownloadURL/artifactDownloadUrl(artifactID/artifactId)`
- `taskTemplates()`
- `startTaskTemplate(templateID/templateId, request)`

桥接能力：

- `localServices()`
- `createBridgeSession(nodeID/nodeId, serviceID/serviceId)`
- `bridgeWebSocketURL/bridgeWebSocketUrl(sessionID/sessionId, token)`
- `listPortBridges()`
- `createPortBridge(sourceNodeID/sourceNodeId, targetNodeID/targetNodeId, targetPort, ...)`
- `getPortBridge(portBridgeID/portBridgeId)`
- `closePortBridge(portBridgeID/portBridgeId)`

## Codex Bridge

Codex Bridge 让手机 App 通过 Hub 连接节点上已注册的本地服务。第一个内置服务是：

```json
{
  "id": "codex.local",
  "host": "127.0.0.1",
  "port": 8390,
  "capability": "codex.local_bridge"
}
```

链路：

```text
mobile app -> Hub -> Worker bridge websocket -> 节点上的 127.0.0.1:8390
```

iOS：

```swift
let bridge = try await client.createBridgeSession(nodeID: "local-mac")
let item = bridge["item"] as? [String: Any]
let metadata = item?["metadata"] as? [String: Any]
let spec = item?["spec"] as? [String: Any]
let sessionID = metadata?["id"] as? String ?? ""
let token = spec?["token"] as? String
let url = try client.bridgeWebSocketURL(sessionID: sessionID, token: token)
```

Android：

```kotlin
val bridge = client.createBridgeSession(nodeId = "local-mac")
val sessionId = bridge.getJSONObject("item").getJSONObject("metadata").getString("id")
val token = bridge.getJSONObject("item").getJSONObject("spec").optString("token")
val url = client.bridgeWebSocketUrl(sessionId, token)
```

## 节点端口桥接

节点端口桥接和 Codex Bridge 不一样。它让手机请求 Hub 创建一个节点到节点的
TCP 桥接。手机只负责控制会话，不转发 TCP 字节。

链路：

```text
A 节点浏览器/工具 -> 127.0.0.1:<source_port>
  -> AgentGrid Hub PortBridge session
  -> B 节点 -> <target_host>:<target_port>
```

规则：

- v1 只支持 TCP。
- `source_bind_host` 固定为 `127.0.0.1`。
- `source_bind_port` 可以是 `0`，表示让源 Worker 自动选择可用端口。
- `target_host` 必须是 loopback、localhost 或私有 IP。
- 源 Worker 和目标 Worker 都必须保持 Hub bridge WebSocket 连接。

iOS：

```swift
let portBridge = try await client.createPortBridge(
    sourceNodeID: "local-mac",
    targetNodeID: "linux-worker-01",
    targetPort: 8080,
    sourceBindPort: 18080,
    purpose: "Open node-local web tool from the source node"
)

let item = portBridge["item"] as? [String: Any]
let metadata = item?["metadata"] as? [String: Any]
let portBridgeID = metadata?["id"] as? String ?? ""
let status = try await client.getPortBridge(portBridgeID)
try await client.closePortBridge(portBridgeID)
```

Android：

```kotlin
val portBridge = client.createPortBridge(
    sourceNodeId = "local-mac",
    targetNodeId = "linux-worker-01",
    targetPort = 8080,
    sourceBindPort = 18080,
    purpose = "Open node-local web tool from the source node",
)

val portBridgeId = portBridge
    .getJSONObject("item")
    .getJSONObject("metadata")
    .getString("id")
val status = client.getPortBridge(portBridgeId)
client.closePortBridge(portBridgeId)
```

## 验证

```bash
swift build --package-path sdk/mobile/ios/agentgrid-mobile-sdk-swift
scripts/check-android-mobile-sdk.sh
cargo check -p agentgrid-hub
```

Hub 暴露机器可读的 Mobile SDK 标准：

```http
GET /api/runtime-standard/mobile-sdk
```

CLI：

```bash
agentgrid standard mobile-sdk
```
