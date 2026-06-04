# AgentGrid Mobile SDK

AgentGrid Mobile SDK is the phone-side console SDK for iOS and Android. It is
for building mobile control consoles, not for turning phones into Worker nodes.

Mobile apps use Hub APIs to inspect the cluster, submit structured tasks, view
execution evidence, open Codex Bridge sessions, and manage Hub-controlled Node
Port Bridge sessions.

## Boundary

Mobile SDKs are:

- Hub API clients
- cluster and node viewers
- structured task submitters
- execution record and artifact viewers
- Codex Bridge control clients
- Node Port Bridge control clients

Mobile SDKs are not:

- Workers
- schedulers
- desktop helpers
- task executors
- natural-language parsers

## Packages

| Platform | Language | Package path | Minimum |
| --- | --- | --- | --- |
| iOS | Swift | `sdk/mobile/ios/agentgrid-mobile-sdk-swift` | iOS 15 |
| Android | Kotlin | `sdk/mobile/android/agentgrid-mobile-sdk-kotlin` | Android API 23 |

Default Hub URL:

```text
http://chenqi.tminos.com:20080/agentgrid
```

Both SDKs accept an optional bearer token and send it as:

```http
Authorization: Bearer <token>
```

## iOS SDK

The iOS SDK is a Swift Package:

```text
sdk/mobile/ios/agentgrid-mobile-sdk-swift
```

Build:

```bash
cd sdk/mobile/ios/agentgrid-mobile-sdk-swift
swift build
```

Basic usage:

```swift
import AgentGridMobileSDK

let client = AgentGridMobileClient()
let health = try await client.health()
let nodes = try await client.nodes()
let tools = try await client.tools()
```

If the Hub still uses plain HTTP, add a scoped App Transport Security exception
to the app target. Prefer HTTPS for production deployments.

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

The Android SDK is a standalone Gradle project:

```text
sdk/mobile/android
```

Module:

```text
:agentgrid-mobile-sdk-kotlin
```

Build and test:

```bash
scripts/check-android-mobile-sdk.sh
```

Or directly:

```bash
cd sdk/mobile/android
./gradlew :agentgrid-mobile-sdk-kotlin:assembleDebug
./gradlew :agentgrid-mobile-sdk-kotlin:testDebugUnitTest
```

Basic usage:

```kotlin
val client = AgentGridMobileClient()
val health = client.health()
val nodes = client.nodes()
val tools = client.tools()
```

The module declares the Android internet permission:

```xml
<uses-permission android:name="android.permission.INTERNET" />
```

## Common Methods

Cluster and standards:

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

Tasks and evidence:

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

Bridge APIs:

- `localServices()`
- `createBridgeSession(nodeID/nodeId, serviceID/serviceId)`
- `bridgeWebSocketURL/bridgeWebSocketUrl(sessionID/sessionId, token)`
- `listPortBridges()`
- `createPortBridge(sourceNodeID/sourceNodeId, targetNodeID/targetNodeId, targetPort, ...)`
- `getPortBridge(portBridgeID/portBridgeId)`
- `closePortBridge(portBridgeID/portBridgeId)`

## Codex Bridge

Codex Bridge lets a mobile app reach a registered node-local service through
Hub. The first built-in service is:

```json
{
  "id": "codex.local",
  "host": "127.0.0.1",
  "port": 8390,
  "capability": "codex.local_bridge"
}
```

Flow:

```text
mobile app -> Hub -> Worker bridge websocket -> 127.0.0.1:8390 on the node
```

iOS:

```swift
let bridge = try await client.createBridgeSession(nodeID: "local-mac")
let item = bridge["item"] as? [String: Any]
let metadata = item?["metadata"] as? [String: Any]
let spec = item?["spec"] as? [String: Any]
let sessionID = metadata?["id"] as? String ?? ""
let token = spec?["token"] as? String
let url = try client.bridgeWebSocketURL(sessionID: sessionID, token: token)
```

Android:

```kotlin
val bridge = client.createBridgeSession(nodeId = "local-mac")
val sessionId = bridge.getJSONObject("item").getJSONObject("metadata").getString("id")
val token = bridge.getJSONObject("item").getJSONObject("spec").optString("token")
val url = client.bridgeWebSocketUrl(sessionId, token)
```

## Node Port Bridge

Node Port Bridge is different from Codex Bridge. It asks Hub to create a
node-to-node TCP bridge. The phone only controls the session; it does not relay
TCP bytes.

Flow:

```text
node A browser/tool -> 127.0.0.1:<source_port>
  -> AgentGrid Hub PortBridge session
  -> node B -> <target_host>:<target_port>
```

Rules:

- v1 supports TCP only.
- `source_bind_host` is `127.0.0.1`.
- `source_bind_port` may be `0`, letting the source Worker choose a free port.
- `target_host` must be loopback, localhost, or a private IP address.
- Source and target Workers must keep their Hub bridge WebSocket connected.

iOS:

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

Android:

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

## Validation

```bash
swift build --package-path sdk/mobile/ios/agentgrid-mobile-sdk-swift
scripts/check-android-mobile-sdk.sh
cargo check -p agentgrid-hub
```

The Hub exposes the machine-readable Mobile SDK standard:

```http
GET /api/runtime-standard/mobile-sdk
```

CLI:

```bash
agentgrid standard mobile-sdk
```
