import Foundation

public typealias AgentGridJSONObject = [String: Any]

public enum AgentGridMobileError: Error, LocalizedError {
    case invalidURL(String)
    case invalidJSON
    case httpStatus(Int, String)
    case apiError(String)

    public var errorDescription: String? {
        switch self {
        case .invalidURL(let value):
            return "Invalid AgentGrid URL: \(value)"
        case .invalidJSON:
            return "AgentGrid response is not a JSON object."
        case .httpStatus(let status, let body):
            return "AgentGrid HTTP error \(status): \(body)"
        case .apiError(let message):
            return message
        }
    }
}

public struct AgentGridMobileClient {
    /// The public AgentGrid hub currently serves HTTP on port 20080.
    /// iOS apps that use this default must add a scoped App Transport Security
    /// exception for `chenqi.tminos.com` in the app target's Info.plist.
    public static let defaultHubURL = URL(string: "http://chenqi.tminos.com:20080/agentgrid")!

    private let hubURL: URL
    private let bearerToken: String?
    private let session: URLSession

    public init(
        hubURL: URL = AgentGridMobileClient.defaultHubURL,
        bearerToken: String? = nil,
        session: URLSession = .shared
    ) {
        self.hubURL = hubURL.absoluteString.hasSuffix("/")
            ? URL(string: String(hubURL.absoluteString.dropLast()))!
            : hubURL
        self.bearerToken = bearerToken
        self.session = session
    }

    public func health() async throws -> AgentGridJSONObject {
        try await get("/api/health")
    }

    public func runtimeStandard() async throws -> AgentGridJSONObject {
        try await get("/api/runtime-standard")
    }

    public func mobileSdkStandard() async throws -> AgentGridJSONObject {
        try await get("/api/runtime-standard/mobile-sdk")
    }

    public func workbenches() async throws -> AgentGridJSONObject {
        try await get("/api/workbenches")
    }

    public func workbench(_ workbenchID: String) async throws -> AgentGridJSONObject {
        try await get("/api/workbenches/\(Self.pathComponent(workbenchID))")
    }

    public func workbenchTimeline(_ workbenchID: String) async throws -> AgentGridJSONObject {
        try await get("/api/workbenches/\(Self.pathComponent(workbenchID))/timeline")
    }

    public func devices() async throws -> AgentGridJSONObject {
        try await get("/api/runtime-standard/devices")
    }

    public func evidenceStandard() async throws -> AgentGridJSONObject {
        try await get("/api/runtime-standard/evidence")
    }

    public func nodes() async throws -> AgentGridJSONObject {
        try await get("/api/nodes")
    }

    public func tools() async throws -> AgentGridJSONObject {
        try await get("/api/tools")
    }

    public func localServices() async throws -> AgentGridJSONObject {
        try await get("/api/local-services")
    }

    public func createBridgeSession(
        nodeID: String,
        serviceID: String = "codex.local"
    ) async throws -> AgentGridJSONObject {
        try await post("/api/bridge-sessions", body: [
            "node_id": nodeID,
            "service_id": serviceID
        ])
    }

    public func bridgeWebSocketURL(sessionID: String, token: String? = nil) throws -> URL {
        guard var components = URLComponents(
            url: try endpointURL("api/bridge-sessions/\(sessionID)/ws"),
            resolvingAgainstBaseURL: false
        ) else {
            throw AgentGridMobileError.invalidURL(sessionID)
        }
        components.scheme = hubURL.scheme == "https" ? "wss" : "ws"
        if let token {
            components.queryItems = [URLQueryItem(name: "token", value: token)]
        }
        guard let url = components.url else {
            throw AgentGridMobileError.invalidURL(sessionID)
        }
        return url
    }

    public func listPortBridges() async throws -> AgentGridJSONObject {
        try await get("/api/port-bridges")
    }

    public func createPortBridge(
        sourceNodeID: String,
        targetNodeID: String,
        targetPort: Int,
        sourceBindPort: Int = 0,
        targetHost: String = "127.0.0.1",
        sourceBindHost: String = "127.0.0.1",
        ttlSeconds: Int = 1800,
        purpose: String? = nil,
        createdBy: String = "agentgrid-mobile-sdk"
    ) async throws -> AgentGridJSONObject {
        var body: AgentGridJSONObject = [
            "source_node_id": sourceNodeID,
            "target_node_id": targetNodeID,
            "source_bind_host": sourceBindHost,
            "source_bind_port": sourceBindPort,
            "target_host": targetHost,
            "target_port": targetPort,
            "protocol": "tcp",
            "ttl_seconds": ttlSeconds,
            "created_by": createdBy
        ]
        if let purpose {
            body["purpose"] = purpose
        }
        return try await post("/api/port-bridges", body: body)
    }

    public func getPortBridge(_ portBridgeID: String) async throws -> AgentGridJSONObject {
        try await get("/api/port-bridges/\(portBridgeID)")
    }

    public func closePortBridge(_ portBridgeID: String) async throws -> AgentGridJSONObject {
        try await delete("/api/port-bridges/\(portBridgeID)")
    }

    public func submitTask(_ request: AgentGridJSONObject) async throws -> AgentGridJSONObject {
        try await post("/api/agent-runtime/tasks", body: request)
    }

    public func runCommand(
        program: String,
        args: [String] = [],
        nodeID: String? = nil,
        workbenchID: String? = nil,
        title: String? = nil
    ) async throws -> AgentGridJSONObject {
        var body: AgentGridJSONObject = [
            "tool_id": "command.run",
            "title": title ?? "command \(program)",
            "payload": [
                "type": "command",
                "program": program,
                "args": args,
                "working_dir": NSNull(),
                "timeout_seconds": 30
            ],
            "verify": ["presets": ["command.exit_zero"]]
        ]
        if let nodeID {
            body["node_id"] = nodeID
        }
        if let workbenchID {
            body["workbench_id"] = workbenchID
        }
        return try await submitTask(body)
    }

    public func runPlugin(
        pluginID: String,
        action: String = "run",
        input: AgentGridJSONObject = [:],
        nodeID: String? = nil,
        workbenchID: String? = nil,
        title: String? = nil
    ) async throws -> AgentGridJSONObject {
        var body: AgentGridJSONObject = [
            "tool_id": "plugin.run",
            "title": title ?? "plugin \(pluginID):\(action)",
            "payload": [
                "type": "plugin",
                "plugin_id": pluginID,
                "action": action,
                "input": input,
                "timeout_seconds": 60
            ],
            "verify": ["rules": [["path": "result.output", "op": "exists"]]]
        ]
        if let nodeID {
            body["node_id"] = nodeID
        }
        if let workbenchID {
            body["workbench_id"] = workbenchID
        }
        return try await submitTask(body)
    }

    public func getTask(_ taskID: String) async throws -> AgentGridJSONObject {
        try await get("/api/agent-runtime/tasks/\(taskID)")
    }

    public func taskEvents(_ taskID: String) async throws -> AgentGridJSONObject {
        try await get("/api/agent-runtime/tasks/\(taskID)/events")
    }

    public func executionRecord(taskID: String) async throws -> AgentGridJSONObject {
        try await get("/api/execution-records/tasks/\(taskID)")
    }

    public func artifacts() async throws -> AgentGridJSONObject {
        try await get("/api/artifacts")
    }

    public func artifactDownloadURL(artifactID: String) -> URL {
        try! endpointURL("api/artifacts/\(artifactID)/download")
    }

    public func taskTemplates() async throws -> AgentGridJSONObject {
        try await get("/api/task-templates")
    }

    public func startTaskTemplate(
        templateID: String,
        request: AgentGridJSONObject = [:]
    ) async throws -> AgentGridJSONObject {
        try await post("/api/task-templates/\(templateID)/start", body: request)
    }

    public func get(_ path: String) async throws -> AgentGridJSONObject {
        try await send(path: path, method: "GET", body: nil)
    }

    public func post(_ path: String, body: AgentGridJSONObject) async throws -> AgentGridJSONObject {
        try await send(path: path, method: "POST", body: body)
    }

    public func delete(_ path: String) async throws -> AgentGridJSONObject {
        try await send(path: path, method: "DELETE", body: nil)
    }

    private func send(
        path: String,
        method: String,
        body: AgentGridJSONObject?
    ) async throws -> AgentGridJSONObject {
        let url = try endpointURL(path)

        var request = URLRequest(url: url)
        request.httpMethod = method
        request.setValue("application/json", forHTTPHeaderField: "accept")
        if let bearerToken {
            request.setValue("Bearer \(bearerToken)", forHTTPHeaderField: "authorization")
        }
        if let body {
            request.setValue("application/json", forHTTPHeaderField: "content-type")
            request.httpBody = try JSONSerialization.data(withJSONObject: body)
        }

        let (data, response) = try await session.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse else {
            throw AgentGridMobileError.invalidJSON
        }
        guard (200..<300).contains(httpResponse.statusCode) else {
            let text = String(data: data, encoding: .utf8) ?? ""
            throw AgentGridMobileError.httpStatus(httpResponse.statusCode, text)
        }
        guard let object = try JSONSerialization.jsonObject(with: data) as? AgentGridJSONObject else {
            throw AgentGridMobileError.invalidJSON
        }
        if let ok = object["ok"] as? Bool, ok == false {
            let error = object["error"] as? AgentGridJSONObject
            let message = error?["message"] as? String ?? "AgentGrid API returned ok=false."
            throw AgentGridMobileError.apiError(message)
        }
        return object
    }

    private func endpointURL(_ path: String) throws -> URL {
        if let url = URL(string: path), url.scheme != nil {
            return url
        }
        let base = hubURL.absoluteString.hasSuffix("/")
            ? hubURL.absoluteString
            : "\(hubURL.absoluteString)/"
        let trimmedPath = path.drop(while: { $0 == "/" })
        guard let url = URL(string: base + trimmedPath) else {
            throw AgentGridMobileError.invalidURL(path)
        }
        return url
    }

    private static func pathComponent(_ value: String) -> String {
        value.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? value
    }
}
