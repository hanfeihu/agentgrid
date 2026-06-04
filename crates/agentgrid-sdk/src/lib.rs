use anyhow::{Context, Result};
use serde_json::{json, Value};

pub const DEFAULT_HUB_URL: &str = "http://127.0.0.1:20181";

#[derive(Clone)]
pub struct AgentGridClient {
    base_url: String,
    http: reqwest::blocking::Client,
}

impl AgentGridClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            http: reqwest::blocking::Client::new(),
        }
    }

    pub fn default_hub() -> Self {
        Self::new(DEFAULT_HUB_URL)
    }

    pub fn health(&self) -> Result<Value> {
        self.get("/api/health")
    }

    pub fn nodes(&self) -> Result<Value> {
        self.get("/api/nodes")
    }

    pub fn tools(&self) -> Result<Value> {
        self.get("/api/tools")
    }

    pub fn tool_probe_center(&self) -> Result<Value> {
        self.get("/api/tools/probe-center")
    }

    pub fn tool_probes(&self) -> Result<Value> {
        self.get("/api/tools/probes")
    }

    pub fn probe_tool(&self, tool_id: Option<&str>, node_id: Option<&str>) -> Result<Value> {
        match (tool_id, node_id) {
            (Some(tool_id), Some(node_id)) => self.post(
                &format!("/api/tools/{tool_id}/nodes/{node_id}/probe"),
                json!({}),
            ),
            (Some(tool_id), None) => self.post(&format!("/api/tools/{tool_id}/probe"), json!({})),
            (None, None) => self.post("/api/tools/probe", json!({})),
            (None, Some(_)) => anyhow::bail!("node_id requires tool_id"),
        }
    }

    pub fn runtime_standard(&self) -> Result<Value> {
        self.get("/api/runtime-standard")
    }

    pub fn mobile_sdk_standard(&self) -> Result<Value> {
        self.get("/api/runtime-standard/mobile-sdk")
    }

    pub fn workbenches(&self) -> Result<Value> {
        self.get("/api/workbenches")
    }

    pub fn workbench(&self, workbench_id: &str) -> Result<Value> {
        self.get(&format!("/api/workbenches/{workbench_id}"))
    }

    pub fn workbench_timeline(&self, workbench_id: &str) -> Result<Value> {
        self.get(&format!("/api/workbenches/{workbench_id}/timeline"))
    }

    pub fn workbench_action(&self, workbench_id: &str, request: Value) -> Result<Value> {
        self.post(&format!("/api/workbenches/{workbench_id}/actions"), request)
    }

    pub fn devices(&self) -> Result<Value> {
        self.get("/api/runtime-standard/devices")
    }

    pub fn evidence_standard(&self) -> Result<Value> {
        self.get("/api/runtime-standard/evidence")
    }

    pub fn runtime_manifest(&self) -> Result<Value> {
        self.get("/api/agent-runtime/manifest")
    }

    pub fn submit_runtime_task(&self, request: Value) -> Result<Value> {
        self.post("/api/agent-runtime/tasks", request)
    }

    pub fn get_runtime_task(&self, task_id: &str) -> Result<Value> {
        self.get(&format!("/api/agent-runtime/tasks/{task_id}"))
    }

    pub fn task_events(&self, task_id: &str) -> Result<Value> {
        self.get(&format!("/api/agent-runtime/tasks/{task_id}/events"))
    }

    pub fn execution_record(&self, task_id: &str) -> Result<Value> {
        self.get(&format!("/api/execution-records/tasks/{task_id}"))
    }

    pub fn artifacts(&self) -> Result<Value> {
        self.get("/api/artifacts")
    }

    pub fn artifact_download_url(&self, artifact_id: &str) -> String {
        format!("{}/api/artifacts/{artifact_id}/download", self.base_url)
    }

    pub fn task_templates(&self) -> Result<Value> {
        self.get("/api/task-templates")
    }

    pub fn start_task_template(&self, template_id: &str, request: Value) -> Result<Value> {
        self.post(&format!("/api/task-templates/{template_id}/start"), request)
    }

    pub fn webhooks(&self) -> Result<Value> {
        self.get("/api/webhooks")
    }

    pub fn create_webhook(&self, request: Value) -> Result<Value> {
        self.post("/api/webhooks", request)
    }

    pub fn webhook_deliveries(&self) -> Result<Value> {
        self.get("/api/webhooks/deliveries")
    }

    pub fn submit_command(
        &self,
        program: &str,
        args: Vec<String>,
        node_id: Option<String>,
        workbench_id: Option<String>,
        title: Option<String>,
    ) -> Result<Value> {
        let mut request = json!({
            "tool_id": "command.run",
            "title": title.unwrap_or_else(|| format!("command {program}")),
            "payload": {
                "type": "command",
                "program": program,
                "args": args,
                "working_dir": null,
                "timeout_seconds": 30
            },
            "verify": { "presets": ["command.exit_zero"] }
        });
        if let Some(node_id) = node_id {
            request["node_id"] = json!(node_id);
        }
        if let Some(workbench_id) = workbench_id {
            request["workbench_id"] = json!(workbench_id);
        }
        self.submit_runtime_task(request)
    }

    pub fn submit_plugin(
        &self,
        plugin_id: &str,
        action: &str,
        input: Value,
        node_id: Option<String>,
        workbench_id: Option<String>,
        title: Option<String>,
    ) -> Result<Value> {
        let mut request = json!({
            "tool_id": "plugin.run",
            "title": title.unwrap_or_else(|| format!("plugin {plugin_id}:{action}")),
            "payload": {
                "type": "plugin",
                "plugin_id": plugin_id,
                "action": action,
                "input": input,
                "timeout_seconds": 60
            },
            "verify": { "rules": [{ "path": "result.output", "op": "exists" }] }
        });
        if let Some(node_id) = node_id {
            request["node_id"] = json!(node_id);
        }
        if let Some(workbench_id) = workbench_id {
            request["workbench_id"] = json!(workbench_id);
        }
        self.submit_runtime_task(request)
    }

    fn get(&self, path: &str) -> Result<Value> {
        let response = self
            .http
            .get(format!("{}{}", self.base_url, path))
            .send()
            .with_context(|| format!("GET {path} failed"))?
            .error_for_status()?;
        Ok(response.json()?)
    }

    fn post(&self, path: &str, body: Value) -> Result<Value> {
        let response = self
            .http
            .post(format!("{}{}", self.base_url, path))
            .json(&body)
            .send()
            .with_context(|| format!("POST {path} failed"))?
            .error_for_status()?;
        Ok(response.json()?)
    }
}
