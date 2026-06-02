# agentgrid-mobile-sdk-swift

Swift SDK for building an AgentGrid mobile console client.

```swift
import AgentGridMobileSDK

let client = AgentGridMobileClient()
let health = try await client.health()
let nodes = try await client.nodes()
let services = try await client.localServices()

let bridge = try await client.createBridgeSession(nodeID: "local-mac")
let item = bridge["item"] as? [String: Any]
let metadata = item?["metadata"] as? [String: Any]
let spec = item?["spec"] as? [String: Any]
let sessionID = metadata?["id"] as? String ?? ""
let token = spec?["token"] as? String
let url = try client.bridgeWebSocketURL(sessionID: sessionID, token: token)
_ = url

let task = try await client.submitTask([
    "tool_id": "desktop.screenshot",
    "title": "capture screen",
    "node_id": "ZZH0610-windows-desktop",
    "payload": [
        "type": "desktop",
        "operation": "screenshot"
    ]
])
```

The SDK calls Hub only. It does not turn iOS into a Worker.

Codex Bridge uses Hub-authenticated WebSocket sessions to reach a registered
`codex.local` service on a node. It does not expose arbitrary local ports.
