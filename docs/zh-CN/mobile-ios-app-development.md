# iOS 手机端 App 开发与真机安装经验

搜索关键词：AgentGrid iOS、手机端、真机安装、Codex Bridge、打开项目、项目目录、devicectl、Xcode、iPhone。

这份文档记录 AgentGrid iOS App 的开发经验和真机安装流程。以后有人打开项目目录，只要搜索上面的关键词，就能找到怎么继续做。

## App 和 SDK 的区别

`apps/agentgrid-mobile-ios` 是真正安装到 iPhone 上的 App。

`sdk/mobile/ios` 是给别的 iOS App 集成 AgentGrid 用的 Swift SDK。

不要把两者混在一起：

- App 负责产品界面、连接 Hub、连接 Codex Bridge、选择项目、聊天。
- SDK 负责封装 Hub API、Bridge API、Port Bridge API。

## 当前 App 目录

```text
apps/agentgrid-mobile-ios/
├── AgentGridMobile.xcodeproj
└── AgentGridMobile/
    ├── App.swift
    ├── ContentView.swift
    ├── Info.plist
    └── Assets.xcassets/
```

当前主要代码集中在：

```text
apps/agentgrid-mobile-ios/AgentGridMobile/ContentView.swift
```

后续如果继续做大，建议把它拆成：

```text
AgentGridMobile/
├── App.swift
├── Models/
├── Services/
├── Screens/
│   ├── Dashboard/
│   ├── Codex/
│   ├── Nodes/
│   ├── Tasks/
│   └── Settings/
├── Components/
└── Theme/
```

## 产品原则

这次最大的经验不是技术，而是产品流程。

正确流程：

```text
进入 Codex
  -> 选择电脑
  -> 连接电脑
  -> 打开项目
  -> 聊天
```

不要把“连接电脑”和“打开项目”合并成一句长按钮。

不要在连接前选择项目。项目路径属于目标电脑，只有连接到某一台电脑后，项目才有意义。

不要展示教育用户的说明文字，例如：

```text
选择一台在线工作电脑，就能像聊天一样使用那台电脑上的 Codex
```

产品界面应该短、克制、明确：

```text
Codex
连接电脑
我的电脑
打开项目
输入消息
```

## 项目目录标准

项目路径必须跟节点绑定。

数据结构里项目必须带：

```text
nodeID
path
name
lastOpenedAt
```

选择项目时必须校验：

```text
project.nodeID == selectedService.nodeID
```

切换电脑时必须清空目录浏览状态：

```text
remoteDirectoryPath
remoteDirectoryNodeID
remoteDirectoryEntries
remoteDirectoryError
```

错误做法：

```text
把 /Users/a1/Desktop/ai-task-scheduler 写死为所有电脑默认路径
```

原因：

- 每台电脑路径不同。
- Windows、Linux、macOS 路径格式不同。
- 手机端不能让用户手写路径，必须通过目标节点浏览。

## 目录浏览

iOS App 通过 Hub 给目标节点提交文件任务：

```json
{
  "type": "file",
  "operation": "list",
  "path": "/Users",
  "recursive": false,
  "max_entries": 300
}
```

任务 labels 必须包含：

```text
node:<node_id>
```

默认起始目录：

```text
local-mac -> /Users/a1/Desktop
Windows   -> C:\Users
macOS     -> /Users
Linux     -> /home
```

这些只是浏览起点，不是全局项目路径。

## Codex Bridge 状态

手机连接 Codex 分两层：

```text
codexBridgeConnected
  表示手机已经连上目标电脑的 Codex Bridge。

codexConnected
  表示已经在某个项目目录里启动 Codex thread，可以聊天。
```

连接电脑时，只建立 bridge：

```text
mobile app -> Hub -> Worker bridge websocket -> node 127.0.0.1:8390
```

打开项目后，必须按项目记录里的 `codexThreadID` 恢复会话：

```json
{
  "method": "thread/resume",
  "params": {
    "threadId": "<项目绑定的 thread id>",
    "cwd": "<目标电脑上的项目路径>",
    "approvalPolicy": "never",
    "sandbox": "danger-full-access",
    "reasoningEffort": "high"
  }
}
```

如果项目没有绑定 thread，才启动新 thread，并保存返回的 `thread.id`：

项目保存的 thread 必须带有当前 App 的 `codexThreadProfile`。当前版本是 `danger-full-access:v2`。如果没有 profile，或 profile 不是当前全权限协议版本，必须清掉旧 thread 并重新创建。这样可以避免手机端恢复早期 `read-only` / `workspace-write` 会话后继续出现“解释但不能干活”的状态。

```json
{
  "method": "thread/start",
  "params": {
    "cwd": "<目标电脑上的项目路径>",
    "approvalPolicy": "never",
    "sandbox": "danger-full-access",
    "reasoningEffort": "high",
    "ephemeral": false,
    "threadSource": "user"
  }
}
```

不要再用 `thread/list` 按 `cwd` 自动恢复最近会话。这个目录可能有 SDK 会话、仓库根目录会话、过期排查会话，自动恢复会让手机端回答错误项目上下文。

不要再使用 `sandbox: "read-only"` + `ephemeral: true`，也不要使用 `workspace-write` 作为手机端默认策略。手机端连接的是用户授权过的工作电脑，默认应请求 `danger-full-access`，让 Codex 具备和电脑端一致的项目操作能力。

发送 `turn/start` 时也要传完整执行策略：

```json
{
  "approvalPolicy": "never",
  "effort": "high",
  "sandboxPolicy": {
    "type": "dangerFullAccess"
  }
}
```

## 图片输入经验

当前项目最低 iOS 版本是 iOS 15，所以不要用 iOS 16+ 的 `PhotosPicker`。

应该用：

```text
PHPickerViewController
UIViewControllerRepresentable
UniformTypeIdentifiers
```

`Info.plist` 需要：

```xml
<key>NSPhotoLibraryUsageDescription</key>
<string>用于选择图片发送给 Codex 分析。</string>
```

图片发送给 Codex Bridge 时，当前做法是压缩后转 data URL。

发送 `turn/start` 时，图片项必须使用 Codex Bridge 当前接受的 `image` variant：

```json
{
  "type": "image",
  "url": "data:image/jpeg;base64,...",
  "mimeType": "image/jpeg",
  "name": "photo.jpg"
}
```

不要使用旧的 OpenAI 风格字段：

```json
{
  "type": "input_image",
  "image_url": "data:image/jpeg;base64,..."
}
```

否则本机 Codex Bridge 会返回：

```text
Invalid request: unknown variant `input_image`, expected one of `text`, `image`, `localImage`, `skill`, `mention`
```

## HTTP Hub 和 ATS

当前 Hub：

```text
http://chenqi.tminos.com:20080/agentgrid
```

iOS 对 HTTP 有限制，所以 `Info.plist` 需要限定域名例外：

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

长期建议给 Hub 上 HTTPS。

## 构建命令

无签名构建，用于快速检查代码：

```bash
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcodebuild \
  -project /Users/a1/Desktop/ai-task-scheduler/apps/agentgrid-mobile-ios/AgentGridMobile.xcodeproj \
  -scheme AgentGridMobile \
  -destination 'generic/platform=iOS' \
  CODE_SIGNING_ALLOWED=NO build
```

真机构建：

```bash
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcodebuild \
  -project /Users/a1/Desktop/ai-task-scheduler/apps/agentgrid-mobile-ios/AgentGridMobile.xcodeproj \
  -scheme AgentGridMobile \
  -destination 'platform=iOS,id=00008140-000E6CA121F1801C' build
```

## 本次真机信息

```text
Device name: 韩
CoreDevice ID: 869D2F1F-DDF6-599C-873A-CDAC9CF13D00
UDID: 00008140-000E6CA121F1801C
Model: iPhone 16 Pro Max
Bundle ID: io.agentgrid.mobile
App display name: AgentGrid
```

查看设备：

```bash
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcrun devicectl list devices
```

## 安装和启动

构建产物：

```text
/Users/a1/Library/Developer/Xcode/DerivedData/AgentGridMobile-ctkpzwwfxavlcnfbwdmfwhhlfyfu/Build/Products/Debug-iphoneos/AgentGrid.app
```

安装：

```bash
APP="$HOME/Library/Developer/Xcode/DerivedData/AgentGridMobile-ctkpzwwfxavlcnfbwdmfwhhlfyfu/Build/Products/Debug-iphoneos/AgentGrid.app"

DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer \
  xcrun devicectl device install app \
  --device 869D2F1F-DDF6-599C-873A-CDAC9CF13D00 \
  "$APP"
```

启动：

```bash
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer \
  xcrun devicectl device process launch \
  --device 869D2F1F-DDF6-599C-873A-CDAC9CF13D00 \
  io.agentgrid.mobile
```

安装并启动：

```bash
APP="$HOME/Library/Developer/Xcode/DerivedData/AgentGridMobile-ctkpzwwfxavlcnfbwdmfwhhlfyfu/Build/Products/Debug-iphoneos/AgentGrid.app"

DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer \
  xcrun devicectl device install app \
  --device 869D2F1F-DDF6-599C-873A-CDAC9CF13D00 \
  "$APP" && \
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer \
  xcrun devicectl device process launch \
  --device 869D2F1F-DDF6-599C-873A-CDAC9CF13D00 \
  io.agentgrid.mobile
```

## 常见问题

### 手机锁屏导致启动失败

如果看到：

```text
Unable to launch io.agentgrid.mobile because the device was not, or could not be, unlocked
```

处理：

- 解锁 iPhone。
- 重新执行启动命令。
- 一般不需要重新安装。

### 未信任开发者

处理：

- 在 iPhone 设置里信任开发者账号。
- 再安装或启动。

### Hub 返回 HTML 或 502

不要把 HTML 显示给用户。产品文案应简化为：

```text
中心入口暂时打不开，请稍后再试
```

### 项目路径串到别的电脑

检查：

- `selectedProject` 是否按 `nodeID` 过滤。
- 切换电脑时是否清空目录浏览状态。
- 最近项目是否来自 `projectsForSelectedService`。
- 是否又写死了某个本机路径。

### 打开 App 后还是旧界面

检查：

- 是否重新真机构建。
- 是否安装的是最新 DerivedData 里的 `AgentGrid.app`。
- iPhone 是否启动了旧缓存进程。必要时退出 App 后再启动。

## 每次修改后的建议流程

1. 先跑无签名构建。
2. 再跑真机构建。
3. 安装到 iPhone。
4. 启动 App。
5. 走一遍 Codex 页面：
   - 我的电脑列表正常
   - 连接电脑
   - 打开项目
   - 输入文字
   - 选择图片

## 相关文件

- `apps/agentgrid-mobile-ios/README.md`
- `apps/agentgrid-mobile-ios/AgentGridMobile/ContentView.swift`
- `apps/agentgrid-mobile-ios/AgentGridMobile/Info.plist`
- `docs/zh-CN/mobile-sdk.md`
- `sdk/mobile/README.md`
