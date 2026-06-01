export const DEFAULT_HUB_URL = 'http://127.0.0.1:20181';

export class AgentGridClient {
  constructor({ hubUrl = DEFAULT_HUB_URL, fetchImpl = fetch } = {}) {
    this.hubUrl = hubUrl.replace(/\/$/, '');
    this.fetch = fetchImpl;
  }

  async runtimeManifest() {
    return this.get('/api/agent-runtime/manifest');
  }

  async tools() {
    return this.get('/api/tools');
  }

  async nodes() {
    return this.get('/api/nodes');
  }

  async runtimeStandard() {
    return this.get('/api/runtime-standard');
  }

  async mobileSdkStandard() {
    return this.get('/api/runtime-standard/mobile-sdk');
  }

  async workbenches() {
    return this.get('/api/runtime-standard/workbench');
  }

  async devices() {
    return this.get('/api/runtime-standard/devices');
  }

  async evidenceStandard() {
    return this.get('/api/runtime-standard/evidence');
  }

  async submitTask(request) {
    return this.post('/api/agent-runtime/tasks', request);
  }

  async getTask(taskId) {
    return this.get(`/api/agent-runtime/tasks/${taskId}`);
  }

  async taskEvents(taskId) {
    return this.get(`/api/agent-runtime/tasks/${taskId}/events`);
  }

  async executionRecord(taskId) {
    return this.get(`/api/execution-records/tasks/${taskId}`);
  }

  async artifacts() {
    return this.get('/api/artifacts');
  }

  artifactDownloadUrl(artifactId) {
    return `${this.hubUrl}/api/artifacts/${artifactId}/download`;
  }

  async taskTemplates() {
    return this.get('/api/task-templates');
  }

  async startTaskTemplate(templateId, request = {}) {
    return this.post(`/api/task-templates/${templateId}/start`, request);
  }

  async webhooks() {
    return this.get('/api/webhooks');
  }

  async createWebhook(request) {
    return this.post('/api/webhooks', request);
  }

  async webhookDeliveries() {
    return this.get('/api/webhooks/deliveries');
  }

  async runCommand({ program, args = [], nodeId, title }) {
    return this.submitTask({
      tool_id: 'command.run',
      title: title || `command ${program}`,
      node_id: nodeId,
      payload: { type: 'command', program, args, working_dir: null, timeout_seconds: 30 },
      verify: { presets: ['command.exit_zero'] },
    });
  }

  async runPlugin({ pluginId, action = 'run', input = {}, nodeId, title }) {
    return this.submitTask({
      tool_id: 'plugin.run',
      title: title || `plugin ${pluginId}:${action}`,
      node_id: nodeId,
      payload: {
        type: 'plugin',
        plugin_id: pluginId,
        action,
        input,
        timeout_seconds: 60,
      },
      verify: { rules: [{ path: 'result.output', op: 'exists' }] },
    });
  }

  async get(path) {
    const response = await this.fetch(`${this.hubUrl}${path}`);
    return this.read(response);
  }

  async post(path, body) {
    const response = await this.fetch(`${this.hubUrl}${path}`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(body),
    });
    return this.read(response);
  }

  async read(response) {
    const data = await response.json();
    if (!response.ok || data.ok === false) {
      throw new Error(data.error?.message || response.statusText);
    }
    return data;
  }
}
