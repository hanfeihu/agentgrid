package io.agentgrid.mobile

import org.junit.Assert.assertEquals
import org.junit.Test

class AgentGridMobileClientTest {
    @Test
    fun bridgeWebSocketUrlKeepsHubPathAndEncodesToken() {
        val client = AgentGridMobileClient("http://example.com/agentgrid/")

        val url = client.bridgeWebSocketUrl("session_123", "token value")

        assertEquals(
            "ws://example.com/agentgrid/api/bridge-sessions/session_123/ws?token=token+value",
            url,
        )
    }

    @Test
    fun bridgeWebSocketUrlUsesWssForHttpsHub() {
        val client = AgentGridMobileClient("https://hub.example.com/agentgrid")

        val url = client.bridgeWebSocketUrl("session_123")

        assertEquals(
            "wss://hub.example.com/agentgrid/api/bridge-sessions/session_123/ws",
            url,
        )
    }

    @Test
    fun artifactDownloadUrlKeepsHubPath() {
        val client = AgentGridMobileClient("http://example.com/agentgrid")

        assertEquals(
            "http://example.com/agentgrid/api/artifacts/artifact_123/download",
            client.artifactDownloadUrl("artifact_123"),
        )
    }
}
