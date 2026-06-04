package io.agentgrid.mobile

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.io.OutputStreamWriter
import java.net.HttpURLConnection
import java.net.URLEncoder
import java.net.URL

class AgentGridApiException(message: String) : Exception(message)

class AgentGridMobileClient(
    hubUrl: String = DEFAULT_HUB_URL,
    private val bearerToken: String? = null,
) {
    private val baseUrl = hubUrl.trimEnd('/')

    suspend fun health(): JSONObject = get("/api/health")

    suspend fun runtimeStandard(): JSONObject = get("/api/runtime-standard")

    suspend fun mobileSdkStandard(): JSONObject = get("/api/runtime-standard/mobile-sdk")

    suspend fun workbenches(): JSONObject = get("/api/workbenches")

    suspend fun workbench(workbenchId: String): JSONObject =
        get("/api/workbenches/${urlEncode(workbenchId)}")

    suspend fun workbenchTimeline(workbenchId: String): JSONObject =
        get("/api/workbenches/${urlEncode(workbenchId)}/timeline")

    suspend fun devices(): JSONObject = get("/api/runtime-standard/devices")

    suspend fun evidenceStandard(): JSONObject = get("/api/runtime-standard/evidence")

    suspend fun nodes(): JSONObject = get("/api/nodes")

    suspend fun tools(): JSONObject = get("/api/tools")

    suspend fun localServices(): JSONObject = get("/api/local-services")

    suspend fun createBridgeSession(
        nodeId: String,
        serviceId: String = "codex.local",
    ): JSONObject = post(
        "/api/bridge-sessions",
        JSONObject()
            .put("node_id", nodeId)
            .put("service_id", serviceId),
    )

    fun bridgeWebSocketUrl(sessionId: String, token: String? = null): String {
        val wsBase = when {
            baseUrl.startsWith("https://") -> "wss://${baseUrl.removePrefix("https://")}"
            baseUrl.startsWith("http://") -> "ws://${baseUrl.removePrefix("http://")}"
            else -> baseUrl
        }
        val suffix = token
            ?.let { "?token=${URLEncoder.encode(it, Charsets.UTF_8.name())}" }
            .orEmpty()
        return "$wsBase/api/bridge-sessions/$sessionId/ws$suffix"
    }

    suspend fun listPortBridges(): JSONObject = get("/api/port-bridges")

    suspend fun createPortBridge(
        sourceNodeId: String,
        targetNodeId: String,
        targetPort: Int,
        sourceBindPort: Int = 0,
        targetHost: String = "127.0.0.1",
        sourceBindHost: String = "127.0.0.1",
        ttlSeconds: Int = 1800,
        purpose: String? = null,
        createdBy: String = "agentgrid-mobile-sdk",
    ): JSONObject {
        val body = JSONObject()
            .put("source_node_id", sourceNodeId)
            .put("target_node_id", targetNodeId)
            .put("source_bind_host", sourceBindHost)
            .put("source_bind_port", sourceBindPort)
            .put("target_host", targetHost)
            .put("target_port", targetPort)
            .put("protocol", "tcp")
            .put("ttl_seconds", ttlSeconds)
            .put("created_by", createdBy)
        purpose?.let { body.put("purpose", it) }
        return post("/api/port-bridges", body)
    }

    suspend fun getPortBridge(portBridgeId: String): JSONObject =
        get("/api/port-bridges/$portBridgeId")

    suspend fun closePortBridge(portBridgeId: String): JSONObject =
        delete("/api/port-bridges/$portBridgeId")

    suspend fun submitTask(request: JSONObject): JSONObject =
        post("/api/agent-runtime/tasks", request)

    suspend fun runCommand(
        program: String,
        args: List<String> = emptyList(),
        nodeId: String? = null,
        workbenchId: String? = null,
        title: String? = null,
    ): JSONObject {
        val payload = JSONObject()
            .put("type", "command")
            .put("program", program)
            .put("args", org.json.JSONArray(args))
            .put("working_dir", JSONObject.NULL)
            .put("timeout_seconds", 30)
        val body = JSONObject()
            .put("tool_id", "command.run")
            .put("title", title ?: "command $program")
            .put("payload", payload)
            .put("verify", JSONObject().put("presets", org.json.JSONArray(listOf("command.exit_zero"))))
        nodeId?.let { body.put("node_id", it) }
        workbenchId?.let { body.put("workbench_id", it) }
        return submitTask(body)
    }

    suspend fun runPlugin(
        pluginId: String,
        action: String = "run",
        input: JSONObject = JSONObject(),
        nodeId: String? = null,
        workbenchId: String? = null,
        title: String? = null,
    ): JSONObject {
        val payload = JSONObject()
            .put("type", "plugin")
            .put("plugin_id", pluginId)
            .put("action", action)
            .put("input", input)
            .put("timeout_seconds", 60)
        val body = JSONObject()
            .put("tool_id", "plugin.run")
            .put("title", title ?: "plugin $pluginId:$action")
            .put("payload", payload)
            .put(
                "verify",
                JSONObject().put(
                    "rules",
                    org.json.JSONArray(
                        listOf(JSONObject().put("path", "result.output").put("op", "exists")),
                    ),
                ),
            )
        nodeId?.let { body.put("node_id", it) }
        workbenchId?.let { body.put("workbench_id", it) }
        return submitTask(body)
    }

    suspend fun getTask(taskId: String): JSONObject =
        get("/api/agent-runtime/tasks/$taskId")

    suspend fun taskEvents(taskId: String): JSONObject =
        get("/api/agent-runtime/tasks/$taskId/events")

    suspend fun executionRecord(taskId: String): JSONObject =
        get("/api/execution-records/tasks/$taskId")

    suspend fun artifacts(): JSONObject = get("/api/artifacts")

    fun artifactDownloadUrl(artifactId: String): String =
        endpointUrl("/api/artifacts/$artifactId/download").toString()

    suspend fun taskTemplates(): JSONObject = get("/api/task-templates")

    suspend fun startTaskTemplate(
        templateId: String,
        request: JSONObject = JSONObject(),
    ): JSONObject = post("/api/task-templates/$templateId/start", request)

    suspend fun get(path: String): JSONObject =
        request(path = path, method = "GET", body = null)

    suspend fun post(path: String, body: JSONObject): JSONObject =
        request(path = path, method = "POST", body = body)

    suspend fun delete(path: String): JSONObject =
        request(path = path, method = "DELETE", body = null)

    private suspend fun request(
        path: String,
        method: String,
        body: JSONObject?,
    ): JSONObject = withContext(Dispatchers.IO) {
        val connection = (endpointUrl(path).openConnection() as HttpURLConnection).apply {
            requestMethod = method
            connectTimeout = 30_000
            readTimeout = 30_000
            setRequestProperty("accept", "application/json")
            bearerToken?.let { setRequestProperty("authorization", "Bearer $it") }
            if (body != null) {
                doOutput = true
                setRequestProperty("content-type", "application/json")
            }
        }

        try {
            if (body != null) {
                OutputStreamWriter(connection.outputStream, Charsets.UTF_8).use { writer ->
                    writer.write(body.toString())
                }
            }

            val status = connection.responseCode
            val stream = if (status in 200..299) connection.inputStream else connection.errorStream
            val text = stream?.bufferedReader(Charsets.UTF_8)?.use { it.readText() }.orEmpty()
            if (status !in 200..299) {
                throw AgentGridApiException("AgentGrid HTTP error $status: $text")
            }

            val json = JSONObject(text)
            if (json.optBoolean("ok", true).not()) {
                val message = json.optJSONObject("error")?.optString("message")
                    ?: "AgentGrid API returned ok=false."
                throw AgentGridApiException(message)
            }
            json
        } finally {
            connection.disconnect()
        }
    }

    companion object {
        const val DEFAULT_HUB_URL = "http://chenqi.tminos.com:20080/agentgrid"
    }

    private fun endpointUrl(path: String): URL {
        if (path.startsWith("http://") || path.startsWith("https://")) {
            return URL(path)
        }
        return URL("$baseUrl/${path.trimStart('/')}")
    }

    private fun urlEncode(value: String): String =
        URLEncoder.encode(value, Charsets.UTF_8.name())
}
