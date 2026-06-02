# agentgrid-mobile-sdk-kotlin

[Full Mobile SDK docs](../../../../docs/mobile-sdk.md) | [中文说明](../../../../docs/zh-CN/mobile-sdk.md)

Kotlin SDK source for building an AgentGrid mobile console client.

```kotlin
val client = AgentGridMobileClient()
val health = client.health()
val nodes = client.nodes()
val services = client.localServices()

val bridge = client.createBridgeSession(nodeId = "local-mac")
val sessionId = bridge.getJSONObject("item").getJSONObject("metadata").getString("id")
val token = bridge.getJSONObject("item").getJSONObject("spec").optString("token")
val bridgeUrl = client.bridgeWebSocketUrl(sessionId, token)

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
val portBridgeStatus = client.getPortBridge(portBridgeId)
val activePortBridges = client.listPortBridges()
client.closePortBridge(portBridgeId)

val task = client.submitTask(
    JSONObject()
        .put("tool_id", "desktop.screenshot")
        .put("title", "capture screen")
        .put("node_id", "ZZH0610-windows-desktop")
        .put("payload", JSONObject()
            .put("type", "desktop")
            .put("operation", "screenshot"))
)
```

The SDK calls Hub only. It does not turn Android into a Worker.

Codex Bridge uses Hub-authenticated WebSocket sessions to reach a registered
`codex.local` service on a node. It does not expose arbitrary local ports.

Node Port Bridge is different: the SDK asks Hub to create a node-to-node TCP
bridge, then the source node opens the returned loopback URL. The phone is only
the control console.
