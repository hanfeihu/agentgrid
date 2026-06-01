# AgentGrid Mobile SDK

AgentGrid Mobile SDK is the console-client SDK family for phones and tablets.
It is intentionally not a Worker runtime.

Mobile apps use this SDK to:

- View Hub health, nodes, workbenches, devices, and tools.
- Submit structured Agent Runtime tasks.
- Poll task status and task events.
- Read execution records.
- View Hub artifacts such as screenshots, logs, reports, and files.

Mobile apps must not execute AgentGrid tasks locally. If a phone app needs a
machine operation, it submits a structured task to Hub and lets Hub schedule an
eligible Worker node.

## Packages

- iOS Swift Package: `sdk/mobile/ios/agentgrid-mobile-sdk-swift`
- Android Kotlin source module: `sdk/mobile/android/agentgrid-mobile-sdk-kotlin`

Default Hub URL:

```text
https://hub.example.com/agentgrid
```

The default Hub URL is plain HTTP. On iOS, App Transport Security blocks that
request unless the app target has a scoped exception for the Hub domain. Add
this to the app target's `Info.plist` when using the default URL:

```xml
<key>NSAppTransportSecurity</key>
<dict>
    <key>NSExceptionDomains</key>
    <dict>
        <key>hub.example.com</key>
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

## Standard

The machine-readable standard is exposed by Hub:

```http
GET /api/runtime-standard/mobile-sdk
```

CLI:

```bash
agentgrid standard mobile-sdk
```
