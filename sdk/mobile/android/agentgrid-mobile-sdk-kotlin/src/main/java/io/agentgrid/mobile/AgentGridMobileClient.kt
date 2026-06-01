package io.agentgrid.mobile

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.io.OutputStreamWriter
import java.net.HttpURLConnection
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

    suspend fun workbenches(): JSONObject = get("/api/runtime-standard/workbench")

    suspend fun devices(): JSONObject = get("/api/runtime-standard/devices")

    suspend fun evidenceStandard(): JSONObject = get("/api/runtime-standard/evidence")

    suspend fun nodes(): JSONObject = get("/api/nodes")

    suspend fun tools(): JSONObject = get("/api/tools")

    suspend fun submitTask(request: JSONObject): JSONObject =
        post("/api/agent-runtime/tasks", request)

    suspend fun getTask(taskId: String): JSONObject =
        get("/api/agent-runtime/tasks/$taskId")

    suspend fun taskEvents(taskId: String): JSONObject =
        get("/api/agent-runtime/tasks/$taskId/events")

    suspend fun executionRecord(taskId: String): JSONObject =
        get("/api/execution-records/tasks/$taskId")

    suspend fun artifacts(): JSONObject = get("/api/artifacts")

    fun artifactDownloadUrl(artifactId: String): String =
        "$baseUrl/api/artifacts/$artifactId/download"

    suspend fun taskTemplates(): JSONObject = get("/api/task-templates")

    suspend fun startTaskTemplate(
        templateId: String,
        request: JSONObject = JSONObject(),
    ): JSONObject = post("/api/task-templates/$templateId/start", request)

    private suspend fun get(path: String): JSONObject =
        request(path = path, method = "GET", body = null)

    private suspend fun post(path: String, body: JSONObject): JSONObject =
        request(path = path, method = "POST", body = body)

    private suspend fun request(
        path: String,
        method: String,
        body: JSONObject?,
    ): JSONObject = withContext(Dispatchers.IO) {
        val connection = (URL("$baseUrl$path").openConnection() as HttpURLConnection).apply {
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
}

