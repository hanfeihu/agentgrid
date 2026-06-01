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
        try await get("/api/runtime-standard/workbench")
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

    public func submitTask(_ request: AgentGridJSONObject) async throws -> AgentGridJSONObject {
        try await post("/api/agent-runtime/tasks", body: request)
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
        hubURL.appendingPathComponent("api/artifacts/\(artifactID)/download")
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

    private func get(_ path: String) async throws -> AgentGridJSONObject {
        try await send(path: path, method: "GET", body: nil)
    }

    private func post(_ path: String, body: AgentGridJSONObject) async throws -> AgentGridJSONObject {
        try await send(path: path, method: "POST", body: body)
    }

    private func send(
        path: String,
        method: String,
        body: AgentGridJSONObject?
    ) async throws -> AgentGridJSONObject {
        guard let url = URL(string: path, relativeTo: hubURL)?.absoluteURL else {
            throw AgentGridMobileError.invalidURL(path)
        }

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
}
