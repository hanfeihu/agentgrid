export function percent(value, total) {
  if (!total) return 0;
  return Math.max(0, Math.min(100, Math.round((value / total) * 100)));
}

export function round(value) {
  return Math.max(0, Math.min(100, Math.round(value || 0)));
}

export function formatMb(value) {
  const mb = Number(value || 0);
  if (mb >= 1024 * 1024) return `${(mb / 1024 / 1024).toFixed(1)} TB`;
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`;
  return `${Math.round(mb)} MB`;
}

export function diskUsedPercent(node) {
  return 100 - percent(node.spec.disk_free_mb, node.spec.disk_total_mb);
}

export function taskStateProgress(value) {
  return {
    assigned: 18,
    todo: 24,
    ready: 24,
    in_progress: 64,
    stopping: 72,
    review: 84,
    done: 100,
    failed: 100,
    cancelled: 100,
    stopped: 100,
  }[value] || 0;
}

export function latestByTime(items, picker) {
  return [...(items || [])].sort((left, right) => {
    const leftTime = new Date(picker(left) || 0).getTime();
    const rightTime = new Date(picker(right) || 0).getTime();
    return rightTime - leftTime;
  })[0] || null;
}

export function resultText(value) {
  if (!value) return '-';
  if (typeof value === 'string') return value;
  if (value.text) return value.text;
  if (value.body) return typeof value.body === 'string' ? value.body : JSON.stringify(value.body, null, 2);
  return JSON.stringify(value, null, 2);
}

export function parseTaskInputPayload(task) {
  const raw = task?.spec?.inputs?.[0];
  if (!raw) return null;
  try {
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

export function taskOperationLabel(payload) {
  if (!payload) return '-';
  if (payload.type === 'desktop') return `desktop.${payload.operation || 'unknown'}`;
  if (payload.type === 'command') return `command.run ${payload.program || ''}`.trim();
  if (payload.type === 'file') return `file.${payload.operation || 'unknown'}`;
  if (payload.type === 'http_request') return `${payload.method || 'GET'} ${payload.url || ''}`.trim();
  return payload.type || '-';
}

export function desktopOperationLabel(value) {
  return {
    screenshot: '截图',
    click: '点击',
    type_text: '输入文本',
    key: '按键',
    event: '事件',
    result: '结果',
  }[value] || value || '-';
}

export function buildDesktopTimeline({ task, inputPayload, result, error, artifacts, events }) {
  const isDesktop = inputPayload?.type === 'desktop' || result?.type === 'desktop_result';
  if (!isDesktop) return [];
  const rows = [];
  if (inputPayload?.type === 'desktop') {
    rows.push({
      id: `${task.metadata.id}-input`,
      time: task.metadata.created_at,
      kind: inputPayload.operation || 'event',
      node: task.status.leased_by_node_id,
      summary: desktopInputSummary(inputPayload),
      raw: inputPayload,
    });
  }
  for (const event of events || []) {
    if (!String(event.spec?.type || '').startsWith('task.')) continue;
    rows.push({
      id: event.metadata.id,
      time: event.metadata.created_at,
      kind: 'event',
      node: task.status.leased_by_node_id,
      summary: event.spec.summary || event.spec.type,
      raw: event.spec.payload || event.spec,
    });
  }
  for (const artifact of artifacts || []) {
    if (!isImageArtifact(artifact)) continue;
    rows.push({
      id: artifact.metadata.id,
      time: artifact.metadata.created_at,
      kind: 'screenshot',
      node: artifact.spec.node_id,
      summary: `${artifact.spec.name} · ${formatBytes(artifact.spec.size_bytes)}`,
      artifact,
      raw: artifact.spec.metadata || artifact.spec,
    });
  }
  if (result) {
    rows.push({
      id: `${task.metadata.id}-result`,
      time: task.status.completed_at || task.metadata.updated_at,
      kind: result.operation || 'result',
      node: task.status.leased_by_node_id,
      summary: result.message || '桌面任务已完成',
      raw: result,
    });
  } else if (error) {
    rows.push({
      id: `${task.metadata.id}-error`,
      time: task.metadata.updated_at,
      kind: 'result',
      node: task.status.leased_by_node_id,
      summary: error.message || '桌面任务失败',
      raw: error,
    });
  }
  return rows.sort((left, right) => new Date(left.time || 0) - new Date(right.time || 0));
}

export function desktopInputSummary(payload) {
  if (!payload) return '-';
  if (payload.operation === 'screenshot') return `截取当前屏幕${payload.path ? `，保存到 ${payload.path}` : ''}`;
  if (payload.operation === 'click') return `点击坐标 (${payload.x}, ${payload.y})，按钮 ${payload.button || 'left'}`;
  if (payload.operation === 'type_text') return `向前台窗口输入 ${String(payload.text || '').length} 个字符`;
  if (payload.operation === 'key') return `发送按键 ${[...(payload.modifiers || []), payload.key].join('+')}`;
  return JSON.stringify(payload);
}

export function compactJson(value) {
  if (value === undefined || value === null) return '-';
  if (typeof value === 'string') return value.length > 80 ? `${value.slice(0, 80)}...` : value;
  return JSON.stringify(value);
}

export function verificationLabel(value) {
  return {
    passed: '通过',
    failed: '失败',
    skipped: '跳过',
  }[value] || '未配置';
}

export function verificationColor(value) {
  return {
    passed: 'green',
    failed: 'red',
    skipped: 'default',
  }[value] || 'default';
}

export function isImageArtifact(artifact) {
  return Boolean(
    artifact?.spec?.content_base64 &&
      (artifact.spec.content_type || '').startsWith('image/')
  );
}

export function previewKind(artifact) {
  if ((artifact?.spec?.content_type || '').startsWith('image/')) return 'image';
  if ((artifact?.spec?.content_type || '').startsWith('text/')) return 'text';
  return 'download';
}

export function shortHash(value) {
  if (!value) return '-';
  return `${value.slice(0, 10)}...${value.slice(-6)}`;
}

export function artifactDataUrl(artifact) {
  return `data:${artifact.spec.content_type};base64,${artifact.spec.content_base64}`;
}

export function artifactTypeLabel(value) {
  return {
    file: '文件',
    log: '日志',
    screenshot: '截图',
    browser_text: '页面文本',
  }[value] || value || '-';
}

export function workbenchLabel(value) {
  return {
    hardware_bench: '硬件工位',
    desktop_bench: '桌面工位',
    compute_bench: '计算工位',
    all: '全部',
  }[value] || value || '-';
}

export function workbenchColor(value) {
  return {
    hardware_bench: 'volcano',
    desktop_bench: 'blue',
    compute_bench: 'green',
  }[value] || 'default';
}

export function deviceLabel(value) {
  return {
    desktop: '桌面',
    browser: '浏览器',
    filesystem: '文件系统',
    plugin_runtime: '插件运行时',
    serial: '串口',
    flasher: '烧录器',
    test_rig: '测试工装',
  }[value] || value || '-';
}

export function evidenceLabel(value) {
  return {
    screenshot: '截图',
    stdout_log: 'stdout 日志',
    stderr_log: 'stderr 日志',
    serial_log: '串口日志',
    file_artifact: '文件产物',
    test_report: '测试报告',
    operation_timeline: '操作时间线',
    page_text: '页面文本',
    downloaded_file: '下载文件',
    plugin_result: '插件结果',
    directory_listing: '目录列表',
  }[value] || value || '-';
}

export function riskLabel(value) {
  return {
    high: '高',
    medium: '中',
    low: '低',
  }[value] || value || '-';
}

export function riskColor(value) {
  return {
    high: 'red',
    medium: 'orange',
    low: 'green',
  }[value] || 'default';
}

export function roleLabel(value) {
  return {
    super_admin: '超级管理员',
    admin: '管理员',
    member: '成员',
  }[value] || value || '-';
}

export function roleColor(value) {
  return {
    super_admin: 'red',
    admin: 'blue',
    member: 'default',
  }[value] || 'default';
}

export function userStatusLabel(value) {
  return {
    active: '启用',
    disabled: '停用',
    pending: '待验证',
  }[value] || value || '-';
}

export function probeStateLabel(value) {
  return {
    verified: '已验证',
    failed: '失败',
    pending: '等待',
    unsupported: '不支持',
    expired: '过期',
    declared_unverified: '未验证',
  }[value] || value || '未验证';
}

export function probeStateColor(value) {
  return {
    verified: 'green',
    failed: 'red',
    pending: 'blue',
    unsupported: 'default',
    expired: 'orange',
    declared_unverified: 'default',
  }[value] || 'default';
}

export function formatBytes(value) {
  const bytes = Number(value || 0);
  if (bytes >= 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(2)} MB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${bytes} B`;
}

export function stateLabel(value) {
  return {
    online: '在线',
    unknown: '未知',
    offline: '离线',
    draft: '草稿',
    running: '运行中',
    pending: '等待依赖',
    ready: '等待调度',
    assigned: '已分配',
    todo: '待处理',
    in_progress: '执行中',
    review: '待审查',
    done: '完成',
    failed: '失败',
    cancelled: '已取消',
    stopping: '停止中',
    stopped: '已停止',
    blocked: '阻塞',
  }[value] || value || '-';
}

export function nodeAuthLabel(value) {
  return {
    legacy: '旧节点兼容',
    pending: '待管理员授权',
    bound: '已绑定',
    rejected: '已拒绝',
  }[value] || value || '-';
}

export function workflowStateColor(value) {
  return {
    draft: 'default',
    pending: 'default',
    ready: 'blue',
    running: 'processing',
    in_progress: 'processing',
    done: 'green',
    failed: 'red',
    cancelled: 'orange',
    stopped: 'orange',
  }[value] || 'default';
}

export function formatTime(value) {
  if (!value) return '-';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString('zh-CN', { hour12: false });
}

export function splitList(value) {
  if (Array.isArray(value)) return value;
  return String(value || '')
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean);
}

export function parseJsonOrDefault(value, fallback) {
  try {
    if (!value) return fallback;
    return JSON.parse(value);
  } catch (_) {
    return fallback;
  }
}

export function taskType(task) {
  const labels = task.spec.labels || [];
  return labels.find((label) => ['http_request', 'command', 'file', 'git', 'docker', 'browser', 'session', 'agentmessage'].includes(label)) || '-';
}

export function routeNode(task) {
  return (task.spec.labels || []).find((label) => label.startsWith('node:'))?.slice(5);
}

export function routeOs(task) {
  return (task.spec.labels || []).find((label) => label.startsWith('os:'))?.slice(3);
}
