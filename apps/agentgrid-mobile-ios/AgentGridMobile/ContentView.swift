import Foundation
import PhotosUI
import SwiftUI
import UniformTypeIdentifiers
#if canImport(UIKit)
import UIKit
#endif

struct AgentNode: Identifiable, Hashable {
    let id: String
    let name: String
    let os: String
    let address: String
    let state: String
    let cpuCores: Int
    let cpuUsagePercent: Double
    let memoryMB: Int
    let memoryUsedMB: Int
    let diskTotalMB: Int
    let diskFreeMB: Int
    let maxConcurrentJobs: Int
    let runningJobs: Int
    let capabilities: [String]
    let tags: [String]

    var isOnline: Bool {
        state == "online"
    }

    var memoryUsagePercent: Double {
        guard memoryMB > 0 else { return 0 }
        return Double(memoryUsedMB) / Double(memoryMB) * 100
    }

    var diskUsagePercent: Double {
        guard diskTotalMB > 0 else { return 0 }
        return Double(diskTotalMB - diskFreeMB) / Double(diskTotalMB) * 100
    }

    var shortOS: String {
        if os.lowercased().contains("windows") { return "Windows" }
        if os.lowercased().contains("darwin") { return "macOS" }
        if os.lowercased().contains("ubuntu") { return "Ubuntu" }
        return os.isEmpty ? "未知系统" : os
    }
}

struct LocalService: Identifiable, Hashable {
    let id: String
    let nodeID: String
    let nodeName: String
    let serviceID: String
    let name: String
    let status: String
    let nodeState: String
    let bridgeWorkerConnected: Bool

    var serviceReady: Bool {
        status == "available" && nodeState == "online"
    }

    var isAvailable: Bool {
        serviceReady && bridgeWorkerConnected
    }

    var statusTitle: String {
        if isAvailable { return "可用" }
        if nodeState != "online" { return "电脑离线" }
        if serviceReady { return "Codex 准备中" }
        return "不可用"
    }

    var businessStatusTitle: String {
        if isAvailable { return "可聊天" }
        if nodeState != "online" { return "电脑离线" }
        if serviceReady { return "Codex 准备中" }
        return "Codex 未启动"
    }

    var businessSubtitle: String {
        if isAvailable { return "可以连接这台电脑上的 Codex" }
        if nodeState != "online" { return "这台电脑暂时不能使用" }
        if serviceReady { return "电脑在线，Codex 正在准备" }
        return "请先在这台电脑上启动 Codex"
    }

    var canTryConnect: Bool {
        serviceReady
    }
}

struct BridgeSessionInfo: Hashable {
    let id: String
    let token: String
    let workerConnected: Bool
}

struct CodexChatMessage: Identifiable, Hashable {
    let id = UUID()
    let role: String
    var text: String
    var isStreaming: Bool = false
    var attachments: [CodexImageAttachment] = []

    var isUser: Bool {
        role == "user"
    }

    var isSystem: Bool {
        role == "system"
    }
}

struct CodexImageAttachment: Identifiable, Hashable {
    let id = UUID()
    let name: String
    let mimeType: String
    let data: Data

    var base64: String {
        data.base64EncodedString()
    }

    var dataURL: String {
        "data:\(mimeType);base64,\(base64)"
    }

    var sizeText: String {
        let kb = Double(data.count) / 1024.0
        if kb >= 1024 {
            return String(format: "%.1f MB", kb / 1024.0)
        }
        return String(format: "%.0f KB", kb)
    }

    #if canImport(UIKit)
    var image: UIImage? {
        UIImage(data: data)
    }
    #endif
}

struct RemoteDirectoryEntry: Identifiable, Hashable {
    let path: String
    let isDirectory: Bool
    let size: Int64

    var id: String { path }

    var name: String {
        let trimmed = path.trimmingCharacters(in: CharacterSet(charactersIn: "/\\ "))
        guard !trimmed.isEmpty else { return path }
        let separators = CharacterSet(charactersIn: "/\\")
        return trimmed.components(separatedBy: separators).filter { !$0.isEmpty }.last ?? path
    }
}

struct CodexProject: Identifiable, Hashable, Codable {
    let id: String
    let nodeID: String
    var name: String
    var path: String
    var lastOpenedAt: String
    var codexThreadID: String?
    var codexThreadProfile: String?

    init(
        nodeID: String,
        name: String,
        path: String,
        lastOpenedAt: String,
        codexThreadID: String? = nil,
        codexThreadProfile: String? = nil
    ) {
        self.nodeID = nodeID
        self.path = path
        self.name = name.isEmpty ? Self.name(from: path) : name
        self.lastOpenedAt = lastOpenedAt
        self.codexThreadID = codexThreadID
        self.codexThreadProfile = codexThreadProfile
        id = Self.makeID(nodeID: nodeID, path: path)
    }

    var displayName: String {
        name.isEmpty ? Self.name(from: path) : name
    }

    var compactPath: String {
        if path.count > 72 {
            return "..." + String(path.suffix(69))
        }
        return path
    }

    static func makeID(nodeID: String, path: String) -> String {
        "\(nodeID)::\(path.trimmingCharacters(in: .whitespacesAndNewlines).lowercased())"
    }

    static func name(from path: String) -> String {
        let trimmed = path.trimmingCharacters(in: CharacterSet(charactersIn: "/\\ "))
        guard !trimmed.isEmpty else { return "未命名项目" }
        let separators = CharacterSet(charactersIn: "/\\")
        return trimmed.components(separatedBy: separators).filter { !$0.isEmpty }.last ?? trimmed
    }
}

struct AgentTaskItem: Identifiable, Hashable {
    let id: String
    let title: String
    let state: String
    let priority: String
    let owner: String
    let nodeID: String
    let updatedAt: String
    let labels: [String]
    let errorMessage: String?

    var stateTitle: String {
        switch state {
        case "assigned": return "排队"
        case "in_progress": return "执行中"
        case "done": return "完成"
        case "failed": return "失败"
        case "blocked": return "阻塞"
        case "cancelled": return "取消"
        default: return state.isEmpty ? "未知" : state
        }
    }

    var shortUpdatedAt: String {
        String(updatedAt.prefix(19)).replacingOccurrences(of: "T", with: " ")
    }
}

enum ReadinessState: Hashable {
    case ok
    case pending
    case warning

    var title: String {
        switch self {
        case .ok: return "正常"
        case .pending: return "待连接"
        case .warning: return "需处理"
        }
    }

    var icon: String {
        switch self {
        case .ok: return "checkmark.circle.fill"
        case .pending: return "circle.dashed"
        case .warning: return "exclamationmark.triangle.fill"
        }
    }

    var color: Color {
        switch self {
        case .ok: return .green
        case .pending: return .gray
        case .warning: return .orange
        }
    }
}

struct ReadinessStep: Identifiable, Hashable {
    let id: String
    let title: String
    let detail: String
    let state: ReadinessState
}

enum AppTab: String, CaseIterable, Identifiable {
    case dashboard
    case codex
    case nodes
    case tasks
    case settings

    var id: String { rawValue }

    var title: String {
        switch self {
        case .dashboard: return "总览"
        case .codex: return "Codex"
        case .nodes: return "节点"
        case .tasks: return "任务"
        case .settings: return "设置"
        }
    }

    var icon: String {
        switch self {
        case .dashboard: return "square.grid.2x2.fill"
        case .codex: return "bolt.horizontal.circle.fill"
        case .nodes: return "server.rack"
        case .tasks: return "list.bullet.rectangle.fill"
        case .settings: return "gearshape.fill"
        }
    }
}

@MainActor
final class AgentGridMobileModel: ObservableObject {
    @Published var hubURL: String {
        didSet { UserDefaults.standard.set(hubURL, forKey: "agentgrid.hubURL") }
    }

    @Published var email: String {
        didSet { UserDefaults.standard.set(email, forKey: "agentgrid.email") }
    }

    @Published var password = ""

    @Published var token: String {
        didSet { UserDefaults.standard.set(token, forKey: "agentgrid.token") }
    }

    @Published var nodes: [AgentNode] = []
    @Published var services: [LocalService] = []
    @Published var tasks: [AgentTaskItem] = []
    @Published var selectedServiceID = ""
    @Published var requestMethod = "GET"
    @Published var requestPath = "/healthz"
    @Published var requestBody = "{\n  \"hello\": \"agentgrid-mobile\"\n}"
    @Published var hubState = "未连接"
    @Published var activityText = "等待操作"
    @Published var rawResponseText = ""
    @Published var lastSessionID = ""
    @Published var isLoading = false
    @Published var codexBridgeConnected = false
    @Published var codexConnected = false
    @Published var codexThreadID = ""
    @Published var codexSessionMode = ""
    @Published var codexChatInput = ""
    @Published var codexMessages: [CodexChatMessage] = []
    @Published var projects: [CodexProject] = [] {
        didSet { saveProjects() }
    }
    @Published var selectedProjectID = "" {
        didSet { UserDefaults.standard.set(selectedProjectID, forKey: "agentgrid.selectedProjectID") }
    }
    @Published var pendingImages: [CodexImageAttachment] = []
    @Published var remoteDirectoryPath = ""
    @Published var remoteDirectoryNodeID = ""
    @Published var remoteDirectoryEntries: [RemoteDirectoryEntry] = []
    @Published var remoteDirectoryError = ""
    @Published var isBrowsingProjects = false

    private let defaultHubURL = "http://chenqi.tminos.com:20080/agentgrid"
    private let demoEmail = "mobile@agentgrid.local"
    private let demoPassword = "AgentGridMobile2026!"
    private static let projectsStorageKey = "agentgrid.codexProjects"
    private static let defaultReasoningEffort = "high"
    private static let codexThreadProfile = "danger-full-access:v3"
    private static let maxPendingImages = 2
    private static let maxImageBytes = 120_000
    private static let maxBridgeEnvelopeBytes = 420_000
    private var codexBridgeTask: URLSessionWebSocketTask?
    private var codexReceiveTask: Task<Void, Never>?
    private var codexRequestID = 1
    private var codexPendingMethods: [Int: String] = [:]
    private var currentAssistantMessageID: UUID?
    private var userSelectedServiceID = false

    func selectService(_ service: LocalService) {
        let previousNodeID = selectedService?.nodeID
        selectedServiceID = service.id
        userSelectedServiceID = true
        if previousNodeID != service.nodeID {
            resetRemoteDirectoryBrowser()
        }
        ensureProjectSelection(for: service)
        disconnectCodexChat(message: nil)
        activityText = "已选择 \(service.nodeName)"
    }

    init() {
        hubURL = UserDefaults.standard.string(forKey: "agentgrid.hubURL") ?? defaultHubURL
        email = UserDefaults.standard.string(forKey: "agentgrid.email") ?? ""
        token = UserDefaults.standard.string(forKey: "agentgrid.token") ?? ""
        selectedProjectID = UserDefaults.standard.string(forKey: "agentgrid.selectedProjectID") ?? ""
        projects = Self.loadStoredProjects(storageKey: Self.projectsStorageKey)
    }

    var isAuthenticated: Bool {
        !token.isEmpty
    }

    var selectedService: LocalService? {
        services.first { $0.id == selectedServiceID }
    }

    var selectedProject: CodexProject? {
        guard let service = selectedService else { return nil }
        return projects.first { $0.id == selectedProjectID && $0.nodeID == service.nodeID }
    }

    var projectsForSelectedService: [CodexProject] {
        guard let service = selectedService else { return [] }
        return projects
            .filter { $0.nodeID == service.nodeID }
            .sorted { $0.lastOpenedAt > $1.lastOpenedAt }
    }

    var selectedProjectTitle: String {
        guard selectedService != nil else { return "先选择电脑" }
        return selectedProject?.displayName ?? "请选择项目"
    }

    var selectedProjectDetail: String {
        guard let service = selectedService else {
            return "先选择一台工作电脑，再从这台电脑里打开项目目录。"
        }
        return selectedProject?.compactPath ?? "从 \(service.nodeName) 浏览并选择目录，Codex 只会在这个电脑的项目里工作。"
    }

    var selectedWorkingDirectory: String {
        selectedProject?.path ?? ""
    }

    var remoteDirectoryTitle: String {
        if remoteDirectoryPath.isEmpty {
            return "选择文件夹"
        }
        return CodexProject.name(from: remoteDirectoryPath)
    }

    var remoteDirectoryParentPath: String? {
        Self.parentDirectory(of: remoteDirectoryPath)
    }

    var canConnectCodexChat: Bool {
        guard !isLoading, let service = selectedService else { return !isLoading }
        return service.canTryConnect
    }

    var canSendCodexMessage: Bool {
        codexConnected
            && !codexThreadID.isEmpty
            && (!codexChatInput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !pendingImages.isEmpty)
    }

    var onlineNodes: [AgentNode] {
        nodes.filter(\.isOnline)
    }

    var availableServices: [LocalService] {
        services.filter(\.isAvailable)
    }

    var selectedServiceSummary: String {
        guard let service = selectedService else {
            return "还没有发现可聊天的电脑"
        }
        return "\(service.nodeName) / \(service.businessStatusTitle)"
    }

    var compactActivityText: String {
        let trimmed = activityText
            .replacingOccurrences(of: "\\s+", with: " ", options: .regularExpression)
            .trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.count > 72 {
            return String(trimmed.prefix(72)) + "..."
        }
        return trimmed.isEmpty ? "等待操作" : trimmed
    }

    var controlCenterStatusTitle: String {
        if isLoading { return "正在连接" }
        return hubState == "在线" ? "控制中心在线" : "等待连接"
    }

    var controlCenterStatusDetail: String {
        if hubState == "在线" {
            return "已连接到 AgentGrid 控制中心，可以查看节点和发起对话。"
        }
        return "保存服务器地址后，AgentGrid 会自动确认连接状态。"
    }

    var accountStatusTitle: String {
        isAuthenticated ? "账号已登录" : "账号未登录"
    }

    var accountStatusDetail: String {
        isAuthenticated ? "可以访问你的节点、任务和 Codex 对话入口。" : "登录后才能连接工作电脑和查看任务。"
    }

    var totalOnlineCores: Int {
        onlineNodes.reduce(0) { $0 + $1.cpuCores }
    }

    var totalOnlineMemoryMB: Int {
        onlineNodes.reduce(0) { $0 + $1.memoryMB }
    }

    var windowsDesktopNodes: Int {
        nodes.filter { $0.shortOS == "Windows" && $0.capabilities.contains("desktop") }.count
    }

    var runningTasks: [AgentTaskItem] {
        tasks.filter { $0.state == "in_progress" }
    }

    var failedTasks: [AgentTaskItem] {
        tasks.filter { $0.state == "failed" }
    }

    var doneTasks: [AgentTaskItem] {
        tasks.filter { $0.state == "done" }
    }

    var activeTaskCount: Int {
        tasks.filter { $0.state == "assigned" || $0.state == "in_progress" }.count
    }

    var systemHealthTitle: String {
        if hubState != "在线" { return "服务未连接" }
        if availableServices.isEmpty { return "等待工作电脑" }
        if !failedTasks.isEmpty { return "有待处理任务" }
        return "可以开始"
    }

    var systemHealthColor: Color {
        if hubState != "在线" || availableServices.isEmpty { return .orange }
        if !failedTasks.isEmpty { return .orange }
        return .green
    }

    var primaryCodexActionTitle: String {
        codexBridgeConnected ? "继续" : "连接电脑"
    }

    var codexBusinessStatusTitle: String {
        if codexConnected { return "聊天中" }
        if codexBridgeConnected { return "已连接" }
        guard let service = selectedService else { return "请选择一台工作电脑" }
        if service.isAvailable { return "准备就绪" }
        if service.nodeState != "online" { return "电脑不在线" }
        if service.serviceReady { return "聊天正在准备" }
        return "Codex 未启动"
    }

    var codexBusinessStatusDetail: String {
        if codexConnected, let project = selectedProject {
            return project.compactPath
        }
        if codexBridgeConnected {
            return selectedService?.nodeName ?? "工作电脑"
        }
        guard let service = selectedService else {
            return "请选择电脑"
        }
        if service.isAvailable {
            return service.nodeName
        }
        if service.nodeState != "online" {
            return "\(service.nodeName) 离线"
        }
        if service.serviceReady {
            return "\(service.nodeName) 准备中"
        }
        return "\(service.nodeName) 未启动 Codex"
    }

    var codexBusinessStatusColor: Color {
        if codexBridgeConnected { return .green }
        if codexConnected { return .green }
        if selectedService?.isAvailable == true { return AppTheme.accent }
        return .orange
    }

    var codexReadinessSteps: [ReadinessStep] {
        let service = selectedService
        return [
            ReadinessStep(
                id: "hub",
                title: "服务连接",
                detail: hubState == "在线" ? "服务在线" : "服务未确认",
                state: hubState == "在线" ? .ok : .warning
            ),
            ReadinessStep(
                id: "auth",
                title: "移动账号",
                detail: isAuthenticated ? "已登录" : "连接时会自动登录测试账号",
                state: isAuthenticated ? .ok : .pending
            ),
            ReadinessStep(
                id: "node",
                title: "工作电脑",
                detail: service?.nodeName ?? "未发现可用电脑",
                state: service == nil ? .warning : (service?.nodeState == "online" ? .ok : .warning)
            ),
            ReadinessStep(
                id: "service",
                title: "Codex 状态",
                detail: service?.businessStatusTitle ?? "未发现电脑",
                state: service?.isAvailable == true ? .ok : .warning
            ),
            ReadinessStep(
                id: "bridge",
                title: "手机聊天",
                detail: codexBridgeConnected ? "已连接" : "等待连接",
                state: codexBridgeConnected ? .ok : .pending
            ),
        ]
    }

    func refreshAll() async {
        await run(successMessage: "状态已刷新") {
            try await loadHealth()
            try await loadNodes()
            try await loadServices()
            try await loadTasks()
        }
    }

    func login() async {
        guard !email.isEmpty, !password.isEmpty else {
            activityText = "请输入邮箱和密码"
            return
        }
        await run(successMessage: "登录成功") {
            try await loginWith(email: email, password: password)
            try await loadServices()
        }
    }

    func logout() {
        token = ""
        password = ""
        lastSessionID = ""
        activityText = "已退出登录"
        rawResponseText = ""
    }

    func selectProject(_ project: CodexProject) {
        guard selectedService?.nodeID == project.nodeID else {
            activityText = "这个项目不属于当前电脑"
            return
        }
        selectedProjectID = project.id
        touchProject(project)
        codexMessages = []
        pendingImages = []
        activityText = "已打开 \(project.displayName)"
        if codexBridgeConnected {
            startCodexThreadForSelectedProject()
        }
    }

    func addProjectFromInput() {
        let path = remoteDirectoryPath.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !path.isEmpty else {
            activityText = "请先选择一个文件夹"
            return
        }
        useRemoteDirectory(path)
    }

    func useRemoteDirectory(_ path: String) {
        guard let service = selectedService else {
            activityText = "请先选择一台工作电脑"
            return
        }
        let project = CodexProject(
            nodeID: service.nodeID,
            name: CodexProject.name(from: path),
            path: path,
            lastOpenedAt: Self.isoNow()
        )
        upsertProject(project)
        selectedProjectID = project.id
        codexMessages = []
        pendingImages = []
        activityText = "已打开 \(project.displayName)"
        if codexBridgeConnected {
            startCodexThreadForSelectedProject()
        }
    }

    func removeProject(_ project: CodexProject) {
        projects.removeAll { $0.id == project.id }
        if selectedProjectID == project.id {
            selectedProjectID = projectsForSelectedService.first?.id ?? ""
            codexConnected = false
            codexThreadID = ""
            codexMessages = []
        }
        activityText = "已移除项目 \(project.displayName)"
    }

    func startProjectBrowser() async {
        guard let service = selectedService else {
            activityText = "请先选择一台工作电脑"
            return
        }
        let startPath = selectedProject?.path.isEmpty == false
            ? selectedProject!.path
            : defaultBrowseDirectory(for: service)
        await browseRemoteDirectory(startPath)
    }

    func browseRemoteDirectory(_ path: String) async {
        let trimmed = path.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        guard let service = selectedService else {
            activityText = "请先选择一台工作电脑"
            return
        }

        isBrowsingProjects = true
        remoteDirectoryNodeID = service.nodeID
        remoteDirectoryPath = trimmed
        remoteDirectoryError = ""
        defer { isBrowsingProjects = false }

        do {
            let entries = try await listRemoteDirectory(path: trimmed, nodeID: service.nodeID)
            remoteDirectoryEntries = entries
                .filter(\.isDirectory)
                .sorted { $0.name.localizedCompare($1.name) == .orderedAscending }
            activityText = "已打开 \(CodexProject.name(from: trimmed))"
        } catch {
            remoteDirectoryEntries = []
            remoteDirectoryError = readableError(error)
            activityText = remoteDirectoryError
        }
    }

    func browseRemoteDirectoryParent() async {
        guard let parent = remoteDirectoryParentPath else { return }
        await browseRemoteDirectory(parent)
    }

    func addPendingImageData(_ data: Data, name: String) {
        guard pendingImages.count < Self.maxPendingImages else {
            activityText = "一次最多发送 \(Self.maxPendingImages) 张图片"
            return
        }
        guard let compressed = Self.compressImageData(data) else {
            activityText = "这张图片暂时无法发送"
            return
        }
        pendingImages.append(
            CodexImageAttachment(
                name: name,
                mimeType: "image/jpeg",
                data: compressed
            )
        )
        activityText = "已添加 \(pendingImages.count) 张图片"
    }

    func addPendingImageDataList(_ items: [Data]) {
        let remaining = max(0, Self.maxPendingImages - pendingImages.count)
        guard remaining > 0 else {
            activityText = "一次最多发送 \(Self.maxPendingImages) 张图片"
            return
        }
        for (index, data) in items.prefix(remaining).enumerated() {
            addPendingImageData(data, name: "image-\(index + 1).jpg")
        }
        if items.count > remaining {
            activityText = "已添加前 \(remaining) 张图片，更多图片请分开发送"
        }
    }

    func removePendingImage(_ attachment: CodexImageAttachment) {
        pendingImages.removeAll { $0.id == attachment.id }
    }

    func loadHealthOnly() async {
        await run(successMessage: "控制中心连接正常") {
            try await loadHealth()
        }
    }

    func connectControlCenter() async {
        await run(successMessage: "控制中心已连接") {
            try await loadHealth()
            if isAuthenticated {
                try await loadNodes()
                try await loadServices()
            }
        }
    }

    func testCodexHealth() async {
        requestMethod = "GET"
        requestPath = "/healthz"
        await sendBridgeRequest(successMessage: "Codex 已准备好")
    }

    func connectCodexChat() async {
        if isLoading {
            activityText = "正在处理，请稍等"
            return
        }

        let preferredServiceID = selectedServiceID
        disconnectCodexChat(message: nil)
        selectedServiceID = preferredServiceID
        pendingImages = []
        currentAssistantMessageID = nil
        activityText = "正在连接电脑..."

        isLoading = true
        defer { isLoading = false }

        activityText = "正在确认电脑状态..."
        do {
            try await loadHealth()
            try await loadServices()
        } catch {
            let message = readableError(error)
            activityText = message
            return
        }

        if !isAuthenticated {
            activityText = "正在准备移动端访问..."
            do {
                email = demoEmail
                try await loginWith(email: demoEmail, password: demoPassword)
                try await loadServices()
            } catch {
                let message = readableError(error)
                activityText = message
                return
            }
        }

        if let preferred = services.first(where: { $0.id == preferredServiceID && $0.isAvailable }) {
            selectedServiceID = preferred.id
        } else if selectedService?.isAvailable != true, let preferred = preferredCodexService() {
            selectedServiceID = preferred.id
        }

        if selectedService?.serviceReady == true && selectedService?.bridgeWorkerConnected == false {
            activityText = "正在确认 \(selectedService?.nodeName ?? "这台电脑") 的状态..."
            do {
                try await loadServices()
            } catch {
                let message = readableError(error)
                activityText = message
                return
            }
        }

        guard let service = selectedService else {
            activityText = "没有找到可聊天的电脑"
            return
        }
        guard service.isAvailable else {
            activityText = service.serviceReady
                ? "\(service.nodeName) 在线，Codex 正在准备，请稍后重试"
                : "\(service.nodeName) 现在还不能聊天"
            return
        }

        activityText = "正在连接 \(service.nodeName)..."

        do {
            let bridgeSession = try await createBridgeSession(service: service)
            let sessionID = bridgeSession.id
            let bridgeToken = bridgeSession.token
            lastSessionID = sessionID

            let url = try bridgeURL(sessionID: sessionID, token: bridgeToken)
            let task = URLSession.shared.webSocketTask(with: url)
            codexBridgeTask = task
            task.resume()
            codexReceiveTask = Task { await receiveCodexBridgeMessages(task: task) }
            try await sendBridgeEnvelope([
                "type": "bridge.websocket.open",
                "path": "/",
            ])
        } catch {
            let message = readableError(error)
            activityText = message
            appendSystemMessage(message)
        }
    }

    func disconnectCodexChat(message: String? = "Codex 连接已断开") {
        let closingTask = codexBridgeTask
        if let closingTask {
            Task {
                try? await sendBridgeEnvelope(["type": "bridge.websocket.close"], to: closingTask)
            }
        }
        codexReceiveTask?.cancel()
        codexBridgeTask?.cancel(with: .normalClosure, reason: nil)
        codexReceiveTask = nil
        codexBridgeTask = nil
        codexBridgeConnected = false
        codexConnected = false
        codexThreadID = ""
        codexSessionMode = ""
        codexPendingMethods.removeAll()
        currentAssistantMessageID = nil
        isLoading = false
        if let message {
            activityText = message
        }
    }

    func sendCodexChatMessage() async {
        let text = codexChatInput.trimmingCharacters(in: .whitespacesAndNewlines)
        let images = pendingImages
        guard !text.isEmpty || !images.isEmpty else { return }
        guard codexConnected, !codexThreadID.isEmpty else {
            activityText = "请先连接 Codex"
            return
        }

        do {
            let params = codexTurnParams(text: text, images: images)
            try validateCodexBridgePayload(method: "turn/start", params: params)

            codexChatInput = ""
            pendingImages = []
            codexMessages.append(CodexChatMessage(role: "user", text: text, attachments: images))
            let assistant = CodexChatMessage(role: "assistant", text: "", isStreaming: true)
            currentAssistantMessageID = assistant.id
            codexMessages.append(assistant)
            activityText = "Codex 正在回复..."

            _ = try await sendCodexRPC(
                method: "turn/start",
                params: params
            )
        } catch {
            let message = readableError(error)
            activityText = message
            if currentAssistantMessageID != nil {
                if codexChatInput.isEmpty {
                    codexChatInput = text
                }
                if pendingImages.isEmpty {
                    pendingImages = images
                }
                finishCurrentAssistantMessage(fallback: message)
            }
        }
    }

    private func listRemoteDirectory(path: String, nodeID: String) async throws -> [RemoteDirectoryEntry] {
        let payload: [String: Any] = [
            "type": "file",
            "operation": "list",
            "path": path,
            "recursive": false,
            "max_entries": 300,
        ]
        let payloadData = try JSONSerialization.data(withJSONObject: payload, options: [.prettyPrinted, .sortedKeys])
        let payloadText = String(data: payloadData, encoding: .utf8) ?? "{}"
        let result = try await request(
            path: "/api/tasks",
            method: "POST",
            body: [
                "title": "选择项目目录 \(CodexProject.name(from: path))",
                "summary": "移动端请求目标节点返回目录列表，用于选择 Codex 工作目录。",
                "created_by": "mobile-client",
                "owner": "worker-agent",
                "assigned_to": ["worker-agent"],
                "priority": "normal",
                "labels": ["compute", "file", "node:\(nodeID)"],
                "inputs": [payloadText],
                "outputs": ["目录项"],
                "acceptance_criteria": [
                    "Worker 在目标节点读取目录",
                    "Hub 返回结构化 entries",
                    "移动端只展示文件夹用于选择项目",
                ],
            ]
        )
        guard let taskID = result
            .value(at: ["item", "metadata", "id"]) as? String
        else {
            throw AgentGridError.invalidResponse
        }
        let snapshot = try await waitForTaskSnapshot(taskID: taskID, timeoutSeconds: 45)
        let state = snapshot["state"] as? String ?? ""
        guard state == "done" else {
            if let message = snapshot.value(at: ["error", "message"]) as? String {
                throw AgentGridError.taskFailed(message)
            }
            if let message = snapshot.value(at: ["error", "result", "message"]) as? String {
                throw AgentGridError.taskFailed(message)
            }
            throw AgentGridError.taskFailed("这台电脑暂时无法打开该文件夹")
        }
        let rawEntries = snapshot.value(at: ["result", "entries"]) as? [[String: Any]]
            ?? snapshot.value(at: ["result", "output", "entries"]) as? [[String: Any]]
            ?? []
        return rawEntries.compactMap(Self.parseRemoteDirectoryEntry)
    }

    private func waitForTaskSnapshot(taskID: String, timeoutSeconds: TimeInterval) async throws -> [String: Any] {
        let deadline = Date().addingTimeInterval(timeoutSeconds)
        while true {
            let snapshot = try await request(path: "/api/tasks/\(taskID)/snapshot", method: "GET")
            let state = snapshot["state"] as? String ?? "unknown"
            if ["done", "failed", "cancelled", "stopped", "blocked", "review"].contains(state) {
                return snapshot
            }
            if Date() >= deadline {
                throw AgentGridError.taskFailed("读取目录超时，请稍后重试")
            }
            try await Task.sleep(nanoseconds: 1_200_000_000)
        }
    }

    private func loadHealth() async throws {
        let result = try await request(path: "/api/health", method: "GET")
        if (result["ok"] as? Bool) == true {
            hubState = "在线"
        } else {
            hubState = "异常"
        }
        rawResponseText = pretty(result)
    }

    private func loginWith(email: String, password: String) async throws {
        let result = try await request(
            path: "/api/auth/login",
            method: "POST",
            body: [
                "email": email,
                "password": password,
            ]
        )
        token = result["token"] as? String ?? ""
        rawResponseText = pretty(result)
    }

    private func loadNodes() async throws {
        let result = try await request(path: "/api/nodes", method: "GET")
        let items = result["items"] as? [[String: Any]] ?? []
        nodes = items.compactMap(Self.parseNode).sorted {
            if $0.isOnline != $1.isOnline { return $0.isOnline && !$1.isOnline }
            return $0.name.localizedCompare($1.name) == .orderedAscending
        }
        rawResponseText = pretty(result)
    }

    private func loadServices() async throws {
        let result = try await request(path: "/api/local-services", method: "GET")
        let items = result["items"] as? [[String: Any]] ?? []
        services = items.compactMap(Self.parseService).sorted {
            if $0.isAvailable != $1.isAvailable { return $0.isAvailable && !$1.isAvailable }
            if $0.nodeID != $1.nodeID {
                if $0.nodeID == "local-mac" { return true }
                if $1.nodeID == "local-mac" { return false }
            }
            return $0.nodeName.localizedCompare($1.nodeName) == .orderedAscending
        }
        if selectedServiceID.isEmpty || !services.contains(where: { $0.id == selectedServiceID }) {
            selectedServiceID = preferredCodexService()?.id ?? services.first?.id ?? ""
            userSelectedServiceID = false
        } else if !userSelectedServiceID,
                  let preferred = preferredCodexService(),
                  selectedService?.isAvailable != true {
            selectedServiceID = preferred.id
        }
        if let service = selectedService {
            ensureProjectSelection(for: service)
        }
        rawResponseText = pretty(result)
    }

    private func loadTasks() async throws {
        let result = try await request(path: "/api/tasks?limit=20", method: "GET")
        let items = result["items"] as? [[String: Any]] ?? []
        tasks = items.compactMap(Self.parseTask)
        rawResponseText = pretty(result)
    }

    private func preferredCodexService() -> LocalService? {
        services.first { $0.nodeID == "local-mac" && $0.isAvailable }
            ?? services.first(where: \.isAvailable)
    }

    private func ensureProjectSelection(for service: LocalService) {
        if let selectedProject,
           selectedProject.nodeID == service.nodeID {
            return
        }
        selectedProjectID = projectsForSelectedService.first?.id ?? ""
    }

    private func defaultBrowseDirectory(for service: LocalService) -> String {
        if service.nodeID == "local-mac" {
            return "/Users/a1/Desktop"
        }
        let node = nodes.first { $0.id == service.nodeID }
        let os = (node?.shortOS ?? service.nodeName).lowercased()
        if os.contains("windows") || service.nodeID.lowercased().contains("windows") {
            return "C:\\Users"
        }
        if os.contains("mac") || os.contains("darwin") {
            return "/Users"
        }
        return "/home"
    }

    private func resetRemoteDirectoryBrowser() {
        remoteDirectoryPath = ""
        remoteDirectoryNodeID = ""
        remoteDirectoryEntries = []
        remoteDirectoryError = ""
        isBrowsingProjects = false
    }

    private func upsertProject(_ project: CodexProject) {
        var next = projects.filter { $0.id != project.id }
        next.insert(project, at: 0)
        projects = Array(next.prefix(30))
    }

    private func touchProject(_ project: CodexProject) {
        var next = project
        next.lastOpenedAt = Self.isoNow()
        upsertProject(next)
    }

    private func persistThreadID(_ threadID: String, forProjectID projectID: String) {
        guard let index = projects.firstIndex(where: { $0.id == projectID }) else { return }
        projects[index].codexThreadID = threadID
        projects[index].codexThreadProfile = Self.codexThreadProfile
        projects[index].lastOpenedAt = Self.isoNow()
    }

    private func clearSavedThreadID(for project: CodexProject) {
        guard let index = projects.firstIndex(where: { $0.id == project.id }) else { return }
        projects[index].codexThreadID = nil
        projects[index].codexThreadProfile = nil
    }

    private func saveProjects() {
        guard let data = try? JSONEncoder().encode(projects) else { return }
        UserDefaults.standard.set(data, forKey: Self.projectsStorageKey)
    }

    private static func loadStoredProjects(storageKey: String) -> [CodexProject] {
        guard let data = UserDefaults.standard.data(forKey: storageKey),
              let value = try? JSONDecoder().decode([CodexProject].self, from: data)
        else {
            return []
        }
        return value.map { project in
            var next = project
            if next.codexThreadProfile != Self.codexThreadProfile {
                next.codexThreadID = nil
                next.codexThreadProfile = nil
            }
            return next
        }
    }

    private static func isoNow() -> String {
        ISO8601DateFormatter().string(from: Date())
    }

    private static func parentDirectory(of path: String) -> String? {
        let trimmed = path.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        let isWindows = trimmed.contains("\\") || trimmed.contains(":")
        if isWindows {
            var value = trimmed.replacingOccurrences(of: "/", with: "\\")
            while value.hasSuffix("\\"), value.count > 3 {
                value.removeLast()
            }
            if value.count <= 3, value.contains(":") {
                return nil
            }
            guard let index = value.lastIndex(of: "\\") else { return nil }
            var parent = String(value[..<index])
            if parent.count == 2, parent.last == ":" {
                parent += "\\"
            }
            return parent.isEmpty ? nil : parent
        }

        var value = trimmed
        while value.hasSuffix("/"), value.count > 1 {
            value.removeLast()
        }
        guard value != "/" else { return nil }
        let parent = (value as NSString).deletingLastPathComponent
        if parent.isEmpty || parent == "." || parent == value {
            return nil
        }
        return parent
    }

    private static func parseRemoteDirectoryEntry(_ item: [String: Any]) -> RemoteDirectoryEntry? {
        guard let path = item["path"] as? String, !path.isEmpty else { return nil }
        let isDirectory = item["is_dir"] as? Bool
            ?? item["isDirectory"] as? Bool
            ?? item["is_directory"] as? Bool
            ?? false
        let size: Int64
        if let value = item["len"] as? Int64 {
            size = value
        } else if let value = item["len"] as? Int {
            size = Int64(value)
        } else if let value = item["len"] as? Double {
            size = Int64(value)
        } else if let value = item["size"] as? Int {
            size = Int64(value)
        } else {
            size = 0
        }
        return RemoteDirectoryEntry(path: path, isDirectory: isDirectory, size: size)
    }

    private static func compressImageData(_ data: Data) -> Data? {
        #if canImport(UIKit)
        guard let image = UIImage(data: data) else { return nil }
        let dimensions: [CGFloat] = [900, 720, 560, 420]
        let qualities: [CGFloat] = [0.70, 0.58, 0.48, 0.38, 0.30, 0.24]

        for maxDimension in dimensions {
            let longest = max(image.size.width, image.size.height)
            let outputImage: UIImage
            if longest > maxDimension {
                let scale = maxDimension / longest
                let size = CGSize(width: image.size.width * scale, height: image.size.height * scale)
                let format = UIGraphicsImageRendererFormat.default()
                format.scale = 1
                let renderer = UIGraphicsImageRenderer(size: size, format: format)
                outputImage = renderer.image { _ in
                    image.draw(in: CGRect(origin: .zero, size: size))
                }
            } else {
                outputImage = image
            }

            for quality in qualities {
                if let encoded = outputImage.jpegData(compressionQuality: quality),
                   encoded.count <= Self.maxImageBytes {
                    return encoded
                }
            }
        }

        return nil
        #else
        return nil
        #endif
    }

    private func codexTurnParams(text: String, images: [CodexImageAttachment]) -> [String: Any] {
        var inputItems: [[String: Any]] = []
        if !text.isEmpty {
            inputItems.append([
                "type": "text",
                "text": text,
                "text_elements": [],
            ])
        }
        for image in images {
            inputItems.append([
                "type": "image",
                "url": image.dataURL,
                "mimeType": image.mimeType,
                "name": image.name,
            ])
        }
        return [
            "threadId": codexThreadID,
            "input": inputItems,
            "fileAttachments": [],
            "approvalPolicy": "never",
            "effort": Self.defaultReasoningEffort,
            "sandboxPolicy": [
                "type": "dangerFullAccess",
            ],
        ]
    }

    private func validateCodexBridgePayload(method: String, params: [String: Any]) throws {
        let envelope: [String: Any] = [
            "type": "bridge.websocket.message",
            "body": [
                "id": codexRequestID,
                "method": method,
                "params": params,
            ],
        ]
        let data = try JSONSerialization.data(withJSONObject: envelope)
        guard data.count <= Self.maxBridgeEnvelopeBytes else {
            throw AgentGridError.payloadTooLarge
        }
    }

    private func sendBridgeRequest(successMessage: String) async {
        guard let service = selectedService else {
            activityText = "请先选择一台工作电脑"
            return
        }
        guard service.isAvailable else {
            activityText = service.serviceReady
                ? "\(service.nodeName) 在线，Codex 正在准备，请稍后重试"
                : "\(service.nodeName) 现在还不能聊天"
            return
        }
        guard isAuthenticated else {
            activityText = "请先登录 Hub"
            return
        }

        await run(successMessage: successMessage) {
            let bridgeSession = try await createBridgeSession(service: service)
            let sessionID = bridgeSession.id
            let bridgeToken = bridgeSession.token
            lastSessionID = sessionID

            let url = try bridgeURL(sessionID: sessionID, token: bridgeToken)
            let response = try await sendBridgeMessage(url: url)
            rawResponseText = pretty(response)
        }
    }

    private func run(successMessage: String, _ operation: () async throws -> Void) async {
        isLoading = true
        activityText = "正在处理..."
        defer { isLoading = false }
        do {
            try await operation()
            activityText = successMessage
        } catch {
            let message = readableError(error)
            activityText = message
            rawResponseText = message
        }
    }

    private func baseURL() throws -> URL {
        let trimmed = hubURL.trimmingCharacters(in: CharacterSet(charactersIn: "/ "))
        guard let url = URL(string: trimmed) else {
            throw AgentGridError.invalidURL(hubURL)
        }
        return url
    }

    private func request(
        path: String,
        method: String,
        body: [String: Any]? = nil
    ) async throws -> [String: Any] {
        let url = try apiURL(path)
        var request = URLRequest(url: url)
        request.httpMethod = method
        request.setValue("application/json", forHTTPHeaderField: "accept")
        if isAuthenticated {
            request.setValue("Bearer \(token)", forHTTPHeaderField: "authorization")
        }
        if let body {
            request.setValue("application/json", forHTTPHeaderField: "content-type")
            request.httpBody = try JSONSerialization.data(withJSONObject: body)
        }

        let (data, response) = try await URLSession.shared.data(for: request)
        guard let http = response as? HTTPURLResponse else {
            throw AgentGridError.invalidResponse
        }
        guard (200..<300).contains(http.statusCode) else {
            throw AgentGridError.http(http.statusCode, String(data: data, encoding: .utf8) ?? "", url.absoluteString)
        }
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw AgentGridError.invalidResponse
        }
        return json
    }

    private func apiURL(_ path: String) throws -> URL {
        let root = try baseURL()
        var components = URLComponents(url: root, resolvingAgainstBaseURL: false)
        let rootPath = root.path.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        var childPath = path.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        if let questionMarkIndex = childPath.firstIndex(of: "?") {
            components?.query = String(childPath[childPath.index(after: questionMarkIndex)...])
            childPath = String(childPath[..<questionMarkIndex])
        } else {
            components?.query = nil
        }
        let joined = [rootPath, childPath].filter { !$0.isEmpty }.joined(separator: "/")
        components?.path = "/" + joined
        components?.fragment = nil
        guard let url = components?.url else {
            throw AgentGridError.invalidURL(path)
        }
        return url
    }

    private func createBridgeSession(service: LocalService) async throws -> BridgeSessionInfo {
        let sessionResult = try await request(
            path: "/api/bridge-sessions",
            method: "POST",
            body: [
                "node_id": service.nodeID,
                "service_id": service.serviceID,
            ]
        )
        let item = sessionResult["item"] as? [String: Any] ?? [:]
        let metadata = item["metadata"] as? [String: Any] ?? [:]
        let spec = item["spec"] as? [String: Any] ?? [:]
        let status = item["status"] as? [String: Any] ?? [:]
        let sessionID = metadata["id"] as? String ?? ""
        let bridgeToken = spec["token"] as? String ?? ""
        let workerConnected = status["worker_connected"] as? Bool ?? false
        guard !sessionID.isEmpty, !bridgeToken.isEmpty else {
            throw AgentGridError.invalidResponse
        }
        guard workerConnected else {
            throw AgentGridError.bridgeWorkerDisconnected(service.nodeName)
        }
        return BridgeSessionInfo(id: sessionID, token: bridgeToken, workerConnected: workerConnected)
    }

    private func bridgeURL(sessionID: String, token: String) throws -> URL {
        let root = try baseURL()
        guard var components = URLComponents(
            url: root.appendingPathComponent("api/bridge-sessions/\(sessionID)/ws"),
            resolvingAgainstBaseURL: false
        ) else {
            throw AgentGridError.invalidURL(sessionID)
        }
        components.scheme = root.scheme == "https" ? "wss" : "ws"
        components.queryItems = [URLQueryItem(name: "token", value: token)]
        guard let url = components.url else {
            throw AgentGridError.invalidURL(sessionID)
        }
        return url
    }

    private func sendBridgeMessage(url: URL) async throws -> [String: Any] {
        let task = URLSession.shared.webSocketTask(with: url)
        task.resume()
        defer { task.cancel(with: .normalClosure, reason: nil) }

        var headers: [String: String] = [:]
        var body: Any = NSNull()
        if requestMethod != "GET" {
            headers["content-type"] = "application/json"
            body = parseRequestBody()
        }

        let message: [String: Any] = [
            "type": "bridge.request",
            "method": requestMethod,
            "path": requestPath.isEmpty ? "/" : requestPath,
            "headers": headers,
            "body": body,
        ]
        let data = try JSONSerialization.data(withJSONObject: message)
        guard let text = String(data: data, encoding: .utf8) else {
            throw AgentGridError.invalidResponse
        }
        try await task.send(.string(text))

        while true {
            let incoming = try await task.receive()
            let data: Data
            switch incoming {
            case .data(let payload):
                data = payload
            case .string(let text):
                data = Data(text.utf8)
            @unknown default:
                continue
            }
            guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
                continue
            }
            if (json["type"] as? String) == "bridge.ready" {
                continue
            }
            return json
        }
    }

    private func receiveCodexBridgeMessages(task: URLSessionWebSocketTask) async {
        while !Task.isCancelled {
            do {
                let incoming = try await task.receive()
                let data: Data
                switch incoming {
                case .data(let payload):
                    data = payload
                case .string(let text):
                    data = Data(text.utf8)
                @unknown default:
                    continue
                }
                guard let envelope = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
                    continue
                }
                guard isCurrentCodexBridgeTask(task) else {
                    return
                }
                await handleCodexBridgeEnvelope(envelope, from: task)
            } catch {
                guard isCurrentCodexBridgeTask(task) else {
                    return
                }
                if !Task.isCancelled {
                    disconnectCodexChat(message: codexDisconnectMessage(error))
                }
                return
            }
        }
    }

    private func handleCodexBridgeEnvelope(_ envelope: [String: Any], from task: URLSessionWebSocketTask) async {
        guard isCurrentCodexBridgeTask(task) else { return }
        let type = envelope["type"] as? String ?? ""
        switch type {
        case "bridge.ready":
            return
        case "bridge.websocket.ready":
            do {
                _ = try await sendCodexRPC(
                    method: "initialize",
                    params: [
                        "clientInfo": [
                            "name": "agentgrid",
                            "title": "AgentGrid",
                            "version": "0.1.0",
                        ],
                        "capabilities": [
                            "experimentalApi": true,
                            "requestAttestation": false,
                            "optOutNotificationMethods": [],
                        ],
                    ]
                )
            } catch {
                disconnectCodexChat(message: readableError(error))
            }
        case "bridge.websocket.message":
            guard let body = envelope["body"] as? String,
                  let data = body.data(using: .utf8),
                  let message = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
            else {
                return
            }
            await handleCodexJSONRPC(message)
        case "bridge.websocket.closed":
            disconnectCodexChat(message: "Codex 连接已关闭")
        case "bridge.error":
            disconnectCodexChat(message: readableCodexMessage(envelope["message"] as? String ?? "Codex 连接失败"))
        default:
            return
        }
    }

    private func isCurrentCodexBridgeTask(_ task: URLSessionWebSocketTask) -> Bool {
        guard let activeTask = codexBridgeTask else { return false }
        return activeTask === task
    }

    private func codexDisconnectMessage(_ error: Error) -> String {
        let message = readableError(error)
        if message == AgentGridError.payloadTooLarge.errorDescription {
            return message
        }
        return "Codex 连接中断：\(message)"
    }

    private func startCodexThreadForSelectedProject() {
        guard codexBridgeConnected, let project = selectedProject else { return }
        codexConnected = false
        codexThreadID = ""
        codexSessionMode = ""
        currentAssistantMessageID = nil
        codexPendingMethods = codexPendingMethods.filter {
            $0.value != "thread/resume" && $0.value != "thread/start"
        }
        activityText = "正在打开 \(project.displayName)..."
        Task {
            do {
                if project.codexThreadProfile == Self.codexThreadProfile,
                   let threadID = project.codexThreadID?.trimmingCharacters(in: .whitespacesAndNewlines),
                   !threadID.isEmpty {
                    _ = try await sendCodexRPC(
                        method: "thread/resume",
                        params: codexResumeParams(threadID: threadID)
                    )
                    return
                }
                if project.codexThreadID?.isEmpty == false {
                    clearSavedThreadID(for: project)
                }
                _ = try await sendCodexRPC(
                    method: "thread/start",
                    params: codexStartParams()
                )
            } catch {
                activityText = readableError(error)
            }
        }
    }

    private func handleCodexJSONRPC(_ message: [String: Any]) async {
        if let error = message["error"] as? [String: Any] {
            let text = readableCodexMessage(error["message"] as? String ?? "Codex 请求失败")
            activityText = text
            let failedMethod = (message["id"] as? Int).flatMap { codexPendingMethods.removeValue(forKey: $0) }
            if failedMethod == "thread/resume",
               let project = selectedProject,
               let savedThreadID = project.codexThreadID,
               !savedThreadID.isEmpty {
                clearSavedThreadID(for: project)
                Task {
                    do {
                        _ = try await sendCodexRPC(
                            method: "thread/start",
                            params: codexStartParams()
                        )
                    } catch {
                        activityText = readableError(error)
                    }
                }
                return
            }
            if currentAssistantMessageID != nil {
                finishCurrentAssistantMessage(fallback: text)
            }
            return
        }

        if let id = message["id"] as? Int {
            let method = codexPendingMethods.removeValue(forKey: id)
            if method == "initialize" {
                codexBridgeConnected = true
                isLoading = false
                activityText = "已连接 \(selectedService?.nodeName ?? "工作电脑")"
                if selectedProject != nil {
                    startCodexThreadForSelectedProject()
                }
                return
            }
            if method == "thread/resume" || method == "thread/start" {
                openCodexThread(from: message["result"] as? [String: Any] ?? [:], resumed: method == "thread/resume")
                return
            }
        }

        guard let method = message["method"] as? String else { return }
        let params = message["params"] as? [String: Any] ?? [:]
        switch method {
        case "item/agentMessage/delta":
            appendAssistantDelta(params["delta"] as? String ?? "")
        case "item/completed":
            if let item = params["item"] as? [String: Any],
               item["type"] as? String == "agentMessage",
               let text = item["text"] as? String {
                finishCurrentAssistantMessage(fallback: text)
            }
        case "turn/completed":
            activityText = "Codex 回复完成"
            finishCurrentAssistantMessage(fallback: nil)
        case "thread/status/changed":
            return
        default:
            return
        }
    }

    private func codexStartParams() -> [String: Any] {
        [
            "cwd": selectedWorkingDirectory,
            "approvalPolicy": "never",
            "sandbox": "danger-full-access",
            "reasoningEffort": Self.defaultReasoningEffort,
            "ephemeral": false,
            "threadSource": "user",
            "baseInstructions": "你正在通过 AgentGrid 手机端操作用户选中的电脑项目。当前项目路径是 \(selectedWorkingDirectory)。如果用户要求查看、修改、运行或排查，优先主动使用可用工具完成；不要让用户粘贴项目文件。用中文简洁说明结果。",
        ]
    }

    private func codexResumeParams(threadID: String) -> [String: Any] {
        [
            "threadId": threadID,
            "cwd": selectedWorkingDirectory,
            "approvalPolicy": "never",
            "sandbox": "danger-full-access",
            "reasoningEffort": Self.defaultReasoningEffort,
        ]
    }

    private func openCodexThread(from result: [String: Any], resumed: Bool) {
        let thread = result["thread"] as? [String: Any] ?? [:]
        codexThreadID = thread["id"] as? String ?? ""
        codexConnected = !codexThreadID.isEmpty
        codexSessionMode = resumed ? "已接入历史会话" : "新会话"
        isLoading = false
        activityText = codexConnected ? "\(codexSessionMode) · \(selectedProject?.displayName ?? "项目")" : "聊天创建失败"
        if codexConnected {
            persistThreadID(codexThreadID, forProjectID: selectedProjectID)
        }

        let restoredMessages = Self.messages(fromCodexThread: thread)
        if !restoredMessages.isEmpty {
            codexMessages = restoredMessages
        }
    }

    private static func messages(fromCodexThread thread: [String: Any]) -> [CodexChatMessage] {
        let turns = thread["turns"] as? [[String: Any]] ?? []
        var messages: [CodexChatMessage] = []
        for turn in turns {
            let items = turn["items"] as? [[String: Any]] ?? []
            for item in items {
                guard let type = item["type"] as? String else { continue }
                switch type {
                case "userMessage":
                    let parsed = parseUserMessage(item)
                    if !parsed.text.isEmpty || !parsed.images.isEmpty {
                        messages.append(CodexChatMessage(role: "user", text: parsed.text, attachments: parsed.images))
                    }
                case "agentMessage":
                    let text = parseAgentMessage(item)
                    if !text.isEmpty {
                        messages.append(CodexChatMessage(role: "assistant", text: text))
                    }
                default:
                    continue
                }
            }
        }
        return messages
    }

    private static func parseUserMessage(_ item: [String: Any]) -> (text: String, images: [CodexImageAttachment]) {
        let content = item["content"] as? [[String: Any]] ?? []
        var parts: [String] = []
        var images: [CodexImageAttachment] = []
        for value in content {
            let type = value["type"] as? String ?? ""
            if type == "text" || type == "input_text" {
                if let text = value["text"] as? String, !text.isEmpty {
                    parts.append(text)
                }
            } else if type == "image" || type == "localImage" {
                if let image = imageAttachment(from: value) {
                    images.append(image)
                }
            }
        }
        return (parts.joined(separator: "\n"), images)
    }

    private static func parseAgentMessage(_ item: [String: Any]) -> String {
        if let text = item["text"] as? String {
            return text.trimmingCharacters(in: .whitespacesAndNewlines)
        }
        let content = item["content"] as? [[String: Any]] ?? []
        return content.compactMap { value in
            value["text"] as? String
        }
        .joined(separator: "\n")
        .trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private static func imageAttachment(from value: [String: Any]) -> CodexImageAttachment? {
        guard let url = value["url"] as? String,
              let commaIndex = url.firstIndex(of: ",")
        else { return nil }
        let header = String(url[..<commaIndex])
        let payload = String(url[url.index(after: commaIndex)...])
        guard let data = Data(base64Encoded: payload) else { return nil }
        let mimeType = header
            .replacingOccurrences(of: "data:", with: "")
            .replacingOccurrences(of: ";base64", with: "")
        return CodexImageAttachment(
            name: value["name"] as? String ?? "image.jpg",
            mimeType: mimeType.isEmpty ? "image/jpeg" : mimeType,
            data: data
        )
    }

    private func sendCodexRPC(method: String, params: [String: Any]) async throws -> Int {
        let id = codexRequestID
        codexRequestID += 1
        codexPendingMethods[id] = method
        try await sendBridgeEnvelope([
            "type": "bridge.websocket.message",
            "body": [
                "id": id,
                "method": method,
                "params": params,
            ],
        ])
        return id
    }

    private func sendCodexNotification(method: String, params: [String: Any]? = nil) async throws {
        var body: [String: Any] = ["method": method]
        if let params {
            body["params"] = params
        }
        try await sendBridgeEnvelope([
            "type": "bridge.websocket.message",
            "body": body,
        ])
    }

    private func sendBridgeEnvelope(_ envelope: [String: Any]) async throws {
        guard let task = codexBridgeTask else {
            throw AgentGridError.invalidResponse
        }
        try await sendBridgeEnvelope(envelope, to: task)
    }

    private func sendBridgeEnvelope(_ envelope: [String: Any], to task: URLSessionWebSocketTask) async throws {
        let data = try JSONSerialization.data(withJSONObject: envelope)
        guard let text = String(data: data, encoding: .utf8) else {
            throw AgentGridError.invalidResponse
        }
        try await task.send(.string(text))
    }

    private func appendSystemMessage(_ text: String) {
        guard !text.isEmpty else { return }
        codexMessages.append(CodexChatMessage(role: "system", text: text))
    }

    private func appendAssistantDelta(_ delta: String) {
        guard !delta.isEmpty else { return }
        if let id = currentAssistantMessageID,
           let index = codexMessages.firstIndex(where: { $0.id == id }) {
            codexMessages[index].text += delta
        } else {
            let message = CodexChatMessage(role: "assistant", text: delta, isStreaming: true)
            currentAssistantMessageID = message.id
            codexMessages.append(message)
        }
    }

    private func finishCurrentAssistantMessage(fallback: String?) {
        if let id = currentAssistantMessageID,
           let index = codexMessages.firstIndex(where: { $0.id == id }) {
            if codexMessages[index].text.isEmpty, let fallback {
                codexMessages[index].text = fallback
            }
            codexMessages[index].isStreaming = false
        } else if let fallback, !fallback.isEmpty {
            codexMessages.append(CodexChatMessage(role: "assistant", text: fallback))
        }
        currentAssistantMessageID = nil
    }

    private func parseRequestBody() -> Any {
        let data = Data(requestBody.utf8)
        if let json = try? JSONSerialization.jsonObject(with: data) {
            return json
        }
        return requestBody
    }

    private static func parseNode(_ item: [String: Any]) -> AgentNode? {
        let metadata = item["metadata"] as? [String: Any] ?? [:]
        let spec = item["spec"] as? [String: Any] ?? [:]
        let status = item["status"] as? [String: Any] ?? [:]
        let id = metadata["id"] as? String ?? ""
        guard !id.isEmpty else { return nil }
        return AgentNode(
            id: id,
            name: metadata["name"] as? String ?? id,
            os: spec["os"] as? String ?? "",
            address: spec["address"] as? String ?? "",
            state: status["state"] as? String ?? "unknown",
            cpuCores: spec["cpu_cores"] as? Int ?? 0,
            cpuUsagePercent: spec["cpu_usage_percent"] as? Double ?? 0,
            memoryMB: spec["memory_mb"] as? Int ?? 0,
            memoryUsedMB: spec["memory_used_mb"] as? Int ?? 0,
            diskTotalMB: spec["disk_total_mb"] as? Int ?? 0,
            diskFreeMB: spec["disk_free_mb"] as? Int ?? 0,
            maxConcurrentJobs: spec["max_concurrent_jobs"] as? Int ?? 0,
            runningJobs: status["running_jobs"] as? Int ?? 0,
            capabilities: spec["capabilities"] as? [String] ?? [],
            tags: spec["tags"] as? [String] ?? []
        )
    }

    private static func parseService(_ item: [String: Any]) -> LocalService? {
        let metadata = item["metadata"] as? [String: Any] ?? [:]
        let spec = item["spec"] as? [String: Any] ?? [:]
        let status = item["status"] as? [String: Any] ?? [:]
        let nodeID = metadata["node_id"] as? String ?? ""
        let serviceID = spec["id"] as? String ?? ""
        guard !nodeID.isEmpty, !serviceID.isEmpty else { return nil }
        return LocalService(
            id: "\(nodeID):\(serviceID)",
            nodeID: nodeID,
            nodeName: metadata["node_name"] as? String ?? nodeID,
            serviceID: serviceID,
            name: spec["name"] as? String ?? serviceID,
            status: spec["status"] as? String ?? "unknown",
            nodeState: status["node_state"] as? String ?? "unknown",
            bridgeWorkerConnected: status["bridge_worker_connected"] as? Bool ?? true
        )
    }

    private static func parseTask(_ item: [String: Any]) -> AgentTaskItem? {
        let metadata = item["metadata"] as? [String: Any] ?? [:]
        let spec = item["spec"] as? [String: Any] ?? [:]
        let status = item["status"] as? [String: Any] ?? [:]
        let id = metadata["id"] as? String ?? ""
        guard !id.isEmpty else { return nil }
        let error = status["error"] as? [String: Any]
        return AgentTaskItem(
            id: id,
            title: spec["title"] as? String ?? id,
            state: status["state"] as? String ?? "unknown",
            priority: spec["priority"] as? String ?? "normal",
            owner: spec["owner"] as? String ?? "",
            nodeID: status["leased_by_node_id"] as? String ?? "",
            updatedAt: metadata["updated_at"] as? String ?? "",
            labels: spec["labels"] as? [String] ?? [],
            errorMessage: error?["message"] as? String
        )
    }

    private func pretty(_ value: Any) -> String {
        guard JSONSerialization.isValidJSONObject(value),
              let data = try? JSONSerialization.data(withJSONObject: value, options: [.prettyPrinted, .sortedKeys])
        else {
            return String(describing: value)
        }
        return String(data: data, encoding: .utf8) ?? String(describing: value)
    }

    private func readableError(_ error: Error) -> String {
        if let agentGridError = error as? AgentGridError {
            return agentGridError.errorDescription ?? "请求失败"
        }
        return readableCodexMessage(error.localizedDescription)
    }

    private func readableCodexMessage(_ message: String) -> String {
        if message.localizedCaseInsensitiveContains("message too long") {
            return "图片太大了，请删除一张或分开发送"
        }
        return message
    }
}

enum AgentGridError: LocalizedError {
    case invalidURL(String)
    case invalidResponse
    case http(Int, String, String)
    case bridgeWorkerDisconnected(String)
    case taskFailed(String)
    case payloadTooLarge

    var errorDescription: String? {
        switch self {
        case .invalidURL(let value):
            return "无效地址：\(value)"
        case .invalidResponse:
            return "控制中心返回内容无法解析"
        case .bridgeWorkerDisconnected(let nodeName):
            return "\(nodeName) 在线，但还没有准备好聊天。请确认这台电脑上的 AgentGrid 和 Codex 都在运行。"
        case .taskFailed(let message):
            return message
        case .payloadTooLarge:
            return "图片太大了，请删除一张或分开发送"
        case .http(let status, let body, let url):
            let title = shortHTTPMessage(status)
            let bodySummary = summarizeHTTPBody(body)
            if status == 502 || bodyLooksLikeHTML(body) {
                return title
            }
            if bodySummary.isEmpty {
                return "\(title)\n\(url)"
            }
            return "\(title)\n\(url)\n\(bodySummary)"
        }
    }

    private func shortHTTPMessage(_ status: Int) -> String {
        switch status {
        case 401:
            return "登录已失效或账号密码不正确"
        case 403:
            return "没有权限访问这个接口"
        case 404:
            return "地址不可用，请检查控制中心地址"
        case 502:
            return "中心入口暂时打不开，请稍后再试"
        case 503:
            return "中心暂时不可用"
        default:
            return "请求失败（HTTP \(status)）"
        }
    }

    private func summarizeHTTPBody(_ body: String) -> String {
        var text = body
            .replacingOccurrences(of: "<[^>]+>", with: " ", options: .regularExpression)
            .replacingOccurrences(of: "\\s+", with: " ", options: .regularExpression)
            .trimmingCharacters(in: .whitespacesAndNewlines)
        if text.lowercased().contains("bad gateway") {
            text = "服务器网关错误"
        }
        if text.count > 180 {
            return String(text.prefix(180)) + "..."
        }
        return text
    }

    private func bodyLooksLikeHTML(_ body: String) -> Bool {
        let lower = body.lowercased()
        return lower.contains("<html") || lower.contains("<!doctype") || lower.contains("<body")
    }
}

extension Dictionary where Key == String, Value == Any {
    func value(at path: [String]) -> Any? {
        var current: Any = self
        for key in path {
            guard let object = current as? [String: Any],
                  let next = object[key]
            else {
                return nil
            }
            current = next
        }
        return current
    }
}

struct ContentView: View {
    @StateObject private var model = AgentGridMobileModel()
    @State private var selectedTab: AppTab = .dashboard

    var body: some View {
        TabView(selection: $selectedTab) {
            DashboardView(model: model, selectedTab: $selectedTab)
                .tabItem { Label(AppTab.dashboard.title, systemImage: AppTab.dashboard.icon) }
                .tag(AppTab.dashboard)

            CodexBridgeView(model: model)
                .tabItem { Label(AppTab.codex.title, systemImage: AppTab.codex.icon) }
                .tag(AppTab.codex)

            NodesView(model: model)
                .tabItem { Label(AppTab.nodes.title, systemImage: AppTab.nodes.icon) }
                .tag(AppTab.nodes)

            TasksView(model: model)
                .tabItem { Label(AppTab.tasks.title, systemImage: AppTab.tasks.icon) }
                .tag(AppTab.tasks)

            SettingsView(model: model)
                .tabItem { Label(AppTab.settings.title, systemImage: AppTab.settings.icon) }
                .tag(AppTab.settings)
        }
        .task {
            if model.nodes.isEmpty {
                await model.refreshAll()
            }
        }
    }
}

struct DashboardView: View {
    @ObservedObject var model: AgentGridMobileModel
    @Binding var selectedTab: AppTab

    var body: some View {
        NavigationView {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    HeaderBlock(
                        title: "AgentGrid",
                        subtitle: "连接你的工作电脑",
                        icon: "point.3.connected.trianglepath.dotted"
                    )

                    MobileCommandCenterCard(model: model) {
                        selectedTab = .codex
                        Task { await model.connectCodexChat() }
                    }

                    SystemHealthCard(model: model)

                    ActionPanel(
                        model: model,
                        openCodex: {
                            selectedTab = .codex
                            Task { await model.connectCodexChat() }
                        },
                        openTasks: {
                            selectedTab = .tasks
                        }
                    )
                }
                .padding(18)
            }
            .background(AppTheme.background.ignoresSafeArea())
            .navigationBarHidden(true)
        }
    }
}

struct MobileCommandCenterCard: View {
    @ObservedObject var model: AgentGridMobileModel
    let connectCodex: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack(alignment: .top, spacing: 14) {
                ZStack {
                    Circle()
                        .fill(model.codexConnected ? Color.green.opacity(0.14) : AppTheme.accent.opacity(0.14))
                        .frame(width: 54, height: 54)
                    Image(systemName: model.codexConnected ? "bubble.left.and.bubble.right.fill" : "iphone.and.arrow.forward")
                        .font(.system(size: 24, weight: .semibold))
                        .foregroundColor(model.codexConnected ? .green : AppTheme.accent)
                }

                VStack(alignment: .leading, spacing: 5) {
                    Text(model.codexBridgeConnected ? "Codex 已连接" : "Codex")
                        .font(.title3.weight(.bold))
                    Text(model.codexBusinessStatusDetail)
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                        .lineLimit(1)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 0)
            }

            HStack(spacing: 8) {
                StatusPill(title: model.hubState == "在线" ? "服务在线" : "服务未确认", color: model.hubState == "在线" ? .green : .orange)
                StatusPill(title: model.availableServices.isEmpty ? "等待电脑" : "可连接", color: model.availableServices.isEmpty ? .orange : .green)
            }

            Button(action: connectCodex) {
                WideButtonLabel(title: model.primaryCodexActionTitle, icon: "bolt.horizontal.circle.fill")
            }
            .buttonStyle(FilledButtonStyle())
            .disabled(model.isLoading)
        }
        .padding(18)
        .background(
            LinearGradient(
                colors: [AppTheme.card, AppTheme.accent.opacity(0.08)],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
        )
        .overlay(
            RoundedRectangle(cornerRadius: 18)
                .stroke(AppTheme.accent.opacity(0.12), lineWidth: 1)
        )
        .cornerRadius(18)
    }
}

struct SystemHealthCard: View {
    @ObservedObject var model: AgentGridMobileModel

    var body: some View {
        SectionCard(title: "连接状态", icon: "checkmark.seal.fill") {
            VStack(alignment: .leading, spacing: 12) {
                HStack(spacing: 12) {
                    if model.isLoading {
                        ProgressView()
                    } else {
                        Image(systemName: "circle.fill")
                            .font(.caption)
                            .foregroundColor(model.systemHealthColor)
                    }
                    VStack(alignment: .leading, spacing: 3) {
                        Text(model.systemHealthTitle)
                            .font(.title3.weight(.semibold))
                        Text(model.compactActivityText)
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }
                    Spacer()
                    StatusPill(title: model.hubState == "在线" ? "服务在线" : "服务未确认", color: model.hubState == "在线" ? .green : .orange)
                }

                HStack(spacing: 8) {
                    StatusPill(title: model.availableServices.isEmpty ? "暂无可用电脑" : "电脑可用", color: model.availableServices.isEmpty ? .orange : .green)
                    StatusPill(title: model.failedTasks.isEmpty ? "任务正常" : "有待处理任务", color: model.failedTasks.isEmpty ? .green : .orange)
                }
            }
        }
    }
}

struct ActionPanel: View {
    @ObservedObject var model: AgentGridMobileModel
    let openCodex: () -> Void
    let openTasks: () -> Void

    var body: some View {
        SectionCard(title: "快捷操作", icon: "bolt.fill") {
            VStack(spacing: 10) {
                Button {
                    Task { await model.refreshAll() }
                } label: {
                    WideButtonLabel(title: "刷新状态", icon: "arrow.clockwise")
                }
                .buttonStyle(FilledButtonStyle())
                .disabled(model.isLoading)

                Button {
                    openCodex()
                } label: {
                    WideButtonLabel(title: model.primaryCodexActionTitle, icon: "bolt.horizontal.circle.fill")
                }
                .buttonStyle(SoftButtonStyle())
                .disabled(model.isLoading)

                Button {
                    openTasks()
                } label: {
                    WideButtonLabel(title: "查看任务", icon: "list.bullet.rectangle.fill")
                }
                .buttonStyle(SoftButtonStyle())
            }
        }
    }
}

struct NodesView: View {
    @ObservedObject var model: AgentGridMobileModel

    var body: some View {
        NavigationView {
            ScrollView {
                VStack(spacing: 12) {
                    HeaderBlock(title: "节点", subtitle: "\(model.onlineNodes.count) 台在线", icon: "server.rack")

                    SectionCard(title: "节点概况", icon: "chart.bar.xaxis") {
                        LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 10) {
                            MetricCard(title: "在线", value: "\(model.onlineNodes.count)", subtitle: "可接任务")
                            MetricCard(title: "离线", value: "\(max(model.nodes.count - model.onlineNodes.count, 0))", subtitle: "不可调度")
                            MetricCard(title: "桌面节点", value: "\(model.windowsDesktopNodes)", subtitle: "可截图 / 操作")
                            MetricCard(title: "Codex 电脑", value: "\(model.availableServices.count)", subtitle: "可聊天")
                        }
                    }

                    ForEach(model.nodes) { node in
                        NodeCard(node: node)
                    }

                    if model.nodes.isEmpty {
                        EmptyStateView(title: "暂无节点", icon: "server.rack")
                    }
                }
                .padding(18)
                .padding(.bottom, 96)
            }
            .refreshable {
                await model.refreshAll()
            }
            .background(AppTheme.background.ignoresSafeArea())
            .navigationBarHidden(true)
        }
    }
}

struct TasksView: View {
    @ObservedObject var model: AgentGridMobileModel

    var body: some View {
        NavigationView {
            ScrollView {
                VStack(spacing: 12) {
                    HeaderBlock(title: "任务", subtitle: "\(model.tasks.count) 条最近记录", icon: "list.bullet.rectangle.fill")

                    SectionCard(title: "任务状态", icon: "chart.bar.fill") {
                        LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 10) {
                            MetricCard(title: "活跃", value: "\(model.activeTaskCount)", subtitle: "排队或执行中")
                            MetricCard(title: "完成", value: "\(model.doneTasks.count)", subtitle: "最近成功")
                            MetricCard(title: "失败", value: "\(model.failedTasks.count)", subtitle: "需要排查")
                            MetricCard(title: "总数", value: "\(model.tasks.count)", subtitle: "最近记录")
                        }
                    }

                    if model.tasks.isEmpty {
                        EmptyStateView(title: "暂无任务记录", icon: "list.bullet.rectangle")
                    } else {
                        ForEach(model.tasks) { task in
                            TaskCard(task: task)
                        }
                    }
                }
                .padding(18)
            }
            .refreshable {
                await model.refreshAll()
            }
            .background(AppTheme.background.ignoresSafeArea())
            .navigationBarHidden(true)
        }
    }
}

struct CodexBridgeView: View {
    @ObservedObject var model: AgentGridMobileModel
    @State private var showingComputerPicker = false
    @State private var showingProjectPicker = false

    var body: some View {
        NavigationView {
            Group {
                if model.codexBridgeConnected {
                    VStack(spacing: 0) {
                        CodexChatTopBar(
                            model: model,
                            chooseComputer: {
                                showingComputerPicker = true
                            },
                            chooseProject: {
                                showingProjectPicker = true
                            }
                        )
                        CodexChatCard(
                            model: model,
                            chooseProject: {
                                showingProjectPicker = true
                            }
                        )
                    }
                } else {
                    CodexConnectView(model: model, chooseComputer: {
                        showingComputerPicker = true
                    })
                }
            }
            .refreshable {
                await model.refreshAll()
            }
            .background(AppTheme.background.ignoresSafeArea())
            .navigationBarHidden(true)
            .sheet(isPresented: $showingComputerPicker) {
                CodexComputerPickerSheet(model: model)
            }
            .sheet(isPresented: $showingProjectPicker) {
                CodexProjectPickerSheet(model: model)
            }
            .hideTabBarWhen(model.codexBridgeConnected)
        }
    }
}

struct CodexConnectView: View {
    @ObservedObject var model: AgentGridMobileModel
    let chooseComputer: () -> Void

    private var connectTitle: String {
        model.selectedService == nil ? "刷新电脑" : "连接电脑"
    }

    private var canConnect: Bool {
        if model.isLoading { return false }
        guard let service = model.selectedService else { return true }
        return service.canTryConnect
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                CodexHeaderBlock(
                    title: "Codex",
                    subtitle: model.codexBusinessStatusDetail,
                    statusTitle: model.codexBusinessStatusTitle,
                    statusColor: model.codexBusinessStatusColor
                )

                Button {
                    Task {
                        if model.selectedService == nil {
                            await model.refreshAll()
                        } else {
                            await model.connectCodexChat()
                        }
                    }
                } label: {
                    WideButtonLabel(title: connectTitle, icon: "bolt.horizontal.circle.fill")
                }
                .buttonStyle(FilledButtonStyle())
                .disabled(!canConnect)

                HStack(spacing: 8) {
                    if model.isLoading {
                        ProgressView()
                            .scaleEffect(0.78)
                    } else {
                        Circle()
                            .fill(model.codexBusinessStatusColor)
                            .frame(width: 7, height: 7)
                    }
                    Text(model.compactActivityText)
                        .font(.caption)
                        .foregroundColor(.secondary)
                        .lineLimit(2)
                    Spacer(minLength: 0)
                }
                .padding(.horizontal, 2)

                VStack(alignment: .leading, spacing: 10) {
                    HStack {
                        Text("我的电脑")
                            .font(.headline.weight(.bold))
                        Spacer()
                        Button {
                            Task { await model.refreshAll() }
                        } label: {
                            Image(systemName: "arrow.clockwise")
                                .font(.system(size: 14, weight: .bold))
                                .foregroundColor(AppTheme.accent)
                                .frame(width: 32, height: 32)
                                .background(AppTheme.accent.opacity(0.10))
                                .clipShape(Circle())
                        }
                        .disabled(model.isLoading)
                    }

                    if model.services.isEmpty {
                        EmptyStateView(title: "暂无可用电脑", icon: "desktopcomputer")
                    } else {
                        ForEach(model.services) { service in
                            Button {
                                model.selectService(service)
                            } label: {
                                ComputerChoiceRow(service: service, selected: service.id == model.selectedServiceID)
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }
                .padding(16)
                .background(AppTheme.card)
                .cornerRadius(20)
            }
            .padding(16)
        }
        .background(AppTheme.background.ignoresSafeArea())
        .task {
            if model.services.isEmpty {
                await model.refreshAll()
            }
        }
    }
}

struct CodexHeaderBlock: View {
    let title: String
    let subtitle: String
    let statusTitle: String
    let statusColor: Color

    var body: some View {
        HStack(alignment: .center, spacing: 12) {
            VStack(alignment: .leading, spacing: 5) {
                Text(title)
                    .font(.system(size: 28, weight: .bold))
                Text(subtitle)
                    .font(.subheadline)
                    .foregroundColor(.secondary)
                    .lineLimit(1)
            }
            Spacer(minLength: 0)
            StatusPill(title: statusTitle, color: statusColor)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.top, 4)
    }
}

struct SelectedComputerCard: View {
    let service: LocalService?
    let statusTitle: String
    let statusColor: Color
    let detail: String
    let projectTitle: String
    let projectDetail: String
    let hasProject: Bool
    let chooseComputer: () -> Void
    let chooseProject: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .top, spacing: 13) {
                ZStack {
                    RoundedRectangle(cornerRadius: 16)
                        .fill(statusColor.opacity(0.12))
                        .frame(width: 50, height: 50)
                    Image(systemName: service == nil ? "desktopcomputer" : "macwindow")
                        .font(.system(size: 22, weight: .semibold))
                        .foregroundColor(statusColor)
                }

                VStack(alignment: .leading, spacing: 4) {
                    Text(service?.nodeName ?? "还没有选择电脑")
                        .font(.title3.weight(.bold))
                        .foregroundColor(.primary)
                        .lineLimit(1)
                    Text(detail)
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                        .lineLimit(3)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 0)
            }

            HStack {
                StatusPill(title: statusTitle, color: statusColor)
                Spacer()
                Button {
                    chooseComputer()
                } label: {
                    Label("换电脑", systemImage: "rectangle.2.swap")
                        .font(.caption.weight(.bold))
                        .foregroundColor(AppTheme.accent)
                        .padding(.horizontal, 12)
                        .padding(.vertical, 8)
                        .background(AppTheme.accent.opacity(0.10))
                        .clipShape(Capsule())
                }
            }

            Divider()

            HStack(alignment: .center, spacing: 12) {
                ZStack {
                    RoundedRectangle(cornerRadius: 13)
                        .fill((hasProject ? AppTheme.accent : Color.orange).opacity(0.12))
                        .frame(width: 40, height: 40)
                    Image(systemName: hasProject ? "folder.fill" : "folder.badge.questionmark")
                        .font(.system(size: 18, weight: .semibold))
                        .foregroundColor(hasProject ? AppTheme.accent : .orange)
                }

                VStack(alignment: .leading, spacing: 3) {
                    Text(projectTitle)
                        .font(.subheadline.weight(.bold))
                        .foregroundColor(.primary)
                        .lineLimit(1)
                    Text(projectDetail)
                        .font(.caption)
                        .foregroundColor(.secondary)
                        .lineLimit(2)
                }

                Spacer(minLength: 0)

                Button {
                    chooseProject()
                } label: {
                    Label(hasProject ? "换项目" : "选择项目", systemImage: "folder")
                        .font(.caption.weight(.bold))
                        .foregroundColor(AppTheme.accent)
                        .padding(.horizontal, 12)
                        .padding(.vertical, 8)
                        .background(AppTheme.accent.opacity(0.10))
                        .clipShape(Capsule())
                }
            }
        }
        .padding(16)
        .background(AppTheme.card)
        .cornerRadius(20)
    }
}

struct CodexConversationHeader: View {
    @ObservedObject var model: AgentGridMobileModel
    let chooseComputer: () -> Void

    private var actionTitle: String {
        if model.selectedService == nil { return "查找电脑" }
        if model.codexConnected { return "重连" }
        return "连接 Codex"
    }

    private var actionDisabled: Bool {
        if model.isLoading { return true }
        guard let service = model.selectedService else { return false }
        return !service.canTryConnect
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack(alignment: .center, spacing: 12) {
                ZStack {
                    RoundedRectangle(cornerRadius: 17)
                        .fill(model.codexBusinessStatusColor.opacity(0.12))
                        .frame(width: 52, height: 52)
                    Image(systemName: model.codexConnected ? "bubble.left.and.bubble.right.fill" : "desktopcomputer.and.macbook")
                        .font(.system(size: 23, weight: .semibold))
                        .foregroundColor(model.codexBusinessStatusColor)
                }

                VStack(alignment: .leading, spacing: 5) {
                    Text("Codex")
                        .font(.system(size: 28, weight: .bold))
                    Text("连接一台工作电脑，在手机上直接对话")
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                }
                Spacer(minLength: 0)

                Button {
                    chooseComputer()
                } label: {
                    Label("换电脑", systemImage: "rectangle.2.swap")
                        .font(.caption.weight(.bold))
                        .foregroundColor(AppTheme.accent)
                        .padding(.horizontal, 12)
                        .padding(.vertical, 8)
                        .background(AppTheme.accent.opacity(0.10))
                        .clipShape(Capsule())
                }
            }

            VStack(alignment: .leading, spacing: 14) {
                HStack(alignment: .top, spacing: 12) {
                    VStack(alignment: .leading, spacing: 5) {
                        Text(model.selectedService?.nodeName ?? "还没有选择电脑")
                            .font(.title3.weight(.bold))
                            .foregroundColor(.primary)
                            .lineLimit(1)
                        Text(model.codexConnected ? "手机已经接到这台电脑，可以开始发消息。" : model.codexBusinessStatusDetail)
                            .font(.subheadline)
                            .foregroundColor(.secondary)
                            .lineLimit(3)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                    Spacer(minLength: 0)
                    StatusPill(
                        title: model.codexConnected ? "聊天中" : model.codexBusinessStatusTitle,
                        color: model.codexConnected ? .green : model.codexBusinessStatusColor
                    )
                }

                Button {
                    Task {
                        if model.selectedService == nil {
                            await model.refreshAll()
                        } else {
                            await model.connectCodexChat()
                        }
                    }
                } label: {
                    WideButtonLabel(title: actionTitle, icon: model.codexConnected ? "arrow.clockwise.circle.fill" : "bolt.horizontal.circle.fill")
                }
                .buttonStyle(FilledButtonStyle())
                .disabled(actionDisabled)

                HStack(spacing: 8) {
                    if model.isLoading {
                        ProgressView()
                            .scaleEffect(0.78)
                    } else {
                        Circle()
                            .fill(model.codexBusinessStatusColor)
                            .frame(width: 7, height: 7)
                    }
                    Text(model.compactActivityText)
                        .font(.caption)
                        .foregroundColor(.secondary)
                        .lineLimit(2)
                    Spacer()
                }

                if model.codexConnected {
                    Button {
                        model.disconnectCodexChat()
                    } label: {
                        Label("结束当前聊天", systemImage: "xmark.circle")
                            .font(.caption.weight(.semibold))
                            .foregroundColor(.red)
                    }
                }
            }
            .padding(16)
            .background(AppTheme.card)
            .cornerRadius(20)
        }
    }
}

struct CodexChatTopBar: View {
    @ObservedObject var model: AgentGridMobileModel
    let chooseComputer: () -> Void
    let chooseProject: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            ZStack {
                RoundedRectangle(cornerRadius: 12)
                    .fill(Color.green.opacity(0.13))
                    .frame(width: 38, height: 38)
                Image(systemName: "bubble.left.and.bubble.right.fill")
                    .font(.system(size: 17, weight: .semibold))
                    .foregroundColor(.green)
            }

            VStack(alignment: .leading, spacing: 2) {
                Text(model.selectedService?.nodeName ?? "Codex")
                    .font(.headline.weight(.bold))
                    .lineLimit(1)
                Text(model.selectedProject?.displayName ?? "Codex 已连接")
                    .font(.caption)
                    .foregroundColor(.secondary)
                    .lineLimit(1)
            }

            Spacer(minLength: 0)

            Button {
                chooseProject()
            } label: {
                Image(systemName: "folder.fill")
                    .font(.system(size: 16, weight: .semibold))
                    .foregroundColor(AppTheme.accent)
                    .frame(width: 34, height: 34)
                    .background(AppTheme.accent.opacity(0.10))
                    .clipShape(Circle())
            }

            Button {
                chooseComputer()
            } label: {
                Image(systemName: "desktopcomputer")
                    .font(.system(size: 16, weight: .semibold))
                    .foregroundColor(AppTheme.accent)
                    .frame(width: 34, height: 34)
                    .background(AppTheme.accent.opacity(0.10))
                    .clipShape(Circle())
            }

            Button {
                model.disconnectCodexChat()
            } label: {
                Image(systemName: "xmark")
                    .font(.system(size: 15, weight: .bold))
                    .foregroundColor(.red)
                    .frame(width: 34, height: 34)
                    .background(Color.red.opacity(0.10))
                    .clipShape(Circle())
            }
        }
        .padding(.horizontal, 16)
        .padding(.top, 10)
        .padding(.bottom, 10)
        .background(AppTheme.card)
        .overlay(alignment: .bottom) {
            Rectangle()
                .fill(Color.black.opacity(0.06))
                .frame(height: 1)
        }
    }
}

struct CodexComputerPickerSheet: View {
    @ObservedObject var model: AgentGridMobileModel
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationView {
            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    VStack(alignment: .leading, spacing: 6) {
                        Text("选择工作电脑")
                            .font(.system(size: 28, weight: .bold))
                        Text("只连接已经准备好 Codex 聊天入口的电脑。")
                            .font(.subheadline)
                            .foregroundColor(.secondary)
                    }
                    .padding(.top, 4)

                    if model.services.isEmpty {
                        EmptyStateView(title: "还没有发现可聊天的电脑", icon: "desktopcomputer")
                    } else {
                        ForEach(model.services) { service in
                            Button {
                                model.selectService(service)
                                dismiss()
                            } label: {
                                ComputerChoiceRow(service: service, selected: service.id == model.selectedServiceID)
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }
                .padding(18)
            }
            .background(AppTheme.background.ignoresSafeArea())
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("关闭") { dismiss() }
                }
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button {
                        Task { await model.refreshAll() }
                    } label: {
                        Image(systemName: "arrow.clockwise")
                    }
                    .disabled(model.isLoading)
                }
            }
        }
    }
}

struct CodexProjectPickerSheet: View {
    @ObservedObject var model: AgentGridMobileModel
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationView {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    VStack(alignment: .leading, spacing: 6) {
                        Text("选择项目")
                            .font(.system(size: 28, weight: .bold))
                        Text(model.selectedService?.nodeName ?? "先选择一台工作电脑")
                            .font(.subheadline)
                            .foregroundColor(.secondary)
                            .lineLimit(1)
                    }
                    .padding(.top, 4)

                    SectionCard(title: "当前文件夹", icon: "folder.fill") {
                        VStack(alignment: .leading, spacing: 12) {
                            CurrentDirectoryRow(
                                title: model.remoteDirectoryTitle,
                                path: model.remoteDirectoryPath.isEmpty ? "正在读取这台电脑的目录" : model.remoteDirectoryPath,
                                loading: model.isBrowsingProjects
                            )

                            HStack(spacing: 10) {
                                Button {
                                    Task { await model.browseRemoteDirectoryParent() }
                                } label: {
                                    Label("上一级", systemImage: "chevron.up")
                                }
                                .buttonStyle(SoftButtonStyle())
                                .disabled(model.remoteDirectoryParentPath == nil || model.isBrowsingProjects)

                                Button {
                                    model.addProjectFromInput()
                                    if model.selectedProject != nil {
                                        dismiss()
                                    }
                                } label: {
                                    Label("使用此文件夹", systemImage: "checkmark.circle.fill")
                                }
                                .buttonStyle(FilledButtonStyle())
                                .disabled(model.remoteDirectoryPath.isEmpty || model.isBrowsingProjects)
                            }

                            if !model.remoteDirectoryError.isEmpty {
                                Text(model.remoteDirectoryError)
                                    .font(.caption)
                                    .foregroundColor(.red)
                                    .padding(10)
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                    .background(Color.red.opacity(0.08))
                                    .cornerRadius(12)
                            }
                        }
                    }

                    SectionCard(title: "文件夹", icon: "folder") {
                        VStack(spacing: 9) {
                            if model.isBrowsingProjects && model.remoteDirectoryEntries.isEmpty {
                                ProgressView()
                                    .frame(maxWidth: .infinity)
                                    .padding(.vertical, 18)
                            } else if model.remoteDirectoryEntries.isEmpty {
                                EmptyStateView(title: "这个位置没有可选文件夹", icon: "folder")
                            } else {
                                ForEach(model.remoteDirectoryEntries) { entry in
                                    Button {
                                        Task { await model.browseRemoteDirectory(entry.path) }
                                    } label: {
                                        RemoteDirectoryRow(entry: entry)
                                    }
                                    .buttonStyle(.plain)
                                    .disabled(model.isBrowsingProjects)
                                }
                            }
                        }
                    }

                    SectionCard(title: "最近项目", icon: "clock.arrow.circlepath") {
                        VStack(spacing: 10) {
                            if model.projectsForSelectedService.isEmpty {
                                EmptyStateView(title: "还没有项目记录", icon: "folder")
                            } else {
                                ForEach(model.projectsForSelectedService) { project in
                                    Button {
                                        model.selectProject(project)
                                        dismiss()
                                    } label: {
                                        ProjectChoiceRow(project: project, selected: project.id == model.selectedProjectID) {
                                            model.removeProject(project)
                                        }
                                    }
                                    .buttonStyle(.plain)
                                }
                            }
                        }
                    }
                }
                .padding(18)
            }
            .background(AppTheme.background.ignoresSafeArea())
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarLeading) {
                    Button("关闭") { dismiss() }
                }
            }
        }
        .task(id: model.selectedService?.nodeID ?? "") {
            if model.remoteDirectoryPath.isEmpty || model.remoteDirectoryNodeID != model.selectedService?.nodeID {
                await model.startProjectBrowser()
            }
        }
    }
}

struct CodexServicePickerCard: View {
    @ObservedObject var model: AgentGridMobileModel

    var body: some View {
        SectionCard(title: "我的电脑", icon: "desktopcomputer") {
            VStack(spacing: 10) {
                Button {
                    Task { await model.refreshAll() }
                } label: {
                    WideButtonLabel(title: "刷新工作电脑", icon: "arrow.clockwise")
                }
                .buttonStyle(SoftButtonStyle())
                .disabled(model.isLoading)

                if model.services.isEmpty {
                    EmptyStateView(title: "还没有发现可聊天的电脑", icon: "desktopcomputer")
                } else {
                    ForEach(model.services) { service in
                        Button {
                            model.selectService(service)
                        } label: {
                            ServiceRow(service: service, selected: service.id == model.selectedServiceID)
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
            .padding(.top, 10)
        }
    }
}

struct SettingsView: View {
    @ObservedObject var model: AgentGridMobileModel

    var body: some View {
        NavigationView {
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    HeaderBlock(title: "设置", subtitle: model.accountStatusTitle, icon: "gearshape.fill")

                    SectionCard(title: "连接", icon: "server.rack") {
                        VStack(alignment: .leading, spacing: 14) {
                            StatusBanner(
                                title: model.controlCenterStatusTitle,
                                subtitle: model.controlCenterStatusDetail,
                                loading: model.isLoading,
                                color: model.hubState == "在线" ? .green : .orange,
                                icon: model.hubState == "在线" ? "checkmark.circle.fill" : "clock.fill"
                            )

                            LabeledField(title: "地址") {
                                TextField("AgentGrid 控制中心地址", text: $model.hubURL)
                                    .textInputAutocapitalization(.never)
                                    .keyboardType(.URL)
                                    .disableAutocorrection(true)
                            }

                            Button {
                                Task { await model.connectControlCenter() }
                            } label: {
                                WideButtonLabel(title: "保存并连接", icon: "checkmark.seal.fill")
                            }
                            .buttonStyle(FilledButtonStyle())
                            .disabled(model.isLoading)
                        }
                    }

                    SectionCard(title: "账号", icon: "person.crop.circle.fill") {
                        VStack(alignment: .leading, spacing: 12) {
                            StatusBanner(
                                title: model.accountStatusTitle,
                                subtitle: model.accountStatusDetail,
                                loading: false,
                                color: model.isAuthenticated ? .green : .orange,
                                icon: model.isAuthenticated ? "checkmark.circle.fill" : "person.crop.circle.badge.exclamationmark"
                            )

                            LabeledField(title: "邮箱") {
                                TextField("邮箱", text: $model.email)
                                    .textInputAutocapitalization(.never)
                                    .keyboardType(.emailAddress)
                                    .disableAutocorrection(true)
                            }
                            LabeledField(title: "密码") {
                                SecureField("密码", text: $model.password)
                            }

                            HStack {
                                Button {
                                    Task { await model.login() }
                                } label: {
                                    Label("登录", systemImage: "person.crop.circle.badge.checkmark")
                                }
                                .buttonStyle(FilledButtonStyle())
                                .disabled(model.isLoading)

                                Button {
                                    model.logout()
                                } label: {
                                    Label("退出", systemImage: "rectangle.portrait.and.arrow.right")
                                }
                                .buttonStyle(SoftButtonStyle())
                                .disabled(!model.isAuthenticated)
                            }
                        }
                    }

                    SectionCard(title: "当前状态", icon: "checkmark.circle.fill") {
                        VStack(alignment: .leading, spacing: 10) {
                            SettingStatusRow(
                                icon: "server.rack",
                                title: "控制中心",
                                value: model.hubState == "在线" ? "在线" : model.compactActivityText,
                                color: model.hubState == "在线" ? .green : .orange
                            )
                            SettingStatusRow(
                                icon: "desktopcomputer",
                                title: "工作电脑",
                                value: model.availableServices.isEmpty ? "未发现可用电脑" : "可连接",
                                color: model.availableServices.isEmpty ? .orange : .green
                            )
                            SettingStatusRow(
                                icon: "bubble.left.and.bubble.right.fill",
                                title: "Codex 对话",
                                value: model.codexConnected ? "已连接" : model.codexBusinessStatusTitle,
                                color: model.codexConnected ? .green : model.codexBusinessStatusColor
                            )
                        }
                    }
                }
                .padding(18)
            }
            .background(AppTheme.background.ignoresSafeArea())
            .navigationBarHidden(true)
        }
    }
}

struct SettingStatusRow: View {
    let icon: String
    let title: String
    let value: String
    let color: Color

    var body: some View {
        HStack(spacing: 12) {
            ZStack {
                Circle()
                    .fill(color.opacity(0.12))
                    .frame(width: 34, height: 34)
                Image(systemName: icon)
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundColor(color)
            }

            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.subheadline.weight(.semibold))
                Text(value)
                    .font(.caption)
                    .foregroundColor(.secondary)
                    .lineLimit(2)
            }
            Spacer(minLength: 0)
        }
        .padding(12)
        .background(AppTheme.field)
        .cornerRadius(14)
    }
}

struct HeaderBlock: View {
    let title: String
    let subtitle: String
    let icon: String

    var body: some View {
        HStack(alignment: .center, spacing: 14) {
            ZStack {
                RoundedRectangle(cornerRadius: 18)
                    .fill(AppTheme.accent.opacity(0.13))
                    .frame(width: 58, height: 58)
                Image(systemName: icon)
                    .font(.system(size: 25, weight: .semibold))
                    .foregroundColor(AppTheme.accent)
            }

            VStack(alignment: .leading, spacing: 4) {
                Text(title)
                    .font(.system(size: 34, weight: .bold))
                    .foregroundColor(.primary)
                Text(subtitle)
                    .font(.subheadline)
                    .foregroundColor(.secondary)
            }
            Spacer()
        }
    }
}

struct StatusBanner: View {
    let title: String
    let subtitle: String
    let loading: Bool
    var color: Color = .green
    var icon: String = "checkmark.circle.fill"

    var body: some View {
        HStack(spacing: 12) {
            if loading {
                ProgressView()
            } else {
                Image(systemName: icon)
                    .foregroundColor(color)
            }
            VStack(alignment: .leading, spacing: 4) {
                Text(title)
                    .font(.headline)
                Text(subtitle)
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            Spacer()
        }
        .padding(14)
        .background(AppTheme.card)
        .cornerRadius(16)
    }
}

struct MetricCard: View {
    let title: String
    let value: String
    let subtitle: String

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title)
                .font(.caption.weight(.semibold))
                .foregroundColor(.secondary)
            Text(value)
                .font(.system(size: 28, weight: .bold))
                .minimumScaleFactor(0.7)
                .lineLimit(1)
            Text(subtitle)
                .font(.caption)
                .foregroundColor(.secondary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(14)
        .background(AppTheme.card)
        .cornerRadius(16)
    }
}

struct SectionCard<Content: View>: View {
    let title: String
    let icon: String
    let content: Content

    init(title: String, icon: String, @ViewBuilder content: () -> Content) {
        self.title = title
        self.icon = icon
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack {
                Image(systemName: icon)
                    .foregroundColor(AppTheme.accent)
                Text(title)
                    .font(.headline)
                Spacer()
            }
            content
        }
        .padding(16)
        .background(AppTheme.card)
        .cornerRadius(16)
    }
}

struct NodeCard: View {
    let node: AgentNode

    var body: some View {
        SectionCard(title: node.name, icon: iconForNode(node)) {
            VStack(alignment: .leading, spacing: 12) {
                HStack {
                    StatusPill(title: node.isOnline ? "在线" : node.state, color: node.isOnline ? .green : .gray)
                    StatusPill(title: node.shortOS, color: .blue)
                    Spacer()
                    Text("\(node.runningJobs)/\(node.maxConcurrentJobs) 槽")
                        .font(.caption.weight(.semibold))
                        .foregroundColor(.secondary)
                }

                CapabilityTile(icon: "network", title: "主机地址", value: node.address.isEmpty ? "未上报" : node.address)

                ResourceBar(title: "CPU", value: node.cpuUsagePercent, trailing: "\(node.cpuCores) 核")
                ResourceBar(title: "内存", value: node.memoryUsagePercent, trailing: "\(formatMB(node.memoryUsedMB)) / \(formatMB(node.memoryMB))")
                ResourceBar(title: "硬盘", value: node.diskUsagePercent, trailing: "\(formatMB(node.diskTotalMB - node.diskFreeMB)) / \(formatMB(node.diskTotalMB))")

                FlowTags(values: node.capabilities.prefix(8).map { $0 })
            }
        }
    }
}

struct ServiceRow: View {
    let service: LocalService
    let selected: Bool

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: service.isAvailable ? "checkmark.circle.fill" : "circle.dashed")
                .font(.title3)
                .foregroundColor(service.isAvailable ? .green : .gray)
            VStack(alignment: .leading, spacing: 4) {
                Text(service.nodeName)
                    .font(.subheadline.weight(.semibold))
                    .foregroundColor(.primary)
                Text(service.businessSubtitle)
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            Spacer()
            StatusPill(title: service.businessStatusTitle, color: service.isAvailable ? .green : .orange)
            if selected {
                Image(systemName: "checkmark")
                    .foregroundColor(AppTheme.accent)
            }
        }
        .padding(12)
        .background(selected ? AppTheme.accent.opacity(0.10) : AppTheme.field)
        .cornerRadius(14)
    }
}

struct ComputerChoiceRow: View {
    let service: LocalService
    let selected: Bool

    private var statusColor: Color {
        service.isAvailable ? .green : .orange
    }

    var body: some View {
        HStack(spacing: 12) {
            ZStack(alignment: .bottomTrailing) {
                RoundedRectangle(cornerRadius: 15)
                    .fill(statusColor.opacity(0.12))
                    .frame(width: 48, height: 48)
                Image(systemName: "desktopcomputer")
                    .font(.system(size: 21, weight: .semibold))
                    .foregroundColor(statusColor)
                Circle()
                    .fill(statusColor)
                    .frame(width: 11, height: 11)
                    .overlay(Circle().stroke(AppTheme.card, lineWidth: 2))
                    .offset(x: 1, y: 1)
            }

            VStack(alignment: .leading, spacing: 4) {
                Text(service.nodeName)
                    .font(.subheadline.weight(.bold))
                    .foregroundColor(.primary)
                    .lineLimit(1)
                Text(service.businessSubtitle)
                    .font(.caption)
                    .foregroundColor(.secondary)
                    .lineLimit(2)
            }

            Spacer(minLength: 0)

            if selected {
                Image(systemName: "checkmark.circle.fill")
                    .font(.title3)
                    .foregroundColor(AppTheme.accent)
            } else {
                Image(systemName: "chevron.right")
                    .font(.caption.weight(.bold))
                    .foregroundColor(.secondary.opacity(0.7))
            }
        }
        .padding(13)
        .background(selected ? AppTheme.accent.opacity(0.10) : AppTheme.card)
        .overlay(
            RoundedRectangle(cornerRadius: 18)
                .stroke(selected ? AppTheme.accent.opacity(0.28) : Color.black.opacity(0.05), lineWidth: 1)
        )
        .cornerRadius(18)
    }
}

struct CurrentDirectoryRow: View {
    let title: String
    let path: String
    let loading: Bool

    var body: some View {
        HStack(spacing: 12) {
            ZStack {
                RoundedRectangle(cornerRadius: 15)
                    .fill(AppTheme.accent.opacity(0.12))
                    .frame(width: 48, height: 48)
                if loading {
                    ProgressView()
                        .scaleEffect(0.78)
                } else {
                    Image(systemName: "folder.fill")
                        .font(.system(size: 21, weight: .semibold))
                        .foregroundColor(AppTheme.accent)
                }
            }

            VStack(alignment: .leading, spacing: 4) {
                Text(title)
                    .font(.subheadline.weight(.bold))
                    .foregroundColor(.primary)
                    .lineLimit(1)
                Text(path)
                    .font(.caption)
                    .foregroundColor(.secondary)
                    .lineLimit(2)
            }
            Spacer(minLength: 0)
        }
        .padding(13)
        .background(AppTheme.field)
        .cornerRadius(18)
    }
}

struct RemoteDirectoryRow: View {
    let entry: RemoteDirectoryEntry

    var body: some View {
        HStack(spacing: 12) {
            ZStack {
                RoundedRectangle(cornerRadius: 14)
                    .fill(AppTheme.accent.opacity(0.10))
                    .frame(width: 44, height: 44)
                Image(systemName: "folder")
                    .font(.system(size: 20, weight: .semibold))
                    .foregroundColor(AppTheme.accent)
            }

            VStack(alignment: .leading, spacing: 3) {
                Text(entry.name)
                    .font(.subheadline.weight(.semibold))
                    .foregroundColor(.primary)
                    .lineLimit(1)
                Text(entry.path)
                    .font(.caption)
                    .foregroundColor(.secondary)
                    .lineLimit(1)
            }

            Spacer(minLength: 0)

            Image(systemName: "chevron.right")
                .font(.caption.weight(.bold))
                .foregroundColor(.secondary.opacity(0.7))
        }
        .padding(12)
        .background(AppTheme.card)
        .overlay(
            RoundedRectangle(cornerRadius: 17)
                .stroke(Color.black.opacity(0.05), lineWidth: 1)
        )
        .cornerRadius(17)
    }
}

struct ProjectChoiceRow: View {
    let project: CodexProject
    let selected: Bool
    let remove: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            ZStack {
                RoundedRectangle(cornerRadius: 15)
                    .fill((selected ? AppTheme.accent : Color.gray).opacity(0.12))
                    .frame(width: 48, height: 48)
                Image(systemName: selected ? "folder.fill" : "folder")
                    .font(.system(size: 21, weight: .semibold))
                    .foregroundColor(selected ? AppTheme.accent : .secondary)
            }

            VStack(alignment: .leading, spacing: 4) {
                Text(project.displayName)
                    .font(.subheadline.weight(.bold))
                    .foregroundColor(.primary)
                    .lineLimit(1)
                Text(project.compactPath)
                    .font(.caption)
                    .foregroundColor(.secondary)
                    .lineLimit(2)
            }

            Spacer(minLength: 0)

            if selected {
                Image(systemName: "checkmark.circle.fill")
                    .font(.title3)
                    .foregroundColor(AppTheme.accent)
            }

            Button {
                remove()
            } label: {
                Image(systemName: "trash")
                    .font(.caption.weight(.bold))
                    .foregroundColor(.red)
                    .frame(width: 30, height: 30)
                    .background(Color.red.opacity(0.08))
                    .clipShape(Circle())
            }
            .buttonStyle(.plain)
        }
        .padding(13)
        .background(selected ? AppTheme.accent.opacity(0.10) : AppTheme.card)
        .overlay(
            RoundedRectangle(cornerRadius: 18)
                .stroke(selected ? AppTheme.accent.opacity(0.28) : Color.black.opacity(0.05), lineWidth: 1)
        )
        .cornerRadius(18)
    }
}

struct TaskCard: View {
    let task: AgentTaskItem

    var body: some View {
        SectionCard(title: task.title, icon: iconForTask(task)) {
            VStack(alignment: .leading, spacing: 12) {
                HStack {
                    StatusPill(title: task.stateTitle, color: colorForTaskState(task.state))
                    StatusPill(title: task.priority, color: .blue)
                    Spacer()
                    Text(task.shortUpdatedAt)
                        .font(.caption2)
                        .foregroundColor(.secondary)
                }

                if !task.nodeID.isEmpty {
                    CapabilityTile(icon: "server.rack", title: "执行节点", value: task.nodeID)
                }

                if let error = task.errorMessage, !error.isEmpty {
                    Text(error)
                        .font(.caption)
                        .foregroundColor(.red)
                        .lineLimit(3)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(10)
                        .background(Color.red.opacity(0.08))
                        .cornerRadius(12)
                }

                if task.labels.isEmpty {
                    Text("没有任务标签")
                        .font(.caption)
                        .foregroundColor(.secondary)
                } else {
                    FlowTags(values: task.labels.prefix(6).map { $0 })
                }
            }
        }
    }
}

struct CodexChatCard: View {
    @ObservedObject var model: AgentGridMobileModel
    let chooseProject: () -> Void
    @FocusState private var inputFocused: Bool
    @State private var showingPhotoPicker = false

    private var visibleMessages: [CodexChatMessage] {
        model.codexMessages.filter { !$0.isSystem }
    }

    private var showSuggestions: Bool {
        model.codexConnected && visibleMessages.isEmpty && !inputFocused
    }

    private var canSend: Bool {
        model.canSendCodexMessage
    }

    var body: some View {
        VStack(spacing: 0) {
            if model.selectedProject == nil {
                ConnectedProjectPrompt(model: model, chooseProject: chooseProject)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                if showSuggestions {
                    QuickPromptBar(model: model)
                        .padding(.horizontal, 16)
                        .padding(.top, 8)
                        .padding(.bottom, 6)
                }

                ScrollViewReader { proxy in
                    ScrollView {
                        VStack(spacing: 12) {
                            if visibleMessages.isEmpty {
                                CodexConversationEmptyState(projectName: model.selectedProject?.displayName)
                                    .padding(.top, 20)
                            } else {
                                ForEach(visibleMessages) { message in
                                    ChatBubble(message: message)
                                        .id(message.id)
                                }
                            }
                        }
                        .padding(.horizontal, 14)
                        .padding(.top, showSuggestions ? 2 : 10)
                        .padding(.bottom, 10)
                    }
                    .dismissesKeyboardInteractively()
                    .onTapGesture {
                        inputFocused = false
                    }
                    .onChange(of: model.codexMessages) { _ in
                        guard let last = visibleMessages.last else { return }
                        withAnimation(.easeOut(duration: 0.2)) {
                            proxy.scrollTo(last.id, anchor: .bottom)
                        }
                    }
                }

                Divider()

                VStack(spacing: 7) {
                    if !model.pendingImages.isEmpty {
                        ScrollView(.horizontal, showsIndicators: false) {
                            HStack(spacing: 10) {
                                ForEach(model.pendingImages) { image in
                                    PendingImageThumb(image: image) {
                                        model.removePendingImage(image)
                                    }
                                }
                            }
                            .padding(.horizontal, 16)
                        }
                    }

                    HStack(alignment: .bottom, spacing: 10) {
                        Button {
                            showingPhotoPicker = true
                        } label: {
                            Image(systemName: "photo.on.rectangle.angled")
                                .font(.system(size: 18, weight: .semibold))
                                .foregroundColor(model.codexConnected ? AppTheme.accent : .gray)
                                .frame(width: 38, height: 38)
                                .background(AppTheme.field)
                                .clipShape(Circle())
                        }
                        .disabled(!model.codexConnected)

                        ZStack(alignment: .topLeading) {
                            TextEditor(text: $model.codexChatInput)
                                .font(.body)
                                .frame(height: inputHeight)
                                .padding(.horizontal, 9)
                                .padding(.vertical, 5)
                                .scrollContentBackgroundHiddenIfAvailable()
                                .background(AppTheme.field)
                                .cornerRadius(19)
                                .focused($inputFocused)
                                .disabled(!model.codexConnected)

                            if model.codexChatInput.isEmpty {
                                Text("输入消息")
                                    .font(.body)
                                    .foregroundColor(.secondary.opacity(0.72))
                                    .padding(.horizontal, 14)
                                    .padding(.vertical, 12)
                                    .allowsHitTesting(false)
                            }
                        }

                        Button {
                            inputFocused = false
                            dismissKeyboard()
                            Task { await model.sendCodexChatMessage() }
                        } label: {
                            Image(systemName: "arrow.up")
                                .font(.system(size: 17, weight: .bold))
                                .foregroundColor(.white)
                                .frame(width: 38, height: 38)
                                .background(
                                    Circle()
                                        .fill(canSend ? AppTheme.accent : Color.gray.opacity(0.42))
                                )
                        }
                        .disabled(!canSend)
                    }
                }
                .padding(.top, 8)
                .padding(.horizontal, 14)
                .padding(.bottom, 8)
                .background(AppTheme.card)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(AppTheme.background)
        .sheet(isPresented: $showingPhotoPicker) {
            PhotoLibraryPicker(selectionLimit: 2) { data in
                model.addPendingImageDataList(data)
            }
        }
    }

    private var inputHeight: CGFloat {
        let text = model.codexChatInput
        let explicitLines = text.components(separatedBy: .newlines).count
        let estimatedWrappedLines = max(1, text.count / 18 + 1)
        let lines = min(4, max(explicitLines, estimatedWrappedLines))
        return CGFloat(38 + (lines - 1) * 22)
    }
}

struct ConnectedProjectPrompt: View {
    @ObservedObject var model: AgentGridMobileModel
    let chooseProject: () -> Void

    var body: some View {
        VStack(spacing: 18) {
            Spacer(minLength: 20)

            ZStack {
                Circle()
                    .fill(AppTheme.accent.opacity(0.10))
                    .frame(width: 78, height: 78)
                Image(systemName: "folder.fill")
                    .font(.system(size: 31, weight: .semibold))
                    .foregroundColor(AppTheme.accent)
            }

            VStack(spacing: 6) {
                Text("打开项目")
                    .font(.title3.weight(.bold))
                Text(model.selectedService?.nodeName ?? "工作电脑")
                    .font(.subheadline)
                    .foregroundColor(.secondary)
                    .lineLimit(1)
            }

            Button(action: chooseProject) {
                WideButtonLabel(title: "打开项目", icon: "folder.fill")
            }
            .buttonStyle(FilledButtonStyle())
            .frame(maxWidth: 260)

            if !model.projectsForSelectedService.isEmpty {
                VStack(alignment: .leading, spacing: 10) {
                    Text("最近项目")
                        .font(.caption.weight(.bold))
                        .foregroundColor(.secondary)
                    ForEach(model.projectsForSelectedService.prefix(3)) { project in
                        Button {
                            model.selectProject(project)
                        } label: {
                            HStack(spacing: 10) {
                                Image(systemName: "folder")
                                    .foregroundColor(AppTheme.accent)
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(project.displayName)
                                        .font(.subheadline.weight(.semibold))
                                        .foregroundColor(.primary)
                                    Text(project.compactPath)
                                        .font(.caption)
                                        .foregroundColor(.secondary)
                                        .lineLimit(1)
                                }
                                Spacer(minLength: 0)
                            }
                            .padding(12)
                            .background(AppTheme.card)
                            .cornerRadius(14)
                        }
                        .buttonStyle(.plain)
                    }
                }
                .padding(.horizontal, 18)
            }

            Spacer(minLength: 20)
        }
        .padding(18)
    }
}

struct PhotoLibraryPicker: UIViewControllerRepresentable {
    let selectionLimit: Int
    let onPick: ([Data]) -> Void
    @Environment(\.dismiss) private var dismiss

    func makeUIViewController(context: Context) -> PHPickerViewController {
        var configuration = PHPickerConfiguration(photoLibrary: .shared())
        configuration.filter = .images
        configuration.selectionLimit = selectionLimit
        let controller = PHPickerViewController(configuration: configuration)
        controller.delegate = context.coordinator
        return controller
    }

    func updateUIViewController(_ uiViewController: PHPickerViewController, context: Context) {}

    func makeCoordinator() -> Coordinator {
        Coordinator(onPick: onPick, dismiss: dismiss)
    }

    final class Coordinator: NSObject, PHPickerViewControllerDelegate {
        let onPick: ([Data]) -> Void
        let dismiss: DismissAction

        init(onPick: @escaping ([Data]) -> Void, dismiss: DismissAction) {
            self.onPick = onPick
            self.dismiss = dismiss
        }

        func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) {
            dismiss()
            guard !results.isEmpty else { return }

            let group = DispatchGroup()
            var values = Array<Data?>(repeating: nil, count: results.count)
            for (index, result) in results.enumerated() {
                let provider = result.itemProvider
                guard provider.hasItemConformingToTypeIdentifier(UTType.image.identifier) else {
                    continue
                }
                group.enter()
                provider.loadDataRepresentation(forTypeIdentifier: UTType.image.identifier) { data, _ in
                    values[index] = data
                    group.leave()
                }
            }

            group.notify(queue: .main) {
                self.onPick(values.compactMap { $0 })
            }
        }
    }
}

struct CodexConversationEmptyState: View {
    let projectName: String?

    var body: some View {
        VStack(spacing: 14) {
            ZStack {
                Circle()
                    .fill(AppTheme.accent.opacity(0.10))
                    .frame(width: 72, height: 72)
                Image(systemName: "bubble.left.and.bubble.right.fill")
                    .font(.system(size: 29, weight: .semibold))
                    .foregroundColor(AppTheme.accent)
            }

            VStack(spacing: 6) {
                Text(projectName ?? "项目")
                    .font(.headline.weight(.bold))
                Text("已打开")
                    .font(.subheadline)
                    .foregroundColor(.secondary)
                    .multilineTextAlignment(.center)
                    .lineLimit(3)
            }
        }
        .frame(maxWidth: .infinity)
    }
}

struct QuickPromptBar: View {
    @ObservedObject var model: AgentGridMobileModel

    private let prompts = [
        ("项目进展", "总结 AgentGrid 当前进展"),
        ("检查代码", "检查本机项目状态"),
        ("下一步", "规划下一步最有价值优化"),
    ]

    var body: some View {
        VStack(alignment: .leading, spacing: 9) {
            Text("你可以这样开始")
                .font(.caption.weight(.bold))
                .foregroundColor(.secondary)
            HStack(spacing: 8) {
                ForEach(prompts, id: \.0) { prompt in
                    Button {
                        model.codexChatInput = prompt.1
                    } label: {
                        Text(prompt.0)
                            .font(.caption.weight(.semibold))
                            .foregroundColor(AppTheme.accent)
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 9)
                            .background(AppTheme.card)
                            .overlay(
                                RoundedRectangle(cornerRadius: 14)
                                    .stroke(AppTheme.accent.opacity(0.14), lineWidth: 1)
                            )
                            .cornerRadius(14)
                    }
                    .buttonStyle(.plain)
                }
            }
        }
    }
}

struct PendingImageThumb: View {
    let image: CodexImageAttachment
    let remove: () -> Void

    var body: some View {
        ZStack(alignment: .topTrailing) {
            Group {
                #if canImport(UIKit)
                if let uiImage = image.image {
                    Image(uiImage: uiImage)
                        .resizable()
                        .scaledToFill()
                } else {
                    Image(systemName: "photo")
                        .font(.title2)
                        .foregroundColor(.secondary)
                }
                #else
                Image(systemName: "photo")
                    .font(.title2)
                    .foregroundColor(.secondary)
                #endif
            }
            .frame(width: 74, height: 74)
            .background(AppTheme.field)
            .clipShape(RoundedRectangle(cornerRadius: 14))

            Button {
                remove()
            } label: {
                Image(systemName: "xmark")
                    .font(.caption2.weight(.bold))
                    .foregroundColor(.white)
                    .frame(width: 21, height: 21)
                    .background(Color.black.opacity(0.62))
                    .clipShape(Circle())
            }
            .offset(x: 7, y: -7)
        }
        .padding(.top, 7)
    }
}

struct MessageImageGrid: View {
    let images: [CodexImageAttachment]

    var body: some View {
        LazyVGrid(columns: [GridItem(.adaptive(minimum: 70), spacing: 7)], spacing: 7) {
            ForEach(images) { image in
                Group {
                    #if canImport(UIKit)
                    if let uiImage = image.image {
                        Image(uiImage: uiImage)
                            .resizable()
                            .scaledToFill()
                    } else {
                        Image(systemName: "photo")
                            .font(.title2)
                            .foregroundColor(.secondary)
                    }
                    #else
                    Image(systemName: "photo")
                        .font(.title2)
                        .foregroundColor(.secondary)
                    #endif
                }
                .frame(width: 78, height: 78)
                .background(AppTheme.field)
                .clipShape(RoundedRectangle(cornerRadius: 13))
            }
        }
        .frame(maxWidth: 190, alignment: .trailing)
    }
}

struct ChatBubble: View {
    let message: CodexChatMessage

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            if !message.isUser {
                assistantAvatar
                    .padding(.top, 2)
            } else {
                Spacer(minLength: 58)
            }

            VStack(alignment: message.isUser ? .trailing : .leading, spacing: 6) {
                if !message.attachments.isEmpty {
                    MessageImageGrid(images: message.attachments)
                }

                if !message.text.isEmpty || message.attachments.isEmpty {
                    Text(formattedText)
                        .font(.system(size: message.isUser ? 16 : 15, weight: .regular))
                        .lineSpacing(message.isUser ? 1 : 3)
                        .foregroundColor(message.isUser ? .white : .primary)
                        .textSelection(.enabled)
                        .padding(.horizontal, 14)
                        .padding(.vertical, message.isUser ? 9 : 10)
                        .background(backgroundColor)
                        .overlay(
                            ChatBubbleShape(isUser: message.isUser)
                                .stroke(borderColor, lineWidth: message.isUser ? 0 : 1)
                        )
                        .clipShape(ChatBubbleShape(isUser: message.isUser))
                        .frame(maxWidth: message.isUser ? 274 : 304, alignment: message.isUser ? .trailing : .leading)
                }

                if message.isStreaming {
                    Text("正在整理")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                }
            }

            if !message.isUser {
                Spacer(minLength: 38)
            }
        }
    }

    private var backgroundColor: Color {
        if message.isUser { return AppTheme.accent }
        if message.isSystem { return Color.orange.opacity(0.14) }
        return AppTheme.card
    }

    private var borderColor: Color {
        message.isUser ? Color.clear : Color.black.opacity(0.05)
    }

    private var assistantAvatar: some View {
        ZStack {
            Circle()
                .fill(AppTheme.accent.opacity(0.10))
            Image(systemName: "bolt.horizontal.circle.fill")
                .font(.system(size: 15, weight: .semibold))
                .foregroundColor(AppTheme.accent)
        }
        .frame(width: 28, height: 28)
    }

    private var formattedText: AttributedString {
        Self.formattedText(message.text.isEmpty ? "..." : message.text, isUser: message.isUser)
    }

    private static func formattedText(_ text: String, isUser: Bool) -> AttributedString {
        let cleaned = cleanText(text)
        if let markdown = try? AttributedString(
            markdown: cleaned,
            options: AttributedString.MarkdownParsingOptions(interpretedSyntax: .inlineOnlyPreservingWhitespace)
        ) {
            return markdown
        }
        return AttributedString(cleaned)
    }

    private static func cleanText(_ text: String) -> String {
        var value = text
            .replacingOccurrences(of: "\r\n", with: "\n")
            .replacingOccurrences(of: "\r", with: "\n")
        value = value.replacingOccurrences(
            of: "```[a-zA-Z0-9_+\\-]*\\n",
            with: "",
            options: .regularExpression
        )
        value = value.replacingOccurrences(of: "\n```", with: "\n")
        value = value.replacingOccurrences(of: "```", with: "")
        value = value.replacingOccurrences(
            of: #"(?m)^\s*[-*]\s+"#,
            with: "· ",
            options: .regularExpression
        )
        value = value.replacingOccurrences(
            of: #"\n{3,}"#,
            with: "\n\n",
            options: .regularExpression
        )
        return value.trimmingCharacters(in: .whitespacesAndNewlines)
    }
}

struct ChatBubbleShape: Shape {
    let isUser: Bool

    func path(in rect: CGRect) -> Path {
        let corners: UIRectCorner = isUser
            ? [.topLeft, .bottomLeft, .topRight]
            : [.topLeft, .topRight, .bottomRight]
        return Path(
            UIBezierPath(
                roundedRect: rect,
                byRoundingCorners: corners,
                cornerRadii: CGSize(width: 18, height: 18)
            ).cgPath
        )
    }
}

struct MethodPicker: View {
    @Binding var selection: String
    private let methods = ["GET", "POST", "PUT", "PATCH", "DELETE"]

    var body: some View {
        Picker("方法", selection: $selection) {
            ForEach(methods, id: \.self) { method in
                Text(method).tag(method)
            }
        }
        .pickerStyle(.segmented)
    }
}

struct LabeledField<Content: View>: View {
    let title: String
    let content: Content

    init(title: String, @ViewBuilder content: () -> Content) {
        self.title = title
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title)
                .font(.caption.weight(.semibold))
                .foregroundColor(.secondary)
            content
                .padding(12)
                .background(AppTheme.field)
                .cornerRadius(12)
        }
    }
}

struct ResourceBar: View {
    let title: String
    let value: Double
    let trailing: String

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text(title)
                    .font(.caption.weight(.semibold))
                    .foregroundColor(.secondary)
                Spacer()
                Text(trailing)
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            ProgressView(value: min(max(value, 0), 100), total: 100)
                .tint(progressColor(value))
        }
    }
}

struct FlowTags: View {
    let values: [String]

    var body: some View {
        FlexibleWrap(values: values) { value in
            Text(value)
                .font(.caption.weight(.semibold))
                .foregroundColor(.secondary)
                .padding(.horizontal, 9)
                .padding(.vertical, 5)
                .background(AppTheme.field)
                .cornerRadius(999)
        }
    }
}

struct FlexibleWrap<Data: RandomAccessCollection, Content: View>: View where Data.Element: Hashable {
    let values: Data
    let content: (Data.Element) -> Content

    var body: some View {
        LazyVGrid(columns: [GridItem(.adaptive(minimum: 72), spacing: 8)], alignment: .leading, spacing: 8) {
            ForEach(Array(values), id: \.self) { value in
                content(value)
            }
        }
    }
}

struct CapabilityTile: View {
    let icon: String
    let title: String
    let value: String

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: icon)
                .font(.headline)
                .foregroundColor(AppTheme.accent)
                .frame(width: 28)
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.subheadline.weight(.semibold))
                Text(value)
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            Spacer()
        }
        .padding(12)
        .background(AppTheme.field)
        .cornerRadius(14)
    }
}

struct WideButtonLabel: View {
    let title: String
    let icon: String

    var body: some View {
        HStack {
            Image(systemName: icon)
            Text(title)
                .font(.subheadline.weight(.semibold))
            Spacer()
        }
        .padding(.vertical, 2)
    }
}

struct StatusPill: View {
    let title: String
    let color: Color

    var body: some View {
        Text(title)
            .font(.caption.weight(.semibold))
            .foregroundColor(color)
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(color.opacity(0.12))
            .cornerRadius(999)
    }
}

struct EmptyStateView: View {
    let title: String
    let icon: String

    var body: some View {
        VStack(spacing: 8) {
            Image(systemName: icon)
                .font(.title2)
                .foregroundColor(.secondary)
            Text(title)
                .font(.subheadline)
                .foregroundColor(.secondary)
        }
        .frame(maxWidth: .infinity)
        .padding(18)
        .background(AppTheme.field)
        .cornerRadius(14)
    }
}

struct FilledButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.subheadline.weight(.semibold))
            .foregroundColor(.white)
            .padding(.horizontal, 14)
            .padding(.vertical, 11)
            .frame(maxWidth: .infinity)
            .background(configuration.isPressed ? AppTheme.accent.opacity(0.75) : AppTheme.accent)
            .cornerRadius(14)
    }
}

struct SoftButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.subheadline.weight(.semibold))
            .foregroundColor(AppTheme.accent)
            .padding(.horizontal, 14)
            .padding(.vertical, 11)
            .frame(maxWidth: .infinity)
            .background(configuration.isPressed ? AppTheme.accent.opacity(0.18) : AppTheme.accent.opacity(0.10))
            .cornerRadius(14)
    }
}

extension View {
    @ViewBuilder
    func dismissesKeyboardInteractively() -> some View {
        if #available(iOS 16.0, *) {
            self.scrollDismissesKeyboard(.interactively)
        } else {
            self
        }
    }

    @ViewBuilder
    func hideTabBarWhen(_ hidden: Bool) -> some View {
        if #available(iOS 16.0, *) {
            self.toolbar(hidden ? .hidden : .visible, for: .tabBar)
        } else {
            self
        }
    }

    @ViewBuilder
    func scrollContentBackgroundHiddenIfAvailable() -> some View {
        if #available(iOS 16.0, *) {
            self.scrollContentBackground(.hidden)
        } else {
            self
        }
    }
}

enum AppTheme {
    static let accent = Color(red: 0.04, green: 0.36, blue: 0.92)
    static let background = Color(.systemGroupedBackground)
    static let card = Color(.systemBackground)
    static let field = Color(.secondarySystemGroupedBackground)
}

func dismissKeyboard() {
    #if canImport(UIKit)
    UIApplication.shared.sendAction(
        #selector(UIResponder.resignFirstResponder),
        to: nil,
        from: nil,
        for: nil
    )
    #endif
}

func iconForNode(_ node: AgentNode) -> String {
    if node.capabilities.contains("desktop") { return "display" }
    if node.shortOS == "Windows" { return "pc" }
    if node.shortOS == "macOS" { return "macbook" }
    return "server.rack"
}

func iconForTask(_ task: AgentTaskItem) -> String {
    if task.labels.contains("command") { return "terminal" }
    if task.labels.contains("browser") { return "safari" }
    if task.labels.contains("desktop") { return "display" }
    if task.labels.contains("plugin") { return "shippingbox.fill" }
    return "list.bullet.rectangle"
}

func progressColor(_ value: Double) -> Color {
    if value >= 85 { return .red }
    if value >= 65 { return .orange }
    return .green
}

func colorForTaskState(_ state: String) -> Color {
    switch state {
    case "done": return .green
    case "failed": return .red
    case "in_progress": return .blue
    case "assigned": return .orange
    case "blocked": return .purple
    default: return .gray
    }
}

func formatMB(_ value: Int) -> String {
    if value <= 0 { return "0 GB" }
    let gb = Double(value) / 1024.0
    if gb >= 100 {
        return "\(Int(gb.rounded())) GB"
    }
    return String(format: "%.1f GB", gb)
}
