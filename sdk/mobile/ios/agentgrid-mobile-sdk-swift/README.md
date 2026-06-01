# agentgrid-mobile-sdk-swift

Swift SDK for building an AgentGrid mobile console client.

```swift
import AgentGridMobileSDK

let client = AgentGridMobileClient()
let health = try await client.health()
let nodes = try await client.nodes()

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

