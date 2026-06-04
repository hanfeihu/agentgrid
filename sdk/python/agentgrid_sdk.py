import json
import urllib.request
import urllib.parse


DEFAULT_HUB_URL = "http://127.0.0.1:20181"


class AgentGridClient:
    def __init__(self, hub_url=DEFAULT_HUB_URL):
        self.hub_url = hub_url.rstrip("/")

    def runtime_manifest(self):
        return self._get("/api/agent-runtime/manifest")

    def tools(self):
        return self._get("/api/tools")

    def nodes(self):
        return self._get("/api/nodes")

    def runtime_standard(self):
        return self._get("/api/runtime-standard")

    def mobile_sdk_standard(self):
        return self._get("/api/runtime-standard/mobile-sdk")

    def workbenches(self):
        return self._get("/api/workbenches")

    def workbench(self, workbench_id):
        quoted = urllib.parse.quote(workbench_id, safe="")
        return self._get(f"/api/workbenches/{quoted}")

    def workbench_timeline(self, workbench_id):
        quoted = urllib.parse.quote(workbench_id, safe="")
        return self._get(f"/api/workbenches/{quoted}/timeline")

    def workbench_action(self, workbench_id, request):
        quoted = urllib.parse.quote(workbench_id, safe="")
        return self._post(f"/api/workbenches/{quoted}/actions", request)

    def devices(self):
        return self._get("/api/runtime-standard/devices")

    def evidence_standard(self):
        return self._get("/api/runtime-standard/evidence")

    def submit_task(self, request):
        return self._post("/api/agent-runtime/tasks", request)

    def get_task(self, task_id):
        return self._get(f"/api/agent-runtime/tasks/{task_id}")

    def task_events(self, task_id):
        return self._get(f"/api/agent-runtime/tasks/{task_id}/events")

    def execution_record(self, task_id):
        return self._get(f"/api/execution-records/tasks/{task_id}")

    def artifacts(self):
        return self._get("/api/artifacts")

    def artifact_download_url(self, artifact_id):
        return f"{self.hub_url}/api/artifacts/{artifact_id}/download"

    def task_templates(self):
        return self._get("/api/task-templates")

    def start_task_template(self, template_id, request=None):
        return self._post(f"/api/task-templates/{template_id}/start", request or {})

    def webhooks(self):
        return self._get("/api/webhooks")

    def create_webhook(self, request):
        return self._post("/api/webhooks", request)

    def webhook_deliveries(self):
        return self._get("/api/webhooks/deliveries")

    def run_command(self, program, args=None, node_id=None, workbench_id=None, title=None):
        request = {
            "tool_id": "command.run",
            "title": title or f"command {program}",
            "node_id": node_id,
            "workbench_id": workbench_id,
            "payload": {
                "type": "command",
                "program": program,
                "args": args or [],
                "working_dir": None,
                "timeout_seconds": 30,
            },
            "verify": {"presets": ["command.exit_zero"]},
        }
        return self.submit_task(request)

    def run_plugin(self, plugin_id, action="run", input=None, node_id=None, workbench_id=None, title=None):
        request = {
            "tool_id": "plugin.run",
            "title": title or f"plugin {plugin_id}:{action}",
            "node_id": node_id,
            "workbench_id": workbench_id,
            "payload": {
                "type": "plugin",
                "plugin_id": plugin_id,
                "action": action,
                "input": input or {},
                "timeout_seconds": 60,
            },
            "verify": {"rules": [{"path": "result.output", "op": "exists"}]},
        }
        return self.submit_task(request)

    def _get(self, path):
        with urllib.request.urlopen(f"{self.hub_url}{path}", timeout=30) as response:
            return json.loads(response.read().decode("utf-8"))

    def _post(self, path, body):
        data = json.dumps(body).encode("utf-8")
        request = urllib.request.Request(
            f"{self.hub_url}{path}",
            data=data,
            headers={"content-type": "application/json"},
            method="POST",
        )
        with urllib.request.urlopen(request, timeout=30) as response:
            return json.loads(response.read().decode("utf-8"))
