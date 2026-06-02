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

let portBridge = try await client.createPortBridge(
    sourceNodeID: "local-mac",
    targetNodeID: "linux-worker-01",
    targetPort: 8080,
    sourceBindPort: 18080,
    purpose: "Open node-local web tool from the source node"
)
let portBridgeItem = portBridge["item"] as? [String: Any]
let portBridgeID = (portBridgeItem?["metadata"] as? [String: Any])?["id"] as? String ?? ""
let portBridgeStatus = try await client.getPortBridge(portBridgeID)
_ = portBridgeStatus
let activePortBridges = try await client.listPortBridges()
_ = activePortBridges
try await client.closePortBridge(portBridgeID)

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

Node Port Bridge is different: the SDK asks Hub to create a node-to-node TCP
bridge, then the source node opens the returned loopback URL. The phone is only
the control console.
