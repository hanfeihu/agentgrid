# AgentGrid iOS App 开发与真机安装记录

搜索关键词：iOS、手机端、AgentGrid App、Codex Bridge、打开项目、真机安装、devicectl、Xcode、iPhone、工作目录、项目选择。

这个目录是 AgentGrid 手机端 iOS App，不是 Mobile SDK。SDK 在 `sdk/mobile/ios`，这里是可以安装到 iPhone 上的实际 App。

## 目录

```text
apps/agentgrid-mobile-ios/
├── AgentGridMobile.xcodeproj
└── AgentGridMobile/
    ├── App.swift
    ├── ContentView.swift
    ├── Info.plist
    └── Assets.xcassets/
```

主要文件：

- `AgentGridMobile/ContentView.swift`：当前主要业务和界面都在这里。
- `AgentGridMobile/App.swift`：App 入口。
- `AgentGridMobile/Info.plist`：App 名称、HTTP Hub 例外、照片权限。
- `AgentGridMobile/Assets.xcassets`：App 图标。

当前 App 显示名是 `AgentGrid`，不要显示成 `AgentGrid Mobile`。

## 产品边界

手机端是控制台客户端，不是 Worker 节点。

手机端应该做：

- 查看 Hub、节点、任务。
- 选择一台电脑。
- 连接这台电脑上的 Codex Bridge。
- 连接成功后，从这台电脑浏览并打开项目。
- 打开项目后再进入聊天输入。
- 支持图片输入。

手机端不应该做：

- 让用户手写电脑路径。
- 把某台电脑的路径当成全局路径。
- 在连接前强制选择项目。
- 展示大量调试文字或教育用户的说明。
- 把“连接电脑”和“打开项目”揉成一个长按钮。

推荐流程：

```text
Codex 页面
  -> 我的电脑
  -> 连接电脑
  -> 打开项目
  -> 进入聊天
```

## 项目路径规则

项目目录必须属于某一台节点电脑。

正确规则：

- 每个项目记录带 `nodeID`。
- `selectedProject` 必须匹配当前 `selectedService.nodeID`。
- 切换电脑时，目录浏览状态要清空。
- 最近项目按当前电脑过滤。
- 连接 Codex 后再打开项目。

错误做法：

```text
把 /Users/a1/Desktop/ai-task-scheduler 写死为所有电脑的默认项目
```

原因：

- Windows、Linux、macOS 路径完全不同。
- 每台电脑的项目目录也不同。
- 手机端必须通过目标 Worker 的 file/list 任务浏览目标电脑目录。

## 目录浏览实现

手机端通过 Hub 提交 AgentGrid 文件任务，让目标节点读取目录：

```json
{
  "type": "file",
  "operation": "list",
  "path": "/Users",
  "recursive": false,
  "max_entries": 300
}
```

任务标签必须包含目标节点：

```text
node:<node_id>
```

默认起始目录：

- `local-mac`：`/Users/a1/Desktop`
- Windows：`C:\Users`
- macOS：`/Users`
- Linux：`/home`

这些只是起点，不是全局工作目录。

## Codex Bridge 流程

手机端连接的是节点上注册的本地服务：

```text
mobile app -> Hub -> Worker bridge websocket -> node 127.0.0.1:8390
```

连接分两层：

- `codexBridgeConnected`：手机已经连上这台电脑的 Codex 桥。
- `codexConnected`：已经为某个项目启动 Codex thread，可以聊天。

连接电脑时只建立桥，不应该要求项目。

每个项目记录自己的 `codexThreadID`。打开项目时只能恢复这个项目绑定的会话，不要按 `cwd` 自动查找最近会话。

如果项目已有 `codexThreadID`，就恢复：

```json
{
  "method": "thread/resume",
  "params": {
    "threadId": "<项目绑定的 thread id>",
    "cwd": "<当前节点上的项目路径>",
    "approvalPolicy": "never",
    "sandbox": "danger-full-access",
    "reasoningEffort": "high"
  }
}
```

只有项目保存的 `codexThreadProfile` 等于当前 App 的全权限协议版本时，才允许恢复旧会话。当前版本是 `danger-full-access:v2`。旧版本会话要清掉并重新创建，避免手机端继续接入以前的只读/半权限线程。

如果没有绑定会话，才新建，并把返回的 `thread.id` 和 `codexThreadProfile` 保存回项目：

```json
{
  "method": "thread/start",
  "params": {
    "cwd": "<当前节点上的项目路径>",
    "approvalPolicy": "never",
    "sandbox": "danger-full-access",
    "reasoningEffort": "high",
    "ephemeral": false
  }
}
```

不要用 `thread/list` + `cwd` 自动恢复最新会话。一个目录里可能有很多历史线程，自动捞最近会把手机端接到过期上下文，导致 Codex 答错项目。

不要默认用 `read-only + ephemeral`，也不要用 `workspace-write`。手机端是用户明确连接自己电脑后的远程 Codex 控制台，默认应使用 `danger-full-access`，让 Codex 具备和电脑端一致的项目操作能力。

## UI 经验

这次踩过的产品问题：

- “打开项目并开始聊天”太啰嗦，改为外层只显示“连接电脑”。
- 连接前不放“选择项目”，因为项目是连接到某台电脑后才有意义。
- “我的电脑”列表上方不要再放一个重复的“本机 Mac”大卡片。
- 不要写“手机上的 Codex / 选择一台在线工作电脑...”这种教育文案。
- 设置页不要放“测试 Hub / 结果”这种工程师调试入口。
- 聊天页吊起输入法时，输入框要贴底恢复，不能悬在页面中间。

当前 Codex 页期望：

```text
未连接：
  顶部：Codex + 状态
  主按钮：连接电脑
  列表：我的电脑

已连接但未打开项目：
  顶部：当前电脑
  主区域：打开项目
  最近项目：只显示当前电脑的项目

已打开项目：
  顶部：电脑名 + 项目名
  主区域：聊天消息
  底部：图片按钮 + 多行输入框 + 发送
```

## 图片输入

项目目标 iOS 最低版本是 iOS 15，所以不要直接用 iOS 16+ 的 `PhotosPicker`。

当前做法：

- 使用 `PHPickerViewController`。
- 通过 `UIViewControllerRepresentable` 包装。
- 使用 `UniformTypeIdentifiers` 读取图片数据。
- 图片压缩后以 data URL 形式发给 Codex Bridge。

发送给 Codex Bridge 的 `turn/start` 输入项必须是：

```json
{
  "type": "image",
  "url": "data:image/jpeg;base64,...",
  "mimeType": "image/jpeg",
  "name": "photo.jpg"
}
```

不要发：

```json
{
  "type": "input_image",
  "image_url": "data:image/jpeg;base64,..."
}
```

这个格式会被本机 Codex Bridge 拒绝，错误类似：

```text
Invalid request: unknown variant `input_image`, expected one of `text`, `image`, `localImage`, `skill`, `mention`
```

需要的权限：

```xml
<key>NSPhotoLibraryUsageDescription</key>
<string>用于选择图片发送给 Codex 分析。</string>
```

## Hub HTTP 例外

当前默认 Hub：

```text
http://chenqi.tminos.com:20080/agentgrid
```

iOS 默认会拦截 HTTP，所以 `Info.plist` 里需要 ATS 例外：

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

生产环境建议改成 HTTPS。

## 构建

无签名编译，用来快速验证 Swift 代码：

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

本次设备信息：

```text
Device name: 韩
CoreDevice ID: 869D2F1F-DDF6-599C-873A-CDAC9CF13D00
UDID: 00008140-000E6CA121F1801C
Model: iPhone 16 Pro Max
Bundle ID: io.agentgrid.mobile
```

先查看当前连接设备：

```bash
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcrun devicectl list devices
```

## 安装和启动

构建产物路径：

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

### 启动失败：device locked

现象：

```text
Unable to launch io.agentgrid.mobile because the device was not, or could not be, unlocked
```

处理：

- 手机解锁。
- 再执行启动命令。
- 安装通常已经成功，不需要重复构建。

### 设备未信任开发者

处理：

- iPhone 设置里信任开发者账号。
- 再安装或启动。

### App 里 Hub 返回 502 或 HTML

不要把 HTML 原文展示给用户。产品层应该显示：

```text
中心入口暂时打不开，请稍后再试
```

### `PhotosPicker` 编译失败

如果目标仍是 iOS 15，不要用 `PhotosPicker`。使用 `PHPickerViewController` 包装。

### 项目目录显示成别的电脑路径

检查：

- `CodexProject.nodeID`
- `selectedProject` 是否按当前 `selectedService.nodeID` 过滤
- 切换电脑时是否调用目录浏览状态清理
- 最近项目列表是否只取 `projectsForSelectedService`

## 每次改完建议验证

至少做：

```bash
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcodebuild \
  -project /Users/a1/Desktop/ai-task-scheduler/apps/agentgrid-mobile-ios/AgentGridMobile.xcodeproj \
  -scheme AgentGridMobile \
  -destination 'generic/platform=iOS' \
  CODE_SIGNING_ALLOWED=NO build
```

如果要交给用户看，再做真机构建、安装、启动。

## 相关文档

- `docs/zh-CN/mobile-ios-app-development.md`
- `docs/zh-CN/mobile-sdk.md`
- `sdk/mobile/README.md`
