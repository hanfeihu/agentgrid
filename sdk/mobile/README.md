# AgentGrid Mobile SDK

AgentGrid Mobile SDK is the console-client SDK family for phones and tablets.
It is intentionally not a Worker runtime.

Mobile apps use this SDK to:

- View Hub health, nodes, workbenches, devices, and tools.
- Submit structured Agent Runtime tasks.
- Poll task status and task events.
- Read execution records.
- View Hub artifacts such as screenshots, logs, reports, and files.
- Open controlled bridge sessions to registered node-local services such as
  Codex on `127.0.0.1:8390`.

Mobile apps must not execute AgentGrid tasks locally. If a phone app needs a
machine operation, it submits a structured task to Hub and lets Hub schedule an
eligible Worker node.

## Packages

- iOS Swift Package: `sdk/mobile/ios/agentgrid-mobile-sdk-swift`
- Android Kotlin source module: `sdk/mobile/android/agentgrid-mobile-sdk-kotlin`

Default Hub URL:

```text
http://chenqi.tminos.com:20080/agentgrid
```

The default Hub URL is plain HTTP. On iOS, App Transport Security blocks that
request unless the app target has a scoped exception for the Hub domain. Add
this to the app target's `Info.plist` when using the default URL:

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

Prefer switching the Hub to HTTPS and using an `https://` Hub URL when that is
available.

Authentication is deliberately light in v1. Both SDKs accept an optional bearer
token and send it as:

```http
Authorization: Bearer <token>
```

## Codex Bridge

AgentGrid Mobile SDK v1 includes Node Service Bridge helpers. This is not
arbitrary port forwarding. A phone can only connect to services that a Worker
has registered in its heartbeat.

The first built-in service is:

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

Hub endpoints:

```http
GET /api/local-services
POST /api/bridge-sessions
WS /api/bridge-sessions/{session_id}/ws?token=<bridge_token>
WS /api/worker/bridge/ws?node_id=<node_id>
```

Bridge session rules:

- The user must be logged in to create a bridge session.
- Sessions are short-lived and include a one-time bridge token.
- The node must be online and connected to the Worker bridge websocket.
- `codex.local` must report `status: "available"` in the node heartbeat.
- v1 only allows `codex.local` on `127.0.0.1:8390`.

SDK flow:

1. `localServices()` lists nodes that expose `codex.local`.
2. `createBridgeSession(nodeID, "codex.local")` creates a short-lived session.
3. `bridgeWebSocketURL(sessionID, token)` builds the WebSocket endpoint.
4. Send structured messages such as:

```json
{
  "type": "bridge.request",
  "method": "POST",
  "path": "/",
  "headers": {
    "content-type": "application/json"
  },
  "body": {
    "example": true
  }
}
```

Validation:

```bash
scripts/e2e-codex-bridge.sh
```

The E2E test starts a temporary Hub, Worker, and fake `127.0.0.1:8390`
service, then verifies `client -> Hub -> Worker -> local service`.

## Standard

The machine-readable standard is exposed by Hub:

```http
GET /api/runtime-standard/mobile-sdk
```

CLI:

```bash
agentgrid standard mobile-sdk
```
