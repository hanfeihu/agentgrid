# agentgrid-mobile-sdk-kotlin

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
