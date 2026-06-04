import React, { useEffect, useMemo, useState } from 'react';
import { createRoot } from 'react-dom/client';
import {
  ApiOutlined,
  AuditOutlined,
  CheckCircleOutlined,
  CloudServerOutlined,
  DashboardOutlined,
  DatabaseOutlined,
  CodeOutlined,
  DisconnectOutlined,
  FileTextOutlined,
  DownloadOutlined,
  ForkOutlined,
  DeploymentUnitOutlined,
  NodeIndexOutlined,
  ReloadOutlined,
  SettingOutlined,
  SendOutlined,
  TeamOutlined,
  ToolOutlined,
  LinkOutlined,
  MobileOutlined,
  ProfileOutlined,
  SafetyCertificateOutlined,
} from '@ant-design/icons';
import {
  Alert,
  Badge,
  Button,
  Card,
  Col,
  ConfigProvider,
  Form,
  Input,
  Modal,
  InputNumber,
  Drawer,
  Tabs,
  Select,
  Progress,
  Row,
  Space,
  Steps,
  Statistic,
  Table,
  Tag,
  Typography,
  message,
  theme,
} from 'antd';
import { PageContainer, ProCard, ProLayout } from '@ant-design/pro-components';
import 'antd/dist/reset.css';
import './style.css';
import {
  apiBase,
  artifactDownloadUrl,
  clearStoredAuth,
  fetchJson,
  fetchOptionalJson,
  loadStoredAuth,
  saveStoredAuth,
} from './api';
import {
  artifactDataUrl,
  artifactTypeLabel,
  buildDesktopTimeline,
  compactJson,
  desktopOperationLabel,
  deviceLabel,
  diskUsedPercent,
  evidenceLabel,
  formatBytes,
  formatMb,
  formatTime,
  isImageArtifact,
  latestByTime,
  nodeAuthLabel,
  parseJsonOrDefault,
  parseTaskInputPayload,
  percent,
  previewKind,
  probeStateColor,
  probeStateLabel,
  resultText,
  riskColor,
  riskLabel,
  roleColor,
  roleLabel,
  round,
  routeNode,
  routeOs,
  shortHash,
  splitList,
  stateLabel,
  taskOperationLabel,
  taskStateProgress,
  taskType,
  userStatusLabel,
  verificationColor,
  verificationLabel,
  workbenchColor,
  workbenchLabel,
  workflowStateColor,
} from './format';

const { Title, Text } = Typography;

const menuRoutes = [
  { path: '/overview', key: 'overview', name: '集群总览', icon: <DashboardOutlined /> },
  { path: '/nodes', key: 'nodes', name: '电脑管理', icon: <CloudServerOutlined /> },
  { path: '/capabilities', key: 'capabilities', name: '能力清单', icon: <ApiOutlined /> },
  { path: '/tools', key: 'tools', name: '工具目录', icon: <ToolOutlined /> },
  { path: '/node-tools', key: 'nodeTools', name: '节点工具', icon: <DeploymentUnitOutlined /> },
  { path: '/runtime', key: 'runtime', name: 'AI 接入层', icon: <ApiOutlined /> },
  { path: '/standard', key: 'standard', name: '运行标准', icon: <ProfileOutlined /> },
  { path: '/tasks', key: 'tasks', name: '任务中心', icon: <DatabaseOutlined /> },
  { path: '/jobs', key: 'jobs', name: '集群 Job', icon: <DeploymentUnitOutlined /> },
  { path: '/workflows', key: 'workflows', name: '工作流', icon: <ForkOutlined /> },
  { path: '/queue', key: 'queue', name: '任务队列', icon: <NodeIndexOutlined /> },
  { path: '/results', key: 'results', name: '任务结果', icon: <CodeOutlined /> },
  { path: '/records', key: 'records', name: '执行档案', icon: <ProfileOutlined /> },
  { path: '/artifacts', key: 'artifacts', name: '任务产物', icon: <DownloadOutlined /> },
  { path: '/terminal', key: 'terminal', name: '远程终端', icon: <CodeOutlined /> },
  { path: '/scheduler', key: 'scheduler', name: '调度策略', icon: <SettingOutlined /> },
  { path: '/settings', key: 'settings', name: '系统设置', icon: <SafetyCertificateOutlined /> },
  { path: '/users', key: 'users', name: '用户管理', icon: <TeamOutlined /> },
  { path: '/provisioning', key: 'provisioning', name: '节点纳管', icon: <DeploymentUnitOutlined /> },
  { path: '/workflow-templates', key: 'workflowTemplates', name: '工作流模板', icon: <ForkOutlined /> },
  { path: '/events', key: 'events', name: '事件总线', icon: <AuditOutlined /> },
  { path: '/templates', key: 'templates', name: '任务模板', icon: <ToolOutlined /> },
  { path: '/webhooks', key: 'webhooks', name: '任务回调', icon: <LinkOutlined /> },
  { path: '/diagnostics', key: 'diagnostics', name: '运行诊断', icon: <AuditOutlined /> },
  { path: '/audit', key: 'audit', name: '审计日志', icon: <AuditOutlined /> },
  { path: '/docs', key: 'docs', name: '命令文档', icon: <FileTextOutlined /> },
  { path: '/agents', key: 'agents', name: 'AI 员工', icon: <TeamOutlined /> },
  { path: '/submit', key: 'submit', name: '提交 HTTP', icon: <SendOutlined /> },
  { path: '/command', key: 'command', name: '下发命令', icon: <CodeOutlined /> },
];

const pageNames = Object.fromEntries(menuRoutes.map((item) => [item.key, item.name]));

function nodeChannelRole(node) {
  const explicit = node?.spec?.channel_role;
  if (explicit) return explicit;
  const capabilities = node?.spec?.capabilities || [];
  if ((node?.metadata?.id || '').endsWith('-desktop') || capabilities.includes('desktop')) return 'desktop';
  return 'worker';
}

function physicalHostKey(node) {
  return node?.spec?.physical_host_id
    || node?.spec?.machine_fingerprint
    || (node?.metadata?.id || '').replace(/-desktop$/, '');
}

function physicalHostName(node) {
  return (node?.metadata?.name || node?.metadata?.id || '-').replace(/\s+Desktop$/i, '').replace(/-desktop$/i, '');
}

function mergeUnique(values) {
  return Array.from(new Set(values.filter(Boolean)));
}

function groupPhysicalHosts(nodes) {
  return nodes.reduce((hosts, node) => {
    const key = physicalHostKey(node);
    const role = nodeChannelRole(node);
    const existing = hosts.get(key) || {
      id: key,
      name: physicalHostName(node),
      nodes: [],
      channels: {},
      capabilities: [],
      state: 'offline',
      primary: node,
    };
    existing.nodes.push(node);
    existing.channels[role] = node;
    existing.capabilities = mergeUnique([...existing.capabilities, ...(node.spec?.capabilities || [])]);
    if (role === 'worker' || !existing.primary || node.status?.state === 'online') {
      existing.primary = node;
    }
    if (existing.nodes.some((item) => item.status?.state === 'online')) existing.state = 'online';
    else if (existing.nodes.some((item) => item.status?.state === 'unknown')) existing.state = 'unknown';
    else existing.state = existing.nodes[0]?.status?.state || 'offline';
    hosts.set(key, existing);
    return hosts;
  }, new Map());
}

function normalizeWorkbenchRows(workbenches, nodes) {
  if (!workbenches?.length) return Array.from(groupPhysicalHosts(nodes).values());
  return workbenches.map((workbench) => {
    const channels = workbench.spec?.channels || {};
    const channelNodes = Object.values(channels).filter(Boolean);
    const primary = channels.worker
      || channelNodes.find((node) => node.status?.state === 'online')
      || channelNodes[0]
      || {};
    return {
      id: workbench.metadata?.id,
      name: workbench.metadata?.name || workbench.metadata?.id,
      state: workbench.status?.state || 'offline',
      nodes: channelNodes,
      channels,
      capabilities: workbench.spec?.capabilities || [],
      primary,
      resources: workbench.resources || {},
      workbench,
    };
  });
}

const channelRoles = ['worker', 'desktop', 'service', 'bridge', 'device'];

function channelLabel(role) {
  if (role === 'desktop') return '桌面通道';
  if (role === 'service') return '服务通道';
  if (role === 'bridge') return '桥接通道';
  if (role === 'device') return '设备通道';
  return '后台通道';
}

function channelDuty(role) {
  if (role === 'desktop') return '截图、点击、输入、按键、前台应用控制';
  if (role === 'service') return 'Codex、本地服务、WebSocket/SSE 服务桥接';
  if (role === 'bridge') return '节点到节点端口桥接和临时网络通道';
  if (role === 'device') return '串口、烧录、硬件工位、设备 SDK';
  return '命令、文件、插件、Git、Docker、软件安装';
}

function channelColor(role, node) {
  if (!node) return 'default';
  if (node.status?.state !== 'online') return 'default';
  return {
    worker: 'green',
    desktop: 'blue',
    service: 'cyan',
    bridge: 'purple',
    device: 'volcano',
  }[role] || 'green';
}

function ChannelStatus({ role, node }) {
  return (
    <Tag color={channelColor(role, node)}>
      {channelLabel(role)}：{node ? stateLabel(node.status?.state) : '未安装'}
    </Tag>
  );
}

function taskStateColor(state) {
  return {
    assigned: 'blue',
    todo: 'blue',
    in_progress: 'processing',
    stopping: 'orange',
    done: 'green',
    failed: 'red',
    cancelled: 'default',
    stopped: 'default',
    blocked: 'orange',
  }[state] || 'default';
}

function operationEventLabel(type) {
  if (type === 'task.created') return '已提交';
  if (type === 'task.leased') return '已分配';
  if (type === 'task.completed') return '已完成';
  if (type === 'task.failed') return '失败';
  if (type === 'task.done') return '结果';
  if (type === 'artifact.created') return '产物';
  return type || '事件';
}

function operationEventColor(type) {
  if (type === 'task.completed' || type === 'task.done') return 'green';
  if (type === 'task.failed') return 'red';
  if (type === 'task.leased') return 'blue';
  if (type === 'artifact.created') return 'purple';
  return 'default';
}

function workbenchResource(row, key) {
  return row.resources?.[key] ?? row.primary?.spec?.[key] ?? 0;
}

function workbenchOptions(workbenches, nodes) {
  return normalizeWorkbenchRows(workbenches, nodes).map((workbench) => ({
    value: workbench.id,
    label: `${workbench.name} / ${stateLabel(workbench.state)}`,
  }));
}

function workbenchActionPlacement(action, workbench) {
  const normalized = action || 'command';
  const role = normalized === 'screenshot' ? 'desktop' : 'worker';
  const node = workbench?.channels?.[role];
  return {
    role,
    node,
    title: role === 'desktop' ? 'Hub 会派到桌面通道' : 'Hub 会派到后台通道',
    reason: role === 'desktop'
      ? '截图、点击、输入、按键必须发生在用户前台桌面会话里。'
      : '命令、文件、工具运行属于后台执行，适合由普通 Worker 处理。',
    blocked: role === 'desktop'
      ? '不会派到后台通道，因为后台 Worker 通常没有真实用户桌面。'
      : '不会派到桌面通道，因为这个动作不需要前台桌面控制。',
  };
}

function BootstrapAdmin({ onDone }) {
  const [loading, setLoading] = useState(false);
  const submit = async (values) => {
    setLoading(true);
    try {
      const session = await fetchJson('/bootstrap/admin', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(values),
      });
      message.success('超级管理员已创建，请保存好账号密码');
      onDone(session);
    } catch (error) {
      message.error(`初始化失败：${error.message}`);
    } finally {
      setLoading(false);
    }
  };
  return (
    <Modal title="初始化 Hub 超级管理员" open footer={null} closable={false} width={560}>
      <Form layout="vertical" onFinish={submit}>
        <Form.Item name="email" label="管理员邮箱" rules={[{ required: true }]}><Input /></Form.Item>
        <Form.Item name="name" label="姓名" initialValue="超级管理员"><Input /></Form.Item>
        <Form.Item name="password" label="初始密码" rules={[{ required: true, min: 8 }]}><Input.Password /></Form.Item>
        <Button type="primary" htmlType="submit" loading={loading} block>创建唯一超级管理员</Button>
      </Form>
    </Modal>
  );
}

function LoginPanel({ onDone }) {
  const [mode, setMode] = useState('login');
  const [loading, setLoading] = useState(false);
  const sendCode = async (email) => {
    if (!email) {
      message.warning('先填写邮箱');
      return;
    }
    setLoading(true);
    try {
      await fetchJson('/auth/register/request-code', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ email }),
      });
      message.success('验证码已发送');
    } catch (error) {
      message.error(`发送失败：${error.message}`);
    } finally {
      setLoading(false);
    }
  };
  const submit = async (values) => {
    setLoading(true);
    try {
      const path = mode === 'login' ? '/auth/login' : '/auth/register';
      const session = await fetchJson(path, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(values),
      });
      onDone(session);
    } catch (error) {
      message.error(`${mode === 'login' ? '登录' : '注册'}失败：${error.message}`);
    } finally {
      setLoading(false);
    }
  };
  return (
    <Modal title="AgentGrid Hub 登录" open footer={null} closable={false} width={560}>
      <Tabs
        activeKey={mode}
        onChange={setMode}
        items={[
          { key: 'login', label: '登录' },
          { key: 'register', label: '邮箱注册' },
        ]}
      />
      <Form layout="vertical" onFinish={submit}>
        <Form.Item name="email" label="邮箱" rules={[{ required: true }]}><Input /></Form.Item>
        {mode === 'register' && <Form.Item name="name" label="姓名"><Input /></Form.Item>}
        {mode === 'register' && (
          <Form.Item shouldUpdate noStyle>
            {({ getFieldValue }) => (
              <Space.Compact className="full">
                <Form.Item name="code" label="验证码" rules={[{ required: true }]} className="full">
                  <Input />
                </Form.Item>
                <Button className="code-button" loading={loading} onClick={() => sendCode(getFieldValue('email'))}>发送验证码</Button>
              </Space.Compact>
            )}
          </Form.Item>
        )}
        <Form.Item name="password" label="密码" rules={[{ required: true, min: 8 }]}><Input.Password /></Form.Item>
        <Button type="primary" htmlType="submit" loading={loading} block>{mode === 'login' ? '登录' : '注册并登录'}</Button>
      </Form>
    </Modal>
  );
}

function App() {
  const [active, setActive] = useState('overview');
  const [loading, setLoading] = useState(false);
  const [nodes, setNodes] = useState([]);
  const [workbenches, setWorkbenches] = useState([]);
  const [agents, setAgents] = useState([]);
  const [tasks, setTasks] = useState([]);
  const [jobs, setJobs] = useState([]);
  const [artifacts, setArtifacts] = useState([]);
  const [workflows, setWorkflows] = useState([]);
  const [tools, setTools] = useState([]);
  const [probeCenter, setProbeCenter] = useState(null);
  const [runtimeManifest, setRuntimeManifest] = useState({});
  const [runtimeStandard, setRuntimeStandard] = useState({});
  const [workflowTemplates, setWorkflowTemplates] = useState([]);
  const [nodeTools, setNodeTools] = useState([]);
  const [capabilities, setCapabilities] = useState({});
  const [taskTemplatesStore, setTaskTemplatesStore] = useState([]);
  const [webhooks, setWebhooks] = useState([]);
  const [webhookDeliveries, setWebhookDeliveries] = useState([]);
  const [provisioningPlans, setProvisioningPlans] = useState([]);
  const [users, setUsers] = useState([]);
  const [organization, setOrganization] = useState(null);
  const [events, setEvents] = useState([]);
  const [messages, setMessages] = useState([]);
  const [auditEvents, setAuditEvents] = useState([]);
  const [schedulerConfig, setSchedulerConfig] = useState({});
  const [diagnostics, setDiagnostics] = useState({});
  const [commandDoc, setCommandDoc] = useState('');
  const [health, setHealth] = useState({});
  const [bootstrap, setBootstrap] = useState({});
  const [auth, setAuth] = useState(() => loadStoredAuth());
  const [settings, setSettings] = useState({});
  const [selectedTaskId, setSelectedTaskId] = useState(null);

  const refresh = async () => {
    setLoading(true);
    try {
      const [
        bootstrapRes,
        healthRes,
        nodeRes,
        workbenchRes,
        agentRes,
        taskRes,
        jobRes,
        workflowRes,
        toolRes,
        probeCenterRes,
        nodeToolRes,
        capabilityRes,
        runtimeRes,
        standardRes,
        workflowTemplateRes,
        taskTemplateRes,
        webhookRes,
        webhookDeliveryRes,
        provisioningRes,
        userRes,
        eventRes,
        artifactRes,
        messageRes,
        auditRes,
        schedulerRes,
        diagnosticsRes,
        settingsRes,
      ] = await Promise.all([
        fetchJson('/bootstrap'),
        fetchJson('/health'),
        fetchJson('/nodes'),
        fetchJson('/workbenches'),
        fetchJson('/agents'),
        fetchJson('/tasks?limit=100'),
        fetchJson('/jobs?limit=100'),
        fetchJson('/workflows?limit=100'),
        fetchJson('/tools'),
        fetchOptionalJson('/tools/probe-center'),
        fetchJson('/node-tools'),
        fetchJson('/capabilities/manifest'),
        fetchJson('/agent-runtime/manifest'),
        fetchJson('/runtime-standard'),
        fetchJson('/workflow-templates'),
        fetchJson('/task-templates'),
        fetchJson('/webhooks'),
        fetchJson('/webhooks/deliveries'),
        fetchOptionalJson('/node-provisioning/plans'),
        fetchOptionalJson('/users'),
        fetchJson('/events?limit=200'),
        fetchJson('/artifacts'),
        fetchJson('/messages?limit=80'),
        fetchJson('/audit-events'),
        fetchJson('/scheduler-config'),
        fetchJson('/diagnostics'),
        fetchOptionalJson('/settings'),
      ]);
      setBootstrap(bootstrapRes);
      setHealth(healthRes);
      setNodes(nodeRes.items || []);
      setWorkbenches(workbenchRes.items || []);
      setAgents(agentRes.items || []);
      setTasks(taskRes.items || []);
      setJobs(jobRes.items || []);
      setWorkflows(workflowRes.items || []);
      setTools(toolRes.items || []);
      setProbeCenter(probeCenterRes.item || null);
      setNodeTools(nodeToolRes.items || []);
      setCapabilities(capabilityRes || {});
      setRuntimeManifest(runtimeRes || {});
      setRuntimeStandard(standardRes.item || {});
      setWorkflowTemplates(workflowTemplateRes.items || []);
      setTaskTemplatesStore(taskTemplateRes.items || []);
      setWebhooks(webhookRes.items || []);
      setWebhookDeliveries(webhookDeliveryRes.items || []);
      setProvisioningPlans(provisioningRes.items || []);
      setUsers(userRes.items || []);
      setOrganization(userRes.organization || null);
      setEvents(eventRes.items || []);
      setArtifacts(artifactRes.items || []);
      setMessages(messageRes.items || []);
      setAuditEvents(auditRes.items || []);
      setSchedulerConfig(schedulerRes.config || {});
      setDiagnostics(diagnosticsRes.diagnostics || {});
      setSettings(settingsRes.item || {});
    } catch (error) {
      message.error(`刷新失败：${error.message}`);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    refresh();
    const timer = window.setInterval(refresh, 15000);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    if (!auth?.token) return;
    fetchJson('/auth/me')
      .then((data) => {
        if (data.authenticated) {
          setAuth((current) => ({ ...(current || {}), ...data, token: current?.token || auth.token }));
        } else {
          clearStoredAuth();
          setAuth(null);
        }
      })
      .catch(() => {
        clearStoredAuth();
        setAuth(null);
      });
  }, []);

  const acceptAuth = (session) => {
    if (session?.token) {
      saveStoredAuth(session);
    }
    setAuth(session);
  };

  const onlineNodes = nodes.filter((node) => node.status?.state === 'online');
  const offlineNodes = nodes.length - onlineNodes.length;
  const pendingNodes = nodes.filter((node) => node.spec?.auth_status === 'pending');

  return (
    <ConfigProvider
      theme={{
        algorithm: theme.defaultAlgorithm,
        token: {
          colorPrimary: '#1677ff',
          borderRadius: 6,
          fontFamily:
            '-apple-system, BlinkMacSystemFont, "PingFang SC", "Microsoft YaHei", Arial, sans-serif',
        },
      }}
    >
      {bootstrap.needs_bootstrap && (
        <BootstrapAdmin onDone={(session) => { acceptAuth(session); refresh(); }} />
      )}
      {!bootstrap.needs_bootstrap && !auth && (
        <LoginPanel onDone={(session) => { acceptAuth(session); message.success('登录成功'); }} />
      )}
      <ProLayout
        title={false}
        logo={false}
        layout="side"
        navTheme="light"
        siderWidth={256}
        fixSiderbar
        fixedHeader
        className="agentgrid-console"
        token={{
          header: {
            colorBgHeader: '#ffffff',
            heightLayoutHeader: 68,
          },
          sider: {
            colorMenuBackground: '#ffffff',
            colorBgMenuItemHover: '#f3f6fb',
            colorBgMenuItemSelected: '#eaf3ff',
            colorTextMenu: '#374151',
            colorTextMenuSelected: '#1677ff',
            colorTextMenuActive: '#1677ff',
          },
          pageContainer: {
            colorBgPageContainer: '#f5f7fb',
          },
        }}
        route={{ path: '/', routes: menuRoutes }}
        location={{ pathname: `/${active}` }}
        menuItemRender={(item, dom) => (
          <button type="button" className="menu-link" onClick={() => setActive(item.key)}>
            {dom}
          </button>
        )}
        menuHeaderRender={(logo, title) => (
          <div className="pro-brand">
            <div className="brand-mark">AG</div>
            <div>
              <div className="brand-name">AgentGrid</div>
              <div className="brand-sub">AI 机器调度总控台</div>
            </div>
          </div>
        )}
      >
        <PageContainer
          title={pageNames[active] || 'AgentGrid 总控台'}
          breadcrumbRender={false}
          ghost={false}
          className="page-container"
          extra={[
            pendingNodes.length > 0 && <Tag color="orange" key="pending-node">待授权节点 {pendingNodes.length}</Tag>,
            auth?.user && <Tag color="green" key="user">{auth.user.spec?.name || auth.user.spec?.email}</Tag>,
            <Tag color="blue" key="runtime">Rust Hub</Tag>,
            <Text type="secondary" key="time" className="header-time">{formatTime(health.time) || '等待同步'}</Text>,
            <Button key="refresh" icon={<ReloadOutlined />} loading={loading} onClick={refresh}>
              刷新
            </Button>,
          ]}
        >
          <div className="content">
            {active === 'overview' && (
              <Overview
                health={health}
                nodes={nodes}
                workbenches={workbenches}
                onlineNodes={onlineNodes}
                offlineNodes={offlineNodes}
                agents={agents}
                tasks={tasks}
                workflows={workflows}
                messages={messages}
              />
            )}
            {active === 'nodes' && <Nodes nodes={nodes} workbenches={workbenches} onOpenTask={setSelectedTaskId} onDone={refresh} />}
            {active === 'capabilities' && <Capabilities manifest={capabilities} />}
            {active === 'tools' && <Tools tools={tools} probeCenter={probeCenter} onDone={refresh} />}
            {active === 'nodeTools' && <NodeTools nodeTools={nodeTools} nodes={nodes} onDone={refresh} />}
            {active === 'runtime' && <AgentRuntime manifest={runtimeManifest} />}
            {active === 'standard' && <RuntimeStandard standard={runtimeStandard} />}
            {active === 'tasks' && <Tasks tasks={tasks} onOpenTask={setSelectedTaskId} />}
            {active === 'jobs' && <Jobs jobs={jobs} nodes={nodes} tools={tools} onOpenTask={setSelectedTaskId} onDone={refresh} />}
            {active === 'workflows' && <Workflows workflows={workflows} tasks={tasks} nodes={nodes} onOpenTask={setSelectedTaskId} onDone={refresh} />}
            {active === 'queue' && <TaskQueue tasks={tasks} nodes={nodes} onOpenTask={setSelectedTaskId} onDone={refresh} />}
            {active === 'results' && <TaskResults tasks={tasks} onOpenTask={setSelectedTaskId} />}
            {active === 'records' && <ExecutionRecords tasks={tasks} workflows={workflows} />}
            {active === 'artifacts' && <Artifacts artifacts={artifacts} tasks={tasks} />}
            {active === 'terminal' && <RemoteTerminal nodes={nodes} />}
            {active === 'scheduler' && <SchedulerConfig config={schedulerConfig} nodes={nodes} onDone={refresh} />}
            {active === 'settings' && <SystemSettings settings={settings} auth={auth} onDone={refresh} />}
            {active === 'users' && <Users users={users} organization={organization} onDone={refresh} />}
            {active === 'provisioning' && <NodeProvisioning plans={provisioningPlans} settings={settings} onDone={refresh} />}
            {active === 'workflowTemplates' && <WorkflowTemplates templates={workflowTemplates} onDone={refresh} />}
            {active === 'events' && <EventBus initialEvents={events} />}
            {active === 'templates' && <TaskTemplates templates={taskTemplatesStore} nodes={nodes} onDone={refresh} />}
            {active === 'webhooks' && <Webhooks webhooks={webhooks} deliveries={webhookDeliveries} onDone={refresh} />}
            {active === 'diagnostics' && <Diagnostics diagnostics={diagnostics} />}
            {active === 'audit' && <AuditLog events={auditEvents} />}
            {active === 'docs' && <CommandDocs doc={commandDoc} setDoc={setCommandDoc} />}
            {active === 'agents' && <Agents agents={agents} />}
            {active === 'submit' && <SubmitHttp onDone={refresh} />}
            {active === 'command' && <SubmitCommand nodes={nodes} workbenches={workbenches} onDone={refresh} />}
            <TaskDetailModal
              taskId={selectedTaskId}
              tasks={tasks}
              artifacts={artifacts}
              auditEvents={auditEvents}
              onClose={() => setSelectedTaskId(null)}
            />
          </div>
        </PageContainer>
      </ProLayout>
    </ConfigProvider>
  );
}

function Overview({ health, nodes, workbenches, onlineNodes, offlineNodes, agents, tasks, workflows, messages }) {
  const runningTasks = tasks.filter((task) => ['assigned', 'todo', 'in_progress', 'stopping'].includes(task.status?.state)).length;
  const doneTasks = tasks.filter((task) => task.status?.state === 'done').length;
  const latestTask = latestByTime(tasks, (task) => task.metadata?.created_at || task.metadata?.updated_at);
  const verifiedMessages = messages.filter((item) => ['task.completed', 'test.passed', 'node_tool.probe.passed'].includes(item.spec?.type)).length;

  return (
    <Space direction="vertical" size={16} className="full overview-shell">
      <section className="overview-band">
        <div className="overview-copy">
          <Text className="eyebrow">AgentGrid Hub</Text>
          <Title level={2}>AI 操作真实机器和工位的调度层</Title>
          <Text type="secondary">
            统一发现节点、工具、桌面、设备和证据，把结构化任务派到最合适的机器，并留下可审计的执行记录。
          </Text>
        </div>
        <div className="overview-pulse">
          <Badge status={onlineNodes.length ? 'processing' : 'default'} text={onlineNodes.length ? '集群在线' : '等待节点'} />
          <Text type="secondary">{formatTime(health.time) || '等待 Hub 同步'}</Text>
        </div>
      </section>
      <Row gutter={[16, 16]}>
        <Col xs={24} md={12} xl={6}>
          <Metric title="电脑/工位" value={workbenches.length || nodes.length} suffix={offlineNodes ? `离线 ${offlineNodes}` : '全部在线'} prefix={<CloudServerOutlined />} tone="blue" />
        </Col>
        <Col xs={24} md={12} xl={6}>
          <Metric title="在线节点" value={onlineNodes.length} suffix="可接任务" prefix={<CheckCircleOutlined />} tone="green" />
        </Col>
        <Col xs={24} md={12} xl={6}>
          <Metric title="任务运行" value={runningTasks} suffix={`完成 ${doneTasks}`} prefix={<DatabaseOutlined />} tone="amber" />
        </Col>
        <Col xs={24} md={12} xl={6}>
          <Metric title="工作流" value={workflows.length} suffix={`证据 ${verifiedMessages}`} prefix={<ForkOutlined />} tone="violet" />
        </Col>
      </Row>
      <Row gutter={[16, 16]}>
        <Col xs={24} xl={15}>
          <ProCard title="电脑资源" bordered extra={<Tag color="blue">{workbenches.length || nodes.length} 台电脑/工位</Tag>}>
            <NodeResourceList nodes={nodes} workbenches={workbenches} />
          </ProCard>
        </Col>
        <Col xs={24} xl={9}>
          <Space direction="vertical" size={16} className="full">
            <PlacementSnapshot task={latestTask} />
            <HubStatus health={health} agents={agents} />
          </Space>
        </Col>
      </Row>
      <ProCard title="最近消息流" bordered extra={<Tag>{messages.length} 条</Tag>}>
        <MessageList messages={messages.slice(0, 8)} />
      </ProCard>
    </Space>
  );
}

function Metric({ prefix, tone = 'blue', ...props }) {
  return (
    <Card className={`metric metric-${tone}`}>
      <div className="metric-content">
        <Statistic {...props} />
        {prefix && <div className="metric-icon">{prefix}</div>}
      </div>
    </Card>
  );
}

function Nodes({ nodes, workbenches, onOpenTask, onDone }) {
  const [editingNode, setEditingNode] = useState(null);
  const [selectedWorkbench, setSelectedWorkbench] = useState(null);
  const physicalHosts = normalizeWorkbenchRows(workbenches, nodes);
  const approve = async (node) => {
    try {
      await fetchJson(`/nodes/${node.metadata.id}/approve`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ actor: 'super-admin' }),
      });
      message.success('节点已授权');
      onDone();
    } catch (error) {
      message.error(`授权失败：${error.message}`);
    }
  };
  return (
    <ProCard title="电脑管理" bordered extra={<Tag color="blue">{physicalHosts.length} 台电脑 / {nodes.length} 个能力通道</Tag>}>
      <Table
        size="middle"
        className="nodes-table"
        rowKey={(row) => row.id}
        dataSource={physicalHosts}
        pagination={false}
        tableLayout="fixed"
        scroll={{ x: 1320 }}
        columns={[
          {
            title: '状态',
            width: 86,
            render: (_, row) => (
              <Badge status={row.state === 'online' ? 'success' : 'default'} text={stateLabel(row.state)} />
            ),
          },
          {
            title: '电脑',
            width: 220,
            render: (_, row) => (
              <Space direction="vertical" size={1} className="node-identity">
                <Text strong>{row.name}</Text>
                <Text type="secondary">{row.primary?.metadata?.id || row.id}</Text>
                <Space size={4} wrap>
                  {channelRoles.map((role) => (
                    <ChannelStatus key={role} role={role} node={row.channels[role]} />
                  ))}
                </Space>
              </Space>
            ),
          },
          {
            title: '操作系统',
            width: 118,
            render: (_, row) => (
              <Space direction="vertical" size={1}>
                <Text>{row.primary?.spec?.os || '-'}</Text>
                <Text type="secondary">{row.primary?.spec?.arch || '-'}</Text>
              </Space>
            ),
          },
          {
            title: '主机 / IP',
            width: 230,
            render: (_, row) => <Text copyable className="mono-cell">{row.primary?.spec?.address || '-'}</Text>,
          },
          {
            title: 'CPU',
            width: 138,
            render: (_, row) => (
              <ResourceCell
                percent={round(workbenchResource(row, 'cpu_usage_percent'))}
                detail={`${workbenchResource(row, 'cpu_cores')} 核`}
              />
            ),
          },
          {
            title: '内存',
            width: 170,
            render: (_, row) => (
              <ResourceCell
                percent={percent(workbenchResource(row, 'memory_used_mb'), workbenchResource(row, 'memory_mb'))}
                detail={`${formatMb(workbenchResource(row, 'memory_used_mb'))} / ${formatMb(workbenchResource(row, 'memory_mb'))}`}
              />
            ),
          },
          {
            title: '硬盘',
            width: 170,
            render: (_, row) => (
              <ResourceCell
                percent={percent(
                  workbenchResource(row, 'disk_total_mb') - workbenchResource(row, 'disk_free_mb'),
                  workbenchResource(row, 'disk_total_mb'),
                )}
                detail={`${formatMb(workbenchResource(row, 'disk_total_mb') - workbenchResource(row, 'disk_free_mb'))} / ${formatMb(workbenchResource(row, 'disk_total_mb'))}`}
              />
            ),
          },
          {
            title: '能力',
            width: 280,
            render: (_, row) => (
              <div className="capability-list">
                {row.capabilities.map((item) => <Tag key={item}>{item}</Tag>)}
              </div>
            ),
          },
          {
            title: '调度',
            width: 150,
            render: (_, row) => (
              <Space direction="vertical" size={1}>
                <Text>权重 {row.primary?.spec?.weight}</Text>
                <Text type="secondary">
                  槽位 {workbenchResource(row, 'running_jobs')}/{workbenchResource(row, 'max_concurrent_jobs') || 1}
                </Text>
                <Text type="secondary">
                  成功 {row.primary?.status?.success_count || 0} / 失败 {row.primary?.status?.failure_count || 0}
                </Text>
              </Space>
            ),
          },
          {
            title: 'Worker',
            width: 190,
            render: (_, row) => (
              <Space direction="vertical" size={1}>
                <Text>{row.primary?.spec?.worker_version || '-'}</Text>
                <Text type="secondary">{row.primary?.spec?.worker_target || row.primary?.spec?.arch || '-'}</Text>
                <Text type="secondary">授权：{nodeAuthLabel(row.primary?.spec?.auth_status)}</Text>
                <Text type="secondary">
                  glibc {row.primary?.spec?.glibc_version || '-'} · {row.primary?.spec?.auto_update_enabled ? '自动更新' : '手动更新'}
                </Text>
              </Space>
            ),
          },
          {
            title: '最后心跳',
            width: 170,
            render: (_, row) => <Text type="secondary">{formatTime(row.primary?.status?.last_heartbeat_at)}</Text>,
          },
          {
            title: '操作',
            width: 220,
            fixed: 'right',
            render: (_, row) => (
              <Space wrap>
                <Button size="small" type="primary" onClick={() => setSelectedWorkbench(row)}>
                  详情
                </Button>
                {row.nodes.filter((node) => node.spec.auth_status === 'pending').map((node) => (
                  <Button key={node.metadata.id} size="small" type="primary" onClick={() => approve(node)}>授权</Button>
                ))}
                {row.channels.worker && (
                  <Button size="small" icon={<SettingOutlined />} onClick={() => setEditingNode(row.channels.worker)}>
                    配置后台
                  </Button>
                )}
              </Space>
            ),
          },
        ]}
        expandable={{
          expandedRowRender: (row) => (
            <Table
              size="small"
              pagination={false}
              rowKey={(node) => node.metadata.id}
              dataSource={row.nodes}
              columns={[
                { title: '通道', width: 120, render: (_, node) => <Tag>{channelLabel(nodeChannelRole(node))}</Tag> },
                { title: '节点 ID', render: (_, node) => <Text copyable>{node.metadata.id}</Text> },
                { title: '状态', width: 120, render: (_, node) => <Badge status={node.status.state === 'online' ? 'success' : 'default'} text={stateLabel(node.status.state)} /> },
                { title: '职责', width: 220, render: (_, node) => <Text type="secondary">{channelDuty(nodeChannelRole(node))}</Text> },
                { title: '能力', render: (_, node) => <Space wrap>{(node.spec.capabilities || []).map((item) => <Tag key={item}>{item}</Tag>)}</Space> },
                { title: '授权', width: 120, render: (_, node) => nodeAuthLabel(node.spec.auth_status) },
                { title: '心跳', width: 170, render: (_, node) => formatTime(node.status.last_heartbeat_at) },
              ]}
            />
          ),
        }}
      />
      <WorkbenchDetailDrawer
        workbench={selectedWorkbench}
        onClose={() => setSelectedWorkbench(null)}
        onOpenTask={onOpenTask}
        onDone={onDone}
      />
      <NodeConfigModal
        node={editingNode}
        onClose={() => setEditingNode(null)}
        onDone={() => {
          setEditingNode(null);
          onDone();
        }}
      />
    </ProCard>
  );
}

function ResourceCell({ percent: valuePercent, detail }) {
  return (
    <div className="resource-cell">
      <div className="resource-cell-head">
        <Text strong>{valuePercent}%</Text>
        <Text type="secondary">{detail}</Text>
      </div>
      <Progress percent={valuePercent} size="small" showInfo={false} />
    </div>
  );
}

function WorkbenchDetailDrawer({ workbench, onClose, onOpenTask, onDone }) {
  const [form] = Form.useForm();
  const [timeline, setTimeline] = useState(null);
  const [loadingTimeline, setLoadingTimeline] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const workbenchId = workbench?.id;
  const selectedAction = Form.useWatch('action', form);
  const selectedTool = Form.useWatch('tool', form);
  const selectedPlacement = workbenchActionPlacement(selectedAction, workbench);

  const loadTimeline = async () => {
    if (!workbenchId) return;
    setLoadingTimeline(true);
    try {
      const data = await fetchJson(`/workbenches/${encodeURIComponent(workbenchId)}/timeline`);
      setTimeline(data.item);
    } catch (error) {
      message.error(`读取时间线失败：${error.message}`);
    } finally {
      setLoadingTimeline(false);
    }
  };

  useEffect(() => {
    setTimeline(null);
    if (!workbenchId) return;
    form.setFieldsValue({
      action: 'command',
      program: 'hostname',
      path: '',
      tool: '',
      title: '',
      args: '',
      payload: '{}',
    });
    loadTimeline();
  }, [workbenchId]);

  const submit = async (values) => {
    setSubmitting(true);
    try {
      const request = buildWorkbenchActionRequest(workbenchId, values);
      const response = await fetchJson(request.path, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(request.body),
      });
      message.success('已提交到这台电脑');
      onDone();
      loadTimeline();
      const taskId = response?.item?.task_id;
      if (taskId) onOpenTask?.(taskId);
    } catch (error) {
      message.error(`提交失败：${error.message}`);
    } finally {
      setSubmitting(false);
    }
  };

  const timelineEvents = timeline?.events || [];
  const channelRows = channelRoles.map((role) => ({
    role,
    node: workbench?.channels?.[role],
  }));

  return (
    <Drawer
      title={workbench ? `${workbench.name} / 电脑详情` : '电脑详情'}
      open={Boolean(workbench)}
      onClose={onClose}
      width={980}
    >
      {workbench && (
        <Space direction="vertical" className="full" size={16}>
          <Row gutter={12}>
            <Col span={6}><Card size="small"><Statistic title="状态" value={stateLabel(workbench.state)} /></Card></Col>
            <Col span={6}><Card size="small"><Statistic title="在线通道" value={workbench.workbench?.status?.online_channels || workbench.nodes.filter((node) => node.status?.state === 'online').length} /></Card></Col>
            <Col span={6}><Card size="small"><Statistic title="CPU" value={`${workbenchResource(workbench, 'cpu_cores')} 核`} /></Card></Col>
            <Col span={6}><Card size="small"><Statistic title="内存" value={formatMb(workbenchResource(workbench, 'memory_mb'))} /></Card></Col>
          </Row>

          <ProCard title="通道" bordered>
            <Table
              size="small"
              pagination={false}
              rowKey={(row) => row.role}
              dataSource={channelRows}
              columns={[
                { title: '通道', width: 120, render: (_, row) => <Tag color={channelColor(row.role, row.node)}>{channelLabel(row.role)}</Tag> },
                { title: '节点', render: (_, row) => row.node ? <Text copyable>{row.node.metadata.id}</Text> : <Text type="secondary">未安装</Text> },
                { title: '用途', render: (_, row) => channelDuty(row.role) },
                { title: '状态', width: 120, render: (_, row) => row.node ? stateLabel(row.node.status?.state) : '-' },
              ]}
            />
          </ProCard>

          <ProCard title="工具验证" bordered>
            <WorkbenchToolProbeSummary workbench={workbench} />
          </ProCard>

          <ProCard title="操作这台电脑" bordered>
            <Form form={form} layout="vertical" onFinish={submit}>
              <WorkbenchActionTrust workbench={workbench} action={selectedAction} runtimeTool={selectedTool} />
              <Row gutter={12}>
                <Col span={6}>
                  <Form.Item name="action" label="动作" rules={[{ required: true }]}>
                    <Select
                      options={[
                        { value: 'command', label: '执行命令' },
                        { value: 'screenshot', label: '截屏' },
                        { value: 'file_list', label: '查看文件' },
                        { value: 'runtime', label: '运行工具' },
                      ]}
                    />
                  </Form.Item>
                </Col>
                <Col span={10}>
                  <Form.Item noStyle shouldUpdate>
                    {({ getFieldValue }) => {
                      const action = getFieldValue('action');
                      if (action === 'runtime') {
                        return <Form.Item name="tool" label="工具 ID" rules={[{ required: true }]}><Input placeholder="audio.tts.clone" /></Form.Item>;
                      }
                      if (action === 'file_list') {
                        return <Form.Item name="path" label="路径" rules={[{ required: true }]}><Input placeholder="C:\\ 或 /tmp" /></Form.Item>;
                      }
                      return <Form.Item name="program" label="命令"><Input placeholder="hostname" /></Form.Item>;
                    }}
                  </Form.Item>
                </Col>
                <Col span={8}>
                  <Form.Item name="title" label="标题">
                    <Input placeholder="可选" />
                  </Form.Item>
                </Col>
              </Row>
              <Form.Item noStyle shouldUpdate>
                {({ getFieldValue }) => {
                  const action = getFieldValue('action');
                  if (action === 'runtime') {
                    return <Form.Item name="payload" label="Payload JSON"><Input.TextArea rows={5} /></Form.Item>;
                  }
                  if (action === 'command') {
                    return <Form.Item name="args" label="参数，每行一个"><Input.TextArea rows={3} /></Form.Item>;
                  }
                  return null;
                }}
              </Form.Item>
              <Space>
                <Button type="primary" htmlType="submit" loading={submitting}>提交</Button>
                <Button icon={<ReloadOutlined />} onClick={loadTimeline} loading={loadingTimeline}>刷新记录</Button>
              </Space>
            </Form>
          </ProCard>

          <ProCard title="调度解释" bordered>
            <Space direction="vertical" size={6}>
              <Text strong>{selectedPlacement.title}</Text>
              <Text>
                目标通道：<Tag color={channelColor(selectedPlacement.role, selectedPlacement.node)}>{channelLabel(selectedPlacement.role)}</Tag>
                {selectedPlacement.node ? <Text copyable>{selectedPlacement.node.metadata.id}</Text> : <Text type="secondary">当前未安装或未在线</Text>}
              </Text>
              <Text>{selectedPlacement.reason}</Text>
              <Text type="secondary">{selectedPlacement.blocked}</Text>
              <Text type="secondary">用户和 AI 只需要选择这台电脑，Hub 会根据任务契约选择具体通道。</Text>
            </Space>
          </ProCard>

          <ProCard title="最近操作" bordered extra={<Tag>{timelineEvents.length} 条</Tag>}>
            <WorkbenchOperationFeed events={timelineEvents} loading={loadingTimeline} onOpenTask={onOpenTask} />
          </ProCard>

          <ProCard title="操作明细" bordered>
            <Table
              size="small"
              loading={loadingTimeline}
              pagination={{ pageSize: 8 }}
              rowKey={(row) => row.id}
              dataSource={timelineEvents}
              columns={[
                { title: '时间', width: 170, render: (_, row) => formatTime(row.time) },
                { title: '类型', width: 130, render: (_, row) => <Tag>{row.type}</Tag> },
                { title: '节点', width: 210, render: (_, row) => row.node_id || '-' },
                {
                  title: '任务',
                  width: 210,
                  render: (_, row) => row.task_id ? (
                    <Button size="small" type="link" onClick={() => onOpenTask?.(row.task_id)}>
                      {row.task_id}
                    </Button>
                  ) : '-',
                },
                { title: '说明', render: (_, row) => row.summary || '-' },
              ]}
              expandable={{
                expandedRowRender: (row) => <pre className="result-json">{JSON.stringify(row.payload, null, 2)}</pre>,
              }}
            />
          </ProCard>
        </Space>
      )}
    </Drawer>
  );
}

function WorkbenchToolProbeSummary({ workbench }) {
  const tools = workbench?.tools || [];
  const rows = tools.map((tool) => ({
    key: `${tool.metadata?.node_id || tool.metadata?.id}:${tool.spec?.tool_id}`,
    tool_id: tool.spec?.tool_id,
    name: tool.spec?.name,
    executor: tool.spec?.executor,
    node_id: tool.metadata?.node_id,
    state: tool.status?.probe_state || 'declared_unverified',
    last_probe_at: tool.status?.last_probe_at,
    next_probe_at: tool.status?.next_probe_at,
    error: tool.status?.probe_error,
  }));
  if (!rows.length) {
    return <div className="empty-panel">这台电脑还没有注册动态工具。内置能力会在工具目录里统一验证。</div>;
  }
  const verified = rows.filter((row) => row.state === 'verified').length;
  const failed = rows.filter((row) => row.state === 'failed').length;
  return (
    <Space direction="vertical" className="full">
      <Space wrap>
        <Tag color="green">已验证 {verified}</Tag>
        <Tag color={failed ? 'red' : 'default'}>失败 {failed}</Tag>
        <Tag>总数 {rows.length}</Tag>
      </Space>
      <Table
        size="small"
        pagination={false}
        rowKey={(row) => row.key}
        dataSource={rows}
        columns={[
          { title: '工具', render: (_, row) => <Text code>{row.tool_id}</Text> },
          { title: '节点通道', render: (_, row) => <Text copyable>{row.node_id}</Text> },
          { title: '执行器', render: (_, row) => <Text code>{row.executor}</Text> },
          { title: '验证', render: (_, row) => <Tag color={probeStateColor(row.state)}>{probeStateLabel(row.state)}</Tag> },
          { title: '最近验证', render: (_, row) => formatTime(row.last_probe_at) || '-' },
          { title: '下次验证', render: (_, row) => formatTime(row.next_probe_at) || '-' },
          { title: '错误', render: (_, row) => compactJson(row.error) },
        ]}
      />
    </Space>
  );
}

function WorkbenchActionTrust({ workbench, action, runtimeTool }) {
  const toolId = workbenchActionToolId(action, runtimeTool);
  if (!toolId) return null;
  const channels = workbench?.channels || {};
  const channelRole = action === 'screenshot' ? 'desktop' : 'worker';
  const channelNode = channels[channelRole];
  const nodeId = channelNode?.metadata?.id;
  const allDynamicTools = workbench?.tools || [];
  const dynamic = allDynamicTools.find((tool) => (
    tool.spec?.tool_id === toolId && (!nodeId || tool.metadata?.node_id === nodeId)
  ));
  const state = dynamic?.status?.probe_state || 'declared_unverified';
  const color = probeStateColor(state);
  const label = probeStateLabel(state);
  const messageText = dynamic
    ? `这台电脑的 ${toolId} 已注册到 ${dynamic.metadata?.node_id || '节点'}，当前验证状态：${label}。`
    : `这次动作需要 ${toolId}。内置能力会由 Hub 的 Tool Probe Center 验证；如果没有验证记录，调度器会降权或选择已验证节点。`;
  const type = state === 'failed' ? 'warning' : state === 'verified' ? 'success' : 'info';
  return (
    <Alert
      className="workbench-trust-alert"
      showIcon
      type={type}
      message={
        <Space wrap>
          <Text strong>能力可信度</Text>
          <Tag color={color}>{label}</Tag>
          <Text code>{toolId}</Text>
          {nodeId && <Text type="secondary">{nodeId}</Text>}
        </Space>
      }
      description={messageText}
    />
  );
}

function workbenchActionToolId(action, runtimeTool) {
  return {
    command: 'command.run',
    screenshot: 'desktop.screenshot',
    file_list: 'file.list',
    runtime: String(runtimeTool || '').trim() || null,
  }[action] || null;
}

function WorkbenchOperationFeed({ events, loading, onOpenTask }) {
  if (loading && !events.length) {
    return <div className="empty-panel">正在读取这台电脑的操作记录...</div>;
  }
  if (!events.length) {
    return <div className="empty-panel">这台电脑还没有操作记录。</div>;
  }
  return (
    <div className="operation-feed">
      {events.slice(0, 10).map((event) => (
        <div key={event.id} className={`operation-item operation-${event.state || 'unknown'}`}>
          <div className="operation-dot" />
          <div className="operation-main">
            <div className="operation-head">
              <Space wrap>
                <Tag color={operationEventColor(event.type)}>{operationEventLabel(event.type)}</Tag>
                {event.state && <Tag color={taskStateColor(event.state)}>{stateLabel(event.state)}</Tag>}
                <Text strong>{event.summary || '-'}</Text>
              </Space>
              <Text type="secondary">{formatTime(event.time)}</Text>
            </div>
            <div className="operation-meta">
              {event.node_id && <Text type="secondary">节点：{event.node_id}</Text>}
              {event.task_id && (
                <Button size="small" type="link" onClick={() => onOpenTask?.(event.task_id)}>
                  查看任务
                </Button>
              )}
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}

function buildWorkbenchActionRequest(workbenchId, values) {
  const action = values.action || 'command';
  if (action === 'runtime') {
    return {
      path: `/workbenches/${encodeURIComponent(workbenchId)}/actions`,
      body: {
        action: 'runtime.submit',
        payload: {
          tool_id: values.tool,
          payload: parseJsonOrDefault(values.payload, {}),
        },
        title: values.title || `运行工具 ${values.tool}`,
        created_by: 'web-console',
      },
    };
  }
  let payload;
  let apiAction;
  if (action === 'screenshot') {
    apiAction = 'desktop.screenshot';
    payload = { operation: 'screenshot', path: null, timeout_seconds: 30 };
  } else if (action === 'file_list') {
    apiAction = 'file.list';
    payload = { operation: 'list', path: values.path, recursive: false, max_entries: 200 };
  } else {
    apiAction = 'command.run';
    payload = {
      program: values.program || 'hostname',
      args: values.args ? values.args.split('\n').map((item) => item.trim()).filter(Boolean) : [],
      working_dir: null,
      timeout_seconds: 30,
    };
  }
  return {
    path: `/workbenches/${encodeURIComponent(workbenchId)}/actions`,
    body: {
      action: apiAction,
      payload,
      title: values.title || `${workbenchId} ${action}`,
      created_by: 'web-console',
      priority: 'normal',
    },
  };
}

function NodeConfigModal({ node, onClose, onDone }) {
  const [form] = Form.useForm();
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!node) return;
    form.setFieldsValue({
      weight: node.spec.weight,
      max_concurrent_jobs: node.spec.max_concurrent_jobs,
      groups: (node.spec.groups || []).join(', '),
      tags: (node.spec.tags || []).join(', '),
      capabilities: (node.spec.capabilities || []).join(', '),
      status: node.status.reported_state || node.status.state,
    });
  }, [form, node]);

  const submit = async (values) => {
    setSaving(true);
    try {
      await fetchJson(`/nodes/${node.metadata.id}/config`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          weight: Number(values.weight || 1),
          max_concurrent_jobs: Number(values.max_concurrent_jobs || 1),
          groups: splitList(values.groups),
          tags: splitList(values.tags),
          capabilities: splitList(values.capabilities),
          status: values.status,
        }),
      });
      message.success('节点配置已保存');
      onDone();
    } catch (error) {
      message.error(`保存失败：${error.message}`);
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal
      title={node ? `节点配置：${node.metadata.name}` : '节点配置'}
      open={Boolean(node)}
      onCancel={onClose}
      onOk={() => form.submit()}
      confirmLoading={saving}
      width={720}
    >
      <Form form={form} layout="vertical" onFinish={submit}>
        <Row gutter={12}>
          <Col span={8}><Form.Item name="weight" label="节点权重"><InputNumber min={0.1} step={0.1} className="full" /></Form.Item></Col>
          <Col span={8}><Form.Item name="max_concurrent_jobs" label="并发槽位"><InputNumber min={1} max={128} className="full" /></Form.Item></Col>
          <Col span={8}>
            <Form.Item name="status" label="状态">
              <Select
                options={[
                  { value: 'online', label: '在线' },
                  { value: 'disabled', label: '禁用' },
                  { value: 'draining', label: '排空' },
                ]}
              />
            </Form.Item>
          </Col>
        </Row>
        <Form.Item name="groups" label="节点分组"><Input placeholder="default, linux, worker" /></Form.Item>
        <Form.Item name="tags" label="节点标签"><Input placeholder="worker, linux" /></Form.Item>
        <Form.Item name="capabilities" label="节点能力"><Input placeholder="http, command, file, git, docker, browser, agentmessage" /></Form.Item>
      </Form>
    </Modal>
  );
}

function NodeResourceList({ nodes, workbenches = [] }) {
  const physicalHosts = normalizeWorkbenchRows(workbenches, nodes);
  if (!physicalHosts.length) {
    return <div className="empty-panel">暂无节点。安装 Worker 后，这里会显示 CPU、内存、硬盘和可用能力。</div>;
  }

  return (
    <div className="node-resource-list">
      {physicalHosts.map((host) => {
        const node = host.primary;
        return (
        <div key={host.id} className={`node-resource-row node-state-${host.state || 'unknown'}`}>
          <div className="node-summary">
            <div className="node-status-line">
              <Text strong className="node-title">{host.name}</Text>
              <Badge status={host.state === 'online' ? 'success' : 'default'} text={stateLabel(host.state)} />
            </div>
            <Text type="secondary" className="node-meta">
              {node.spec.os || '-'} · {node.spec.cpu_cores || 0} 核 · {node.spec.address || '-'}
            </Text>
            <div className="node-badges">
              <ChannelStatus role="worker" node={host.channels.worker} />
              <ChannelStatus role="desktop" node={host.channels.desktop} />
              {host.capabilities.slice(0, 3).map((item) => <Tag key={item}>{item}</Tag>)}
              {host.capabilities.length > 3 && <Tag>+{host.capabilities.length - 3}</Tag>}
            </div>
          </div>
          <ResourceMeter
            title="CPU"
            value={`${round(node.spec.cpu_usage_percent)}%`}
            detail={`${node.spec.cpu_cores || 0} 核心`}
            percent={round(node.spec.cpu_usage_percent)}
          />
          <ResourceMeter
            title="内存"
            value={`${percent(node.spec.memory_used_mb, node.spec.memory_mb)}%`}
            detail={`${formatMb(node.spec.memory_used_mb)} / ${formatMb(node.spec.memory_mb)}`}
            percent={percent(node.spec.memory_used_mb, node.spec.memory_mb)}
          />
          <ResourceMeter
            title="硬盘"
            value={`${diskUsedPercent(node)}%`}
            detail={`${formatMb((node.spec.disk_total_mb || 0) - (node.spec.disk_free_mb || 0))} / ${formatMb(node.spec.disk_total_mb)}`}
            percent={diskUsedPercent(node)}
          />
        </div>
        );
      })}
    </div>
  );
}

function ResourceMeter({ title, value, detail, percent: valuePercent }) {
  return (
    <div className="resource-meter">
      <div className="resource-meter-head">
        <Text strong>{title}</Text>
        <Text strong>{value}</Text>
      </div>
      <Progress percent={valuePercent} size="small" showInfo={false} />
      <Text type="secondary" className="resource-meter-detail">{detail}</Text>
    </div>
  );
}

function PlacementSnapshot({ task }) {
  const inputPayload = parseTaskInputPayload(task);
  const selectedNode = task ? (task.status?.leased_by_node_id || routeNode(task) || '-') : '-';
  const progress = taskStateProgress(task?.status?.state);
  const scheduler = task?.status?.result?.scheduler || task?.status?.error?.scheduler;
  const reason = scheduler?.reason || task?.status?.blocked_reason || (task ? '等待 Worker 回写调度原因' : '暂无任务');

  return (
    <ProCard title="调度快照" bordered className="decision-card">
      <Space direction="vertical" size={14} className="full">
        <DescriptionsList
          rows={[
            ['任务', task?.spec?.title || '暂无任务'],
            ['工具', taskOperationLabel(inputPayload)],
            ['节点', selectedNode],
            ['原因', reason],
          ]}
        />
        <Progress percent={progress} showInfo={false} strokeColor="#1677ff" />
        <Text type="secondary">
          状态 {stateLabel(task?.status?.state)} · 评分 {scheduler?.score ?? '-'}
        </Text>
      </Space>
    </ProCard>
  );
}

function HubStatus({ health, agents }) {
  return (
    <ProCard title="中心服务器" bordered className="hub-status-card">
      <div className="hub-status-grid">
        <div>
          <Text type="secondary">服务</Text>
          <Text strong>{health.service || '-'}</Text>
        </div>
        <div>
          <Text type="secondary">运行时</Text>
          <Text strong>{health.runtime || 'Rust Hub'}</Text>
        </div>
        <div>
          <Text type="secondary">AI 员工</Text>
          <Text strong>{agents.length}</Text>
        </div>
        <div>
          <Text type="secondary">入口</Text>
          <Text copyable className="hub-url">{window.location.origin}/agentgrid</Text>
        </div>
      </div>
    </ProCard>
  );
}

function Capabilities({ manifest }) {
  const tools = manifest.tools || [];
  const endpoints = Object.entries(manifest.endpoints || {}).map(([name, path]) => ({ name, path }));
  const jobFeatures = manifest.job_features || {};
  const toolsWithNodes = tools.filter((tool) => Number(tool.available_nodes || 0) > 0).length;
  const verifiedNodeCount = tools.reduce((sum, tool) => sum + Number(tool.verified_nodes || 0), 0);
  const partitionTools = tools.filter((tool) => tool.supports_partition).length;
  const highRiskTools = tools.filter((tool) => tool.risk === 'high').length;

  return (
    <Space direction="vertical" size={16} className="full">
      <Row gutter={[16, 16]}>
        <Col xs={24} md={12} xl={6}><Metric title="可调用工具" value={tools.length} prefix={<ApiOutlined />} /></Col>
        <Col xs={24} md={12} xl={6}><Metric title="有节点可跑" value={toolsWithNodes} /></Col>
        <Col xs={24} md={12} xl={6}><Metric title="已验证节点" value={verifiedNodeCount} /></Col>
        <Col xs={24} md={12} xl={6}><Metric title="支持分片" value={partitionTools} /></Col>
      </Row>

      <ProCard title="AI 接入流程" bordered>
        <Steps
          size="small"
          current={(manifest.workflow || []).length - 1}
          items={(manifest.workflow || []).map((item) => ({
            title: {
              discover_capabilities: '发现能力',
              select_tool: '选择工具',
              construct_job: '构造 Job',
              submit_job: '提交任务',
              watch_job: '监听状态',
              read_status_result: '读取结果',
            }[item] || item,
          }))}
        />
      </ProCard>

      <Row gutter={[16, 16]}>
        <Col xs={24} xl={16}>
          <ProCard title="工具能力矩阵" bordered>
            <Table
              rowKey={(row) => row.tool_id}
              dataSource={tools}
              tableLayout="fixed"
              scroll={{ x: 1360 }}
              columns={[
                {
                  title: '工具',
                  width: 260,
                  render: (_, row) => (
                    <Space direction="vertical" size={1}>
                      <Text strong>{row.name || row.tool_id}</Text>
                      <Text copyable type="secondary">{row.tool_id}</Text>
                    </Space>
                  ),
                },
                { title: '分类', width: 120, render: (_, row) => <Tag>{row.category || '-'}</Tag> },
                { title: '能力', width: 120, render: (_, row) => <Text code>{row.capability || '-'}</Text> },
                { title: '可用节点', width: 100, render: (_, row) => <Tag color={row.available_nodes ? 'green' : 'red'}>{row.available_nodes || 0}</Tag> },
                { title: '验证节点', width: 100, render: (_, row) => <Tag color={row.verified_nodes ? 'green' : 'default'}>{row.verified_nodes || 0}</Tag> },
                { title: '分片', width: 90, render: (_, row) => row.supports_partition ? <Tag color="blue">支持</Tag> : <Tag>不支持</Tag> },
                { title: '模板', width: 90, render: (_, row) => row.supports_template ? <Tag color="blue">支持</Tag> : <Tag>不支持</Tag> },
                { title: '归约', width: 130, render: (_, row) => <Text code>{row.recommended_reduce || 'summary'}</Text> },
                { title: '风险', width: 90, render: (_, row) => <Tag color={riskColor(row.risk)}>{riskLabel(row.risk)}</Tag> },
                {
                  title: '说明',
                  render: (_, row) => <Text type="secondary">{row.summary || '-'}</Text>,
                },
              ]}
              expandable={{
                expandedRowRender: (row) => (
                  <Space direction="vertical" size={14} className="full">
                    <Row gutter={[12, 12]}>
                      <Col xs={24} xl={8}>
                        <ProCard size="small" title="输入 Schema" bordered>
                          <pre className="result-json">{JSON.stringify(row.input_schema || {}, null, 2)}</pre>
                        </ProCard>
                      </Col>
                      <Col xs={24} xl={8}>
                        <ProCard size="small" title="Job 示例" bordered>
                          <pre className="result-json">{JSON.stringify(row.job_example || {}, null, 2)}</pre>
                        </ProCard>
                      </Col>
                      <Col xs={24} xl={8}>
                        <ProCard size="small" title="示例 Payload" bordered>
                          <pre className="result-json">{JSON.stringify(row.examples?.[0] || {}, null, 2)}</pre>
                        </ProCard>
                      </Col>
                    </Row>
                    <Table
                      size="small"
                      pagination={false}
                      rowKey={(node) => node.id}
                      dataSource={row.nodes || []}
                      columns={[
                        { title: '节点', render: (_, node) => <Text strong>{node.name || node.id}</Text> },
                        { title: 'ID', dataIndex: 'id' },
                        { title: '系统', render: (_, node) => `${node.os || '-'} / ${node.arch || '-'}` },
                        { title: '地址', render: (_, node) => node.address || '-' },
                        { title: 'CPU', render: (_, node) => `${node.cpu_cores || 0} 核` },
                        { title: '内存', render: (_, node) => formatMb(node.memory_mb) },
                        { title: '并发', render: (_, node) => `${node.running_jobs || 0} / ${node.max_concurrent_jobs || 0}` },
                        { title: '验证', render: (_, node) => <Tag color={probeStateColor(node.verification_status)}>{probeStateLabel(node.verification_status)}</Tag> },
                      ]}
                    />
                  </Space>
                ),
              }}
            />
          </ProCard>
        </Col>

        <Col xs={24} xl={8}>
          <Space direction="vertical" size={16} className="full">
            <ProCard title="Job Runtime 能力" bordered>
              <Space direction="vertical" className="full">
                <div>
                  <Text strong>分区模式</Text>
                  <div className="capability-list">
                    {(jobFeatures.partition || []).map((item) => <Tag key={item}>{item}</Tag>)}
                  </div>
                </div>
                <div>
                  <Text strong>模板变量</Text>
                  <div className="capability-list">
                    {(jobFeatures.template_variables || []).map((item) => <Tag key={item}>{item}</Tag>)}
                  </div>
                </div>
                <div>
                  <Text strong>归约策略</Text>
                  <div className="capability-list">
                    {(jobFeatures.reduce || []).map((item) => <Tag color="blue" key={item}>{item}</Tag>)}
                  </div>
                </div>
                <Space wrap>
                  <Tag color={jobFeatures.checkpoint_resume ? 'green' : 'default'}>断点续调度</Tag>
                  <Tag color={jobFeatures.node_lost_reschedule ? 'green' : 'default'}>节点丢失重调度</Tag>
                  <Tag color={highRiskTools ? 'orange' : 'green'}>高风险工具 {highRiskTools}</Tag>
                </Space>
              </Space>
            </ProCard>

            <ProCard title="标准接口" bordered>
              <Table
                size="small"
                pagination={false}
                rowKey={(row) => row.name}
                dataSource={endpoints}
                columns={[
                  { title: '用途', dataIndex: 'name', width: 120 },
                  { title: '路径', render: (_, row) => <Text copyable code>{row.path}</Text> },
                ]}
              />
            </ProCard>

            <ProCard title="Manifest 元信息" bordered>
              <Space direction="vertical" className="full">
                <Text>版本：<Text code>{manifest.api_version || '-'}</Text></Text>
                <Text>类型：<Text code>{manifest.kind || '-'}</Text></Text>
                <Text>项目：{manifest.metadata?.project_id || '-'}</Text>
                <Text>Hub：<Text copyable>{manifest.metadata?.hub_url || '-'}</Text></Text>
                <Text>生成：{formatTime(manifest.metadata?.generated_at)}</Text>
              </Space>
            </ProCard>
          </Space>
        </Col>
      </Row>
    </Space>
  );
}

function Tools({ tools, probeCenter, onDone }) {
  const [selected, setSelected] = useState(null);
  const [probing, setProbing] = useState(null);
  const categories = Array.from(new Set(tools.map((tool) => tool.category || 'other')));
  const riskCounts = {
    high: tools.filter((tool) => tool.risk === 'high').length,
    medium: tools.filter((tool) => tool.risk === 'medium').length,
    low: tools.filter((tool) => tool.risk === 'low').length,
  };
  const summary = probeCenter?.summary || {};
  const centerTools = probeCenter?.tools || tools;
  const workbenchRows = probeCenter?.workbenches || [];

  const runProbe = async (toolId) => {
    const key = toolId || '__all__';
    setProbing(key);
    try {
      const path = toolId ? `/tools/${toolId}/probe` : '/tools/probe';
      await fetchJson(path, { method: 'POST' });
      message.success('已提交验证任务');
      onDone?.();
    } catch (error) {
      message.error(`验证失败：${error.message}`);
    } finally {
      setProbing(null);
    }
  };

  return (
    <Space direction="vertical" size={16} className="full">
      <Row gutter={[16, 16]}>
        <Col xs={24} md={12} xl={6}><Metric title="工具数量" value={summary.tool_count ?? tools.length} prefix={<ToolOutlined />} /></Col>
        <Col xs={24} md={12} xl={6}><Metric title="已验证链路" value={summary.verified_edges || 0} tone="green" /></Col>
        <Col xs={24} md={12} xl={6}><Metric title="待验证链路" value={summary.declared_unverified_edges || 0} tone="orange" /></Col>
        <Col xs={24} md={12} xl={6}><Metric title="失败链路" value={summary.failed_edges || 0} tone={summary.failed_edges ? 'red' : 'blue'} /></Col>
      </Row>

      <ProCard
        title="能力验证中心"
        bordered
        extra={<Button icon={<ReloadOutlined />} loading={probing === '__all__'} onClick={() => runProbe(null)}>验证全部</Button>}
      >
        <Space direction="vertical" className="full" size={12}>
          <ProbeReadiness summary={summary} />
          <Row gutter={[12, 12]}>
            {(summary.recommendations || []).map((item) => (
              <Col xs={24} md={12} key={item.code}>
                <Alert
                  showIcon
                  type={item.level === 'warning' ? 'warning' : item.level === 'ok' ? 'success' : 'info'}
                  message={item.message}
                />
              </Col>
            ))}
          </Row>
        </Space>
      </ProCard>

      <ProCard title="工具验证矩阵" bordered>
        <Table
          rowKey={(row) => row.id}
          dataSource={centerTools}
          tableLayout="fixed"
          scroll={{ x: 1320 }}
          columns={[
            {
              title: '工具',
              width: 260,
              render: (_, row) => (
                <Space direction="vertical" size={1}>
                  <Text strong>{row.name}</Text>
                  <Text copyable type="secondary">{row.id}</Text>
                </Space>
              ),
            },
            { title: '分类', width: 130, render: (_, row) => <Tag>{row.category}</Tag> },
            { title: 'Payload', width: 130, render: (_, row) => <Text code>{row.payload_type}</Text> },
            { title: '风险', width: 100, render: (_, row) => <Tag color={riskColor(row.risk)}>{riskLabel(row.risk)}</Tag> },
            { title: '策略', width: 100, render: (_, row) => row.requires_policy ? <Tag color="orange">需要</Tag> : <Tag color="green">无需</Tag> },
            { title: '在线节点', width: 110, render: (_, row) => <Tag color={row.node_count ? 'green' : 'red'}>{row.node_count || 0}</Tag> },
            { title: '已验证', width: 110, render: (_, row) => <Tag color={row.verified_node_count ? 'green' : 'default'}>{row.verified_node_count || 0}</Tag> },
            {
              title: 'Probe',
              width: 180,
              render: (_, row) => <ProbeStateStrip nodes={row.nodes || []} />,
            },
            {
              title: '说明',
              render: (_, row) => <Text type="secondary">{row.summary}</Text>,
            },
            {
              title: '操作',
              width: 160,
              render: (_, row) => (
                <Space>
                  <Button size="small" onClick={() => setSelected(row)}>详情</Button>
                  <Button size="small" loading={probing === row.id} onClick={() => runProbe(row.id)}>验证</Button>
                </Space>
              ),
            },
          ]}
          expandable={{
            expandedRowRender: (row) => (
              <Space direction="vertical" className="full">
                <Space wrap>{(row.labels || []).map((label) => <Tag key={label}>{label}</Tag>)}</Space>
                <Text type="secondary">支持节点：{(row.supported_nodes || []).join(', ') || '暂无在线节点'}</Text>
                <Text type="secondary">验证状态：{row.verification_status || '-'}</Text>
                <Table
                  size="small"
                  pagination={false}
                  rowKey={(node) => node.id}
                  dataSource={row.nodes || []}
                  columns={[
                    { title: '节点', render: (_, node) => node.name || node.id },
                    { title: '系统', render: (_, node) => `${node.os || '-'} / ${node.arch || '-'}` },
                    { title: '验证', render: (_, node) => <Tag color={probeStateColor(node.verification_status)}>{probeStateLabel(node.verification_status)}</Tag> },
                    { title: '最后验证', render: (_, node) => formatTime(node.probe?.metadata?.updated_at) || '-' },
                    { title: '错误', render: (_, node) => compactJson(node.probe?.status?.error) },
                  ]}
                />
              </Space>
            ),
          }}
        />
      </ProCard>

      <ProCard title="电脑覆盖" bordered>
        <Table
          rowKey={(row) => row.metadata.id}
          dataSource={workbenchRows}
          tableLayout="fixed"
          scroll={{ x: 1120 }}
          columns={[
            {
              title: '电脑',
              width: 240,
              render: (_, row) => (
                <Space direction="vertical" size={1}>
                  <Text strong>{row.metadata.name}</Text>
                  <Text type="secondary">{row.metadata.id}</Text>
                </Space>
              ),
            },
            { title: '状态', width: 100, render: (_, row) => <Tag>{stateLabel(row.status.state)}</Tag> },
            { title: '系统', width: 120, render: (_, row) => row.spec.os || '-' },
            { title: '已验证', width: 100, render: (_, row) => <Tag color="green">{row.status.verified_tools || 0}</Tag> },
            { title: '失败', width: 90, render: (_, row) => <Tag color={row.status.failed_tools ? 'red' : 'default'}>{row.status.failed_tools || 0}</Tag> },
            { title: '待验证', width: 100, render: (_, row) => <Tag color={row.status.unverified_tools ? 'orange' : 'default'}>{row.status.unverified_tools || 0}</Tag> },
            {
              title: '工具',
              render: (_, row) => (
                <Space wrap>
                  {(row.tools || []).slice(0, 10).map((item) => (
                    <Tag key={`${item.tool_id}:${item.node?.id}`} color={probeStateColor(item.verification_status)}>
                      {item.tool_id}
                    </Tag>
                  ))}
                  {(row.tools || []).length > 10 && <Tag>+{(row.tools || []).length - 10}</Tag>}
                </Space>
              ),
            },
          ]}
          expandable={{
            expandedRowRender: (row) => (
              <Table
                size="small"
                pagination={false}
                rowKey={(item) => `${item.tool_id}:${item.node?.id}`}
                dataSource={row.tools || []}
                columns={[
                  { title: '工具', render: (_, item) => <Text code>{item.tool_id}</Text> },
                  { title: '能力', dataIndex: 'capability' },
                  { title: '节点', render: (_, item) => item.node?.name || item.node?.id },
                  { title: '验证', render: (_, item) => <Tag color={probeStateColor(item.verification_status)}>{probeStateLabel(item.verification_status)}</Tag> },
                  { title: '最近任务', render: (_, item) => item.probe?.metadata?.task_id || '-' },
                ]}
              />
            ),
          }}
        />
      </ProCard>

      <ToolDetailModal tool={selected} onClose={() => setSelected(null)} onDone={onDone} />
    </Space>
  );
}

function ProbeReadiness({ summary }) {
  const readiness = summary.readiness || 'needs_probe';
  const meta = {
    verified: { color: 'green', label: '已验证' },
    probing: { color: 'blue', label: '验证中' },
    attention_required: { color: 'red', label: '需要处理' },
    needs_probe: { color: 'orange', label: '需要验证' },
  }[readiness] || { color: 'default', label: readiness };
  return (
    <section className="probe-readiness">
      <div>
        <Text className="eyebrow">Tool Probe Center</Text>
        <Title level={5}>工具真实可用性</Title>
        <Text type="secondary">
          Hub 通过 Probe 把“节点声明有能力”升级成“运行时确认可用”，调度器会优先选择已验证链路。
        </Text>
      </div>
      <Space>
        <Tag color={meta.color}>{meta.label}</Tag>
        <Tag>工具 {summary.tool_count || 0}</Tag>
        <Tag>电脑 {summary.workbench_count || 0}</Tag>
        <Tag>注册工具 {summary.registered_node_tool_count || 0}</Tag>
      </Space>
    </section>
  );
}

function ProbeStateStrip({ nodes }) {
  const counts = nodes.reduce((acc, node) => {
    const key = node.verification_status || 'declared_unverified';
    acc[key] = (acc[key] || 0) + 1;
    return acc;
  }, {});
  const entries = ['verified', 'pending', 'failed', 'declared_unverified', 'unsupported']
    .filter((key) => counts[key]);
  if (!entries.length) return <Text type="secondary">无节点</Text>;
  return (
    <Space size={4} wrap>
      {entries.map((key) => (
        <Tag key={key} color={probeStateColor(key)}>
          {probeStateLabel(key)} {counts[key]}
        </Tag>
      ))}
    </Space>
  );
}

function ToolDetailModal({ tool, onClose, onDone }) {
  const [probing, setProbing] = useState(null);
  const probeNode = async (nodeId) => {
    if (!tool) return;
    setProbing(nodeId);
    try {
      await fetchJson(`/tools/${tool.id}/nodes/${nodeId}/probe`, { method: 'POST' });
      message.success('Probe 已提交');
      onDone?.();
    } catch (error) {
      message.error(`Probe 失败：${error.message}`);
    } finally {
      setProbing(null);
    }
  };

  return (
    <Modal
      title={tool ? `工具详情：${tool.name}` : '工具详情'}
      open={Boolean(tool)}
      onCancel={onClose}
      footer={null}
      width={980}
    >
      {tool ? (
        <Space direction="vertical" size={14} className="full">
          <Row gutter={12}>
            <Col span={6}><Card size="small"><Statistic title="工具 ID" value={tool.id} /></Card></Col>
            <Col span={6}><Card size="small"><Statistic title="能力" value={tool.capability} /></Card></Col>
            <Col span={6}><Card size="small"><Statistic title="风险" value={riskLabel(tool.risk)} /></Card></Col>
            <Col span={6}><Card size="small"><Statistic title="已验证节点" value={tool.verified_node_count || 0} /></Card></Col>
          </Row>
          <ProCard title="输入 Schema" bordered>
            <pre className="result-json">{JSON.stringify(tool.input_schema, null, 2)}</pre>
          </ProCard>
          <ProCard title="输出 Schema" bordered>
            <pre className="result-json">{JSON.stringify(tool.output_schema, null, 2)}</pre>
          </ProCard>
          <ProCard title="示例 Payload" bordered>
            <pre className="result-json">{JSON.stringify(tool.examples?.[0] || {}, null, 2)}</pre>
          </ProCard>
          <ProCard title="支持节点" bordered>
            <Table
              size="small"
              pagination={false}
              rowKey={(row) => row.id}
              dataSource={tool.nodes || []}
              columns={[
                { title: '节点', render: (_, row) => <Text strong>{row.name || row.id}</Text> },
                { title: 'ID', dataIndex: 'id' },
                { title: '系统', render: (_, row) => `${row.os || '-'} / ${row.arch || '-'}` },
                { title: 'CPU', render: (_, row) => `${row.cpu_cores || 0} 核` },
                { title: '内存', render: (_, row) => formatMb(row.memory_mb) },
                { title: 'Worker', render: (_, row) => row.worker_target || row.worker_version || '-' },
                { title: '验证', render: (_, row) => <Tag color={probeStateColor(row.verification_status)}>{probeStateLabel(row.verification_status)}</Tag> },
                { title: '更新时间', render: (_, row) => formatTime(row.probe?.metadata?.updated_at) || '-' },
                {
                  title: '操作',
                  render: (_, row) => (
                    <Button size="small" loading={probing === row.id} onClick={() => probeNode(row.id)}>
                      Probe
                    </Button>
                  ),
                },
              ]}
            />
          </ProCard>
        </Space>
      ) : null}
    </Modal>
  );
}

function NodeTools({ nodeTools, nodes, onDone }) {
  const [open, setOpen] = useState(false);
  const [probing, setProbing] = useState(null);
  const [nodeId, setNodeId] = useState(nodes[0]?.metadata?.id || '');
  const [jsonText, setJsonText] = useState(JSON.stringify({
    tools: [
      {
        tool_id: 'demo.hello',
        name: 'Demo Hello Tool',
        version: '0.1.0',
        executor: 'plugin:hello-plugin',
        status: 'available',
        confidence: 'declared',
        input_schema: {
          type: 'object',
          properties: { name: { type: 'string' } },
        },
        output_schema: { type: 'object' },
        constraints: {},
        labels: ['compute', 'plugin', 'tool:demo.hello'],
        default_verify: {
          rules: [
            { path: 'result.type', op: 'exists', description: '动态工具必须回写结构化结果' },
          ],
        },
        probe: {
          enabled: true,
          interval_seconds: 300,
          payload: { name: 'AgentGrid' },
          verify: {
            rules: [
              { path: 'result.output.ok', op: 'eq', value: true, description: '插件返回 ok=true' },
            ],
          },
        },
      },
    ],
  }, null, 2));
  const [saving, setSaving] = useState(false);

  const rows = nodeTools.flatMap((tool) => (tool.nodes || []).map((node) => ({
    key: `${tool.tool_id}:${node.node_id}`,
    tool,
    node,
  })));

  const register = async () => {
    setSaving(true);
    try {
      const body = JSON.parse(jsonText || '{}');
      await fetchJson(`/nodes/${nodeId}/tools`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(body),
      });
      message.success('节点工具已注册');
      setOpen(false);
      onDone();
    } catch (error) {
      message.error(`注册失败：${error.message}`);
    } finally {
      setSaving(false);
    }
  };

  const runProbe = async (toolId, targetNodeId) => {
    const key = toolId ? (targetNodeId ? `${toolId}:${targetNodeId}` : toolId) : '__all__';
    setProbing(key);
    try {
      const path = !toolId
        ? '/node-tools/probe'
        : targetNodeId
          ? `/node-tools/${toolId}/nodes/${targetNodeId}/probe`
          : `/node-tools/${toolId}/probe`;
      await fetchJson(path, { method: 'POST' });
      message.success('Probe 已提交');
      onDone();
    } catch (error) {
      message.error(`Probe 失败：${error.message}`);
    } finally {
      setProbing(null);
    }
  };

  return (
    <Space direction="vertical" size={16} className="full">
      <Row gutter={[16, 16]}>
        <Col xs={24} md={12} xl={6}><Metric title="动态工具" value={nodeTools.length} prefix={<DeploymentUnitOutlined />} /></Col>
        <Col xs={24} md={12} xl={6}><Metric title="可用注册" value={rows.length} /></Col>
        <Col xs={24} md={12} xl={6}><Metric title="在线节点覆盖" value={new Set(rows.map((row) => row.node.node_id)).size} /></Col>
        <Col xs={24} md={12} xl={6}><Metric title="执行器类型" value={new Set(nodeTools.map((tool) => tool.executor || 'plugin')).size} /></Col>
      </Row>

      <ProCard
        title="节点工具注册中心"
        bordered
        extra={<Space>
          <Button icon={<ReloadOutlined />} onClick={() => runProbe(null, null)} loading={probing === '__all__'}>全部 Probe</Button>
          <Button type="primary" icon={<ToolOutlined />} onClick={() => setOpen(true)}>注册工具</Button>
        </Space>}
      >
        <Table
          rowKey={(row) => row.tool_id}
          dataSource={nodeTools}
          tableLayout="fixed"
          scroll={{ x: 1260 }}
          columns={[
            {
              title: '工具 / 插件',
              width: 260,
              render: (_, row) => (
                <Space direction="vertical" size={1}>
                  <Text strong>{row.name}</Text>
                  <Text copyable code>{row.tool_id}</Text>
                  {row.plugin_id && <Tag color="purple">{row.plugin_id}</Tag>}
                </Space>
              ),
            },
            { title: '版本', width: 100, dataIndex: 'version' },
            { title: '执行器', width: 180, render: (_, row) => <Text code>{row.executor}</Text> },
            { title: '状态', width: 100, render: (_, row) => <Tag color={row.status === 'available' ? 'green' : 'red'}>{row.status === 'available' ? '可用' : '不可用'}</Tag> },
            { title: 'Probe', width: 120, render: (_, row) => <Tag color={probeStateColor(row.probe_state?.state)}>{probeStateLabel(row.probe_state?.state)}</Tag> },
            { title: '可信来源', width: 120, dataIndex: 'confidence' },
            { title: '可用节点', width: 110, render: (_, row) => <Tag color={row.node_count ? 'green' : 'red'}>{row.node_count || 0}</Tag> },
            {
              title: '节点',
              render: (_, row) => (
                <Space wrap>
                  {(row.nodes || []).map((node) => (
                    <Tag key={node.node_id} color="blue">{node.name || node.node_id}</Tag>
                  ))}
                </Space>
              ),
            },
            {
              title: '操作',
              width: 120,
              render: (_, row) => (
                <Button size="small" loading={probing === row.tool_id} onClick={() => runProbe(row.tool_id)}>
                  Probe
                </Button>
              ),
            },
          ]}
          expandable={{
            expandedRowRender: (row) => (
              <Row gutter={16}>
                <Col span={12}>
                  <Text strong>输入 Schema</Text>
                  <pre className="result-json">{JSON.stringify(row.input_schema || {}, null, 2)}</pre>
                </Col>
                <Col span={12}>
                  <Text strong>默认验收</Text>
                  <pre className="result-json">{JSON.stringify(row.default_verify || {}, null, 2)}</pre>
                </Col>
                <Col span={24}>
                  <Text strong>插件清单</Text>
                  <pre className="result-json">{JSON.stringify(row.plugin_manifest || row.metadata?.manifest || {}, null, 2)}</pre>
                </Col>
              </Row>
            ),
          }}
        />
      </ProCard>

      <ProCard title="节点注册明细" bordered>
        <Table
          rowKey={(row) => row.key}
          dataSource={rows}
          tableLayout="fixed"
          scroll={{ x: 960 }}
          columns={[
            { title: '工具 ID', width: 220, render: (_, row) => <Text copyable code>{row.tool.tool_id}</Text> },
            { title: '节点', width: 180, render: (_, row) => <Text strong>{row.node.name || row.node.node_id}</Text> },
            { title: '节点 ID', width: 180, render: (_, row) => <Text copyable>{row.node.node_id}</Text> },
            { title: '系统', width: 120, render: (_, row) => `${row.node.os || '-'} / ${row.node.arch || '-'}` },
            { title: '执行器', width: 180, render: (_, row) => <Text code>{row.node.executor || row.tool.executor}</Text> },
            { title: 'Probe', width: 120, render: (_, row) => <Tag color={probeStateColor(row.node.probe_state)}>{probeStateLabel(row.node.probe_state)}</Tag> },
            { title: '最后验证', width: 170, render: (_, row) => formatTime(row.node.last_probe_at) },
            { title: '置信度', width: 110, render: (_, row) => row.node.confidence || row.tool.confidence || '-' },
            { title: '约束', render: (_, row) => compactJson(row.node.constraints || {}) },
            {
              title: '操作',
              width: 110,
              fixed: 'right',
              render: (_, row) => (
                <Button size="small" loading={probing === row.key} onClick={() => runProbe(row.tool.tool_id, row.node.node_id)}>
                  Probe
                </Button>
              ),
            },
          ]}
        />
      </ProCard>

      <Modal
        title="注册节点工具"
        open={open}
        onCancel={() => setOpen(false)}
        onOk={register}
        confirmLoading={saving}
        width={940}
      >
        <Form layout="vertical">
          <Form.Item label="目标节点">
            <Select
              value={nodeId}
              onChange={setNodeId}
              options={nodes.map((node) => ({
                value: node.metadata.id,
                label: `${node.metadata.name} / ${node.metadata.id}`,
              }))}
            />
          </Form.Item>
          <Form.Item label="工具声明 JSON">
            <Input.TextArea
              rows={20}
              className="json-editor"
              value={jsonText}
              onChange={(event) => setJsonText(event.target.value)}
            />
          </Form.Item>
        </Form>
      </Modal>
    </Space>
  );
}

function AgentRuntime({ manifest }) {
  const tools = manifest.tools || [];
  const commandExample = `agentgrid runtime submit \\
  --tool command.run \\
  --payload '{"type":"command","program":"hostname","args":[],"working_dir":null,"timeout_seconds":30}' \\
  --wait`;
  const restExample = {
    tool_id: 'command.run',
    title: 'hostname',
    payload: {
      type: 'command',
      program: 'hostname',
      args: [],
      working_dir: null,
      timeout_seconds: 30,
    },
    verify: { presets: ['command.exit_zero'] },
  };

  return (
    <Space direction="vertical" size={16} className="full">
      <Row gutter={[16, 16]}>
        <Col xs={24} md={12} xl={6}><Metric title="Runtime" value={manifest.runtime?.name || 'AgentGrid'} prefix={<ApiOutlined />} /></Col>
        <Col xs={24} md={12} xl={6}><Metric title="工具契约" value={tools.length} /></Col>
        <Col xs={24} md={12} xl={6}><Metric title="事件通道" value={(manifest.runtime?.event_transport || 'sse').toUpperCase()} /></Col>
        <Col xs={24} md={12} xl={6}><Metric title="协议版本" value={manifest.api_version || '-'} /></Col>
      </Row>

      <ProCard title="AI Runtime Manifest" bordered>
        <Row gutter={16}>
          <Col span={12}>
            <Space direction="vertical" className="full">
              <Text strong>核心入口</Text>
              <Text copyable code>GET /api/agent-runtime/manifest</Text>
              <Text copyable code>POST /api/agent-runtime/tasks</Text>
              <Text copyable code>GET /api/agent-runtime/tasks/{'{task_id}'}</Text>
              <Text copyable code>GET /api/agent-runtime/tasks/{'{task_id}'}/events</Text>
            </Space>
          </Col>
          <Col span={12}>
            <Space wrap>
              {Object.entries(manifest.capabilities || {}).map(([key, enabled]) => (
                <Tag key={key} color={enabled ? 'green' : 'default'}>{key}</Tag>
              ))}
            </Space>
          </Col>
        </Row>
      </ProCard>

      <ProCard title="提交示例" bordered>
        <Row gutter={16}>
          <Col span={12}>
            <Text strong>CLI</Text>
            <pre className="result-json">{commandExample}</pre>
          </Col>
          <Col span={12}>
            <Text strong>REST JSON</Text>
            <pre className="result-json">{JSON.stringify(restExample, null, 2)}</pre>
          </Col>
        </Row>
      </ProCard>

      <ProCard title="Tool Contract" bordered>
        <Table
          rowKey={(row) => row.id}
          dataSource={tools}
          tableLayout="fixed"
          scroll={{ x: 1100 }}
          columns={[
            { title: '工具', width: 220, render: (_, row) => <Text strong>{row.name}</Text> },
            { title: 'ID', width: 180, render: (_, row) => <Text copyable code>{row.id}</Text> },
            { title: '契约版本', width: 150, dataIndex: 'contract_version' },
            { title: '默认验收', width: 220, render: (_, row) => compactJson(row.default_verify) },
            { title: '在线节点', width: 110, render: (_, row) => <Tag color={row.node_count ? 'green' : 'red'}>{row.node_count || 0}</Tag> },
            { title: '已验证', width: 110, render: (_, row) => <Tag color={row.verified_node_count ? 'green' : 'default'}>{row.verified_node_count || 0}</Tag> },
            { title: '风险', width: 90, render: (_, row) => <Tag color={riskColor(row.risk)}>{riskLabel(row.risk)}</Tag> },
          ]}
          expandable={{
            expandedRowRender: (row) => (
              <pre className="result-json">{JSON.stringify(row.tool_contract || row, null, 2)}</pre>
            ),
          }}
        />
      </ProCard>
    </Space>
  );
}

function RuntimeStandard({ standard }) {
  const metadata = standard.metadata || {};
  const entrypoints = standard.entrypoints || {};
  const capabilities = standard.capability_registry?.capabilities || [];
  const positioning = standard.positioning || {};
  const workbenches = standard.workbench_standard?.items || [];
  const devices = standard.device_standard?.items || [];
  const evidenceTypes = standard.evidence_standard?.evidence_types || [];
  const runbooks = standard.runbook_standard?.runbook_types || [];
  const mobileSdk = standard.mobile_sdk_standard || {};
  const mobileMethods = mobileSdk.required_methods || [];
  const mobileScreens = mobileSdk.recommended_mobile_screens || [];
  const marketplace = standard.capability_marketplace?.items || [];
  const states = standard.task_state_machine?.states || [];
  const contracts = standard.tool_contracts || [];
  const coreStandards = [
    {
      key: 'capability_graph_standard',
      title: '能力图谱',
      item: standard.capability_graph_standard,
      metric: standard.capability_graph_standard?.snapshot?.counts?.tool_count,
      metricLabel: '工具',
    },
    {
      key: 'execution_contract_standard',
      title: '执行契约',
      item: standard.execution_contract_standard,
      metric: standard.execution_contract_standard?.families?.length,
      metricLabel: '契约',
    },
    {
      key: 'evidence_pipeline_standard',
      title: '证据流水线',
      item: standard.evidence_pipeline_standard,
      metric: standard.evidence_pipeline_standard?.current_snapshot?.artifact_count,
      metricLabel: '产物',
    },
    {
      key: 'probe_engine_standard',
      title: 'Probe 引擎',
      item: standard.probe_engine_standard,
      metric: standard.probe_engine_standard?.current_snapshot?.registered_node_tools,
      metricLabel: '节点工具',
    },
    {
      key: 'placement_engine_standard',
      title: '调度约束',
      item: standard.placement_engine_standard,
      metric: standard.placement_engine_standard?.constraint_types?.hard?.length,
      metricLabel: '硬约束',
    },
    {
      key: 'task_intent_standard',
      title: '任务意图',
      item: standard.task_intent_standard,
      metric: standard.task_intent_standard?.examples?.length,
      metricLabel: '示例',
    },
    {
      key: 'artifact_store_standard',
      title: '产物仓库 v2',
      item: standard.artifact_store_standard,
      metric: standard.artifact_store_standard?.current_snapshot?.artifact_count,
      metricLabel: '产物',
    },
    {
      key: 'event_timeline_standard',
      title: '事件时间线',
      item: standard.event_timeline_standard,
      metric: standard.event_timeline_standard?.current_snapshot?.recent_events?.length,
      metricLabel: '事件',
    },
  ];

  return (
    <Space direction="vertical" size={16} className="full">
      <Row gutter={[16, 16]}>
        <Col xs={24} md={12} xl={6}><Metric title="标准版本" value={standard.api_version || '-'} prefix={<ProfileOutlined />} /></Col>
        <Col xs={24} md={12} xl={6}><Metric title="工位" value={workbenches.length} /></Col>
        <Col xs={24} md={12} xl={6}><Metric title="设备/工具口" value={devices.length} /></Col>
        <Col xs={24} md={12} xl={6}><Metric title="移动 SDK 方法" value={mobileMethods.length} prefix={<MobileOutlined />} /></Col>
      </Row>

      <ProCard title="产品定位" bordered>
        <Space direction="vertical" className="full">
          <Title level={4}>{positioning.one_sentence || 'AgentGrid 是 AI 操作真实机器和工位的调度层。'}</Title>
          <Row gutter={16}>
            <Col span={8}>
              <Text strong>主战场</Text>
              <Space wrap className="doc-tags">
                {(positioning.primary_market || []).map((item) => <Tag color="blue" key={item}>{item}</Tag>)}
              </Space>
            </Col>
            <Col span={8}>
              <Text strong>杀手场景</Text>
              <Space direction="vertical" size={4} className="full">
                {(positioning.killer_scenarios || []).map((item) => (
                  <Text key={item.id}>{item.name}</Text>
                ))}
              </Space>
            </Col>
            <Col span={8}>
              <Text strong>不做什么</Text>
              <Space wrap className="doc-tags">
                {(positioning.anti_positioning || []).map((item) => <Tag key={item}>{item}</Tag>)}
              </Space>
            </Col>
          </Row>
        </Space>
      </ProCard>

      <ProCard title={metadata.name || 'AgentGrid Runtime Standard'} bordered>
        <Row gutter={16}>
          <Col span={12}>
            <Text strong>AgentGrid 边界内</Text>
            <Space wrap className="doc-tags">
              {(metadata.boundary?.included || []).map((item) => <Tag color="green" key={item}>{item}</Tag>)}
            </Space>
          </Col>
          <Col span={12}>
            <Text strong>不属于 AgentGrid</Text>
            <Space wrap className="doc-tags">
              {(metadata.boundary?.excluded || []).map((item) => <Tag key={item}>{item}</Tag>)}
            </Space>
          </Col>
        </Row>
      </ProCard>

      <Row gutter={16}>
        <Col span={12}>
          <ProCard title="AI 工位标准" bordered>
            <Table
              size="small"
              pagination={false}
              rowKey={(row) => row.id}
              dataSource={workbenches}
              columns={[
                { title: '工位', render: (_, row) => <Text strong>{row.name || row.id}</Text> },
                { title: '类型', render: (_, row) => <Tag color={workbenchColor(row.type)}>{workbenchLabel(row.type)}</Tag> },
                { title: '状态', render: (_, row) => <Tag>{stateLabel(row.state)}</Tag>, width: 90 },
                { title: '能力', render: (_, row) => (row.capabilities || []).slice(0, 5).map((id) => <Tag key={id}>{id}</Tag>) },
              ]}
            />
          </ProCard>
        </Col>
        <Col span={12}>
          <ProCard title="设备与工具口" bordered>
            <Table
              size="small"
              pagination={false}
              rowKey={(row) => row.id}
              dataSource={devices}
              columns={[
                { title: '设备', render: (_, row) => <Text code>{row.id}</Text> },
                { title: '类型', render: (_, row) => <Tag>{deviceLabel(row.type)}</Tag>, width: 110 },
                { title: '节点', dataIndex: 'node_id', width: 170 },
                { title: '证据', render: (_, row) => (row.evidence || []).map((id) => <Tag key={id}>{evidenceLabel(id)}</Tag>) },
              ]}
            />
          </ProCard>
        </Col>
      </Row>

      <Row gutter={16}>
        <Col span={12}>
          <ProCard title="证据标准" bordered>
            <Table
              size="small"
              pagination={false}
              rowKey={(row) => row.id}
              dataSource={evidenceTypes}
              columns={[
                { title: '证据', render: (_, row) => <Text strong>{evidenceLabel(row.id)}</Text> },
                { title: '产物类型', render: (_, row) => row.artifact_type || row.source || '-' },
                { title: '用于', render: (_, row) => (row.used_by || []).map((id) => <Tag key={id}>{workbenchLabel(id)}</Tag>) },
              ]}
            />
          </ProCard>
        </Col>
        <Col span={12}>
          <ProCard title="Runbook 标准" bordered>
            <Table
              size="small"
              pagination={false}
              rowKey={(row) => row.id}
              dataSource={runbooks}
              columns={[
                { title: 'Runbook', render: (_, row) => <Text strong>{row.name}</Text> },
                { title: '步骤数', render: (_, row) => row.steps?.length || 0, width: 80 },
                { title: '步骤', render: (_, row) => (row.steps || []).map((step) => <Tag key={step.id}>{step.id}</Tag>) },
              ]}
              expandable={{
                expandedRowRender: (row) => (
                  <pre className="result-json">{JSON.stringify(row.steps || [], null, 2)}</pre>
                ),
              }}
            />
          </ProCard>
        </Col>
      </Row>

      <ProCard title="Mobile SDK 标准" bordered>
        <Space direction="vertical" className="full">
          <Row gutter={16}>
            <Col xs={24} md={8}>
              <Text strong>定位</Text>
              <div className="muted-text">{mobileSdk.purpose || '手机端是 AgentGrid 总控台客户端。'}</div>
            </Col>
            <Col xs={24} md={8}>
              <Text strong>支持平台</Text>
              <Space wrap className="doc-tags">
                {(mobileSdk.platforms || []).map((item) => (
                  <Tag color="blue" key={item.id}>{item.language} · {item.id}</Tag>
                ))}
              </Space>
            </Col>
            <Col xs={24} md={8}>
              <Text strong>不属于手机端</Text>
              <Space wrap className="doc-tags">
                {(mobileSdk.role_boundary?.is_not || []).map((item) => <Tag key={item}>{item}</Tag>)}
              </Space>
            </Col>
          </Row>
          <Row gutter={16}>
            <Col xs={24} lg={12}>
              <Table
                size="small"
                pagination={false}
                rowKey={(row) => row.name}
                dataSource={mobileMethods}
                columns={[
                  { title: '方法', render: (_, row) => <Text code>{row.name}</Text>, width: 180 },
                  { title: '接口', render: (_, row) => <Text copyable code>{row.method === 'LOCAL' ? row.path : `${row.method} ${row.path}`}</Text> },
                  { title: '用途', dataIndex: 'purpose' },
                ]}
              />
            </Col>
            <Col xs={24} lg={12}>
              <Table
                size="small"
                pagination={false}
                rowKey={(row) => row.id}
                dataSource={mobileScreens}
                columns={[
                  { title: '页面', render: (_, row) => <Text strong>{row.title}</Text>, width: 130 },
                  { title: '数据', render: (_, row) => (row.data || []).map((id) => <Tag key={id}>{id}</Tag>) },
                  { title: '展示', render: (_, row) => (row.shows || []).slice(0, 4).map((id) => <Tag key={id}>{id}</Tag>) },
                ]}
              />
            </Col>
          </Row>
        </Space>
      </ProCard>

      <ProCard title="核心底层标准" bordered>
        <Table
          size="small"
          pagination={false}
          rowKey={(row) => row.key}
          dataSource={coreStandards}
          columns={[
            { title: '标准', render: (_, row) => <Text strong>{row.title}</Text>, width: 150 },
            { title: '快照', render: (_, row) => row.metric == null ? '-' : `${row.metric} ${row.metricLabel}`, width: 120 },
            { title: '接口', render: (_, row) => <Text copyable code>{entrypoints[row.key.replace('_standard', '')] || '-'}</Text> },
            { title: '定义', render: (_, row) => row.item?.definition || '-' },
          ]}
          expandable={{
            expandedRowRender: (row) => (
              <pre className="result-json">{JSON.stringify(row.item || {}, null, 2)}</pre>
            ),
          }}
        />
      </ProCard>

      <ProCard title="标准入口" bordered>
        <Table
          size="small"
          pagination={false}
          rowKey={(row) => row.key}
          dataSource={Object.entries(entrypoints).map(([key, value]) => ({ key, value }))}
          columns={[
            { title: '名称', dataIndex: 'key', width: 240 },
            { title: '接口', render: (_, row) => <Text copyable code>{row.value}</Text> },
          ]}
        />
      </ProCard>

      <Row gutter={16}>
        <Col span={12}>
          <ProCard title="能力注册中心" bordered>
            <Table
              size="small"
              pagination={false}
              rowKey={(row) => row.id}
              dataSource={capabilities}
              columns={[
                { title: '能力', render: (_, row) => <Text code>{row.id}</Text> },
                { title: '工具', render: (_, row) => (row.tool_ids || []).map((id) => <Tag key={id}>{id}</Tag>) },
                { title: '在线节点', dataIndex: 'node_count', width: 100 },
              ]}
            />
          </ProCard>
        </Col>
        <Col span={12}>
          <ProCard title="任务状态机" bordered>
            <Table
              size="small"
              pagination={false}
              rowKey={(row) => row.id}
              dataSource={states}
              columns={[
                { title: '状态', render: (_, row) => <Tag color={row.terminal ? 'green' : 'blue'}>{stateLabel(row.id)}</Tag> },
                { title: '终态', render: (_, row) => row.terminal ? '是' : '否', width: 80 },
                { title: '说明', dataIndex: 'description' },
              ]}
            />
          </ProCard>
        </Col>
      </Row>

      <ProCard title="完整标准 JSON" bordered>
        <pre className="result-json execution-record-json">{JSON.stringify(standard, null, 2)}</pre>
      </ProCard>
    </Space>
  );
}

function Tasks({ tasks, onOpenTask }) {
  return (
    <ProCard title="任务中心" bordered>
      <Table
        rowKey={(row) => row.metadata.id}
        dataSource={tasks}
        columns={[
          { title: '任务', render: (_, row) => <Text strong>{row.spec.title}</Text> },
          { title: '负责人', dataIndex: ['spec', 'owner'] },
          { title: '状态', render: (_, row) => <Tag>{stateLabel(row.status.state)}</Tag> },
          { title: '节点', render: (_, row) => row.status.leased_by_node_id || '-' },
          { title: '进度', render: (_, row) => <Progress percent={row.status.progress} size="small" /> },
          {
            title: '验收',
            render: (_, row) => <VerificationTag verification={row.status.result?.verification} />,
          },
          {
            title: '结果',
            render: (_, row) => {
              if (row.status.result) return <Tag color="green">有结果</Tag>;
              if (row.status.error) return <Tag color="red">失败</Tag>;
              return <Tag>等待</Tag>;
            },
          },
          {
            title: '操作',
            render: (_, row) => (
              <Button size="small" icon={<NodeIndexOutlined />} onClick={() => onOpenTask(row.metadata.id)}>
                详情
              </Button>
            ),
          },
        ]}
        expandable={{
          expandedRowRender: (row) => (
            <pre className="result-json">
              {JSON.stringify(row.status.result || row.status.error || row, null, 2)}
            </pre>
          ),
        }}
      />
    </ProCard>
  );
}

function TaskResults({ tasks, onOpenTask }) {
  const doneTasks = tasks.filter((task) => task.status.result || task.status.error);
  return (
    <ProCard title="任务结果详情" bordered>
      <Table
        rowKey={(row) => row.metadata.id}
        dataSource={doneTasks}
        columns={[
          { title: '任务', render: (_, row) => <Text strong>{row.spec.title}</Text> },
          { title: '状态', render: (_, row) => <Tag color={row.status.result ? 'green' : 'red'}>{stateLabel(row.status.state)}</Tag> },
          { title: '执行节点', render: (_, row) => row.status.leased_by_node_id || '-' },
          {
            title: '验收',
            render: (_, row) => <VerificationTag verification={row.status.result?.verification} />,
          },
          { title: '完成时间', render: (_, row) => formatTime(row.status.completed_at || row.metadata.updated_at) },
          {
            title: '结果类型',
            render: (_, row) => row.status.result?.type || row.status.error?.code || '-',
          },
          {
            title: '操作',
            render: (_, row) => (
              <Button size="small" icon={<NodeIndexOutlined />} onClick={() => onOpenTask(row.metadata.id)}>
                详情
              </Button>
            ),
          },
        ]}
        expandable={{
          expandedRowRender: (row) => (
            <pre className="result-json">
              {JSON.stringify(row.status.result || row.status.error, null, 2)}
            </pre>
          ),
        }}
      />
    </ProCard>
  );
}

function Jobs({ jobs, nodes, tools, onOpenTask, onDone }) {
  const [open, setOpen] = useState(false);
  const [saving, setSaving] = useState(false);
  const [planning, setPlanning] = useState(false);
  const [jobPlan, setJobPlan] = useState(null);
  const [form] = Form.useForm();
  const [payloadText, setPayloadText] = useState('{"type":"command","program":"hostname","args":[],"timeout_seconds":30}');
  const [partitionItemsText, setPartitionItemsText] = useState('[]');

  const buildJobRequest = (values) => {
    const partition = values.partition_type === 'items'
      ? { type: 'items', items: JSON.parse(partitionItemsText || '[]') }
      : values.partition_type === 'range'
        ? { type: 'range', start: Number(values.range_start || 0), end: Number(values.range_end || 0), step: Number(values.range_step || 1) }
        : { type: 'none' };
    return {
      title: values.title,
      tool_id: values.tool_id,
      payload: JSON.parse(payloadText || '{}'),
      placement: {
        node_id: values.node_id || undefined,
        os: values.os || undefined,
      },
      strategy: Number(values.shards || 1) > 1 ? {
        type: 'sharded',
        shard_count: Number(values.shards || 1),
        max_parallelism: Number(values.max_parallelism || values.shards || 1),
        payload_mode: 'inject_shard',
      } : { type: 'single' },
      partition,
      reduce: { type: values.reduce || 'summary' },
      retry_policy: {
        max_attempts: values.max_attempts || 3,
        on_node_lost: 'reschedule',
        on_process_failed: 'reschedule_if_idempotent',
      },
      checkpoint_policy: { enabled: true, mode: 'worker_reported' },
      idempotency: values.idempotency_key ? { key: values.idempotency_key, mode: 'idempotent' } : { mode: 'at_least_once' },
      created_by: 'console',
    };
  };

  const plan = async () => {
    setPlanning(true);
    try {
      const values = await form.validateFields();
      const response = await fetchJson('/jobs/plan', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(buildJobRequest(values)),
      });
      setJobPlan(response.item || response);
      message.success('预检完成');
    } catch (error) {
      message.error(`预检失败：${error.message}`);
    } finally {
      setPlanning(false);
    }
  };

  const submit = async (values) => {
    setSaving(true);
    try {
      await fetchJson('/jobs', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(buildJobRequest(values)),
      });
      message.success('Job 已提交');
      setOpen(false);
      setJobPlan(null);
      onDone();
    } catch (error) {
      message.error(`提交失败：${error.message}`);
    } finally {
      setSaving(false);
    }
  };

  return (
    <Space direction="vertical" size={16} className="full">
      <Row gutter={[16, 16]}>
        <Col xs={24} md={12} xl={6}><Metric title="Job 总数" value={jobs.length} /></Col>
        <Col xs={24} md={12} xl={6}><Metric title="运行中" value={jobs.filter((job) => job.status.state === 'running').length} /></Col>
        <Col xs={24} md={12} xl={6}><Metric title="已完成" value={jobs.filter((job) => job.status.state === 'done').length} /></Col>
        <Col xs={24} md={12} xl={6}><Metric title="失败" value={jobs.filter((job) => job.status.state === 'failed').length} /></Col>
      </Row>
      <ProCard title="集群 Job Runtime" bordered extra={<Button type="primary" icon={<DeploymentUnitOutlined />} onClick={() => setOpen(true)}>提交 Job</Button>}>
        <Table
          rowKey={(row) => row.metadata.id}
          dataSource={jobs}
          tableLayout="fixed"
          scroll={{ x: 1280 }}
          columns={[
            {
              title: 'Job',
              width: 280,
              render: (_, row) => (
                <Space direction="vertical" size={1}>
                  <Text strong>{row.spec.title}</Text>
                  <Text copyable type="secondary">{row.metadata.id}</Text>
                </Space>
              ),
            },
            { title: '状态', width: 110, render: (_, row) => <Tag color={workflowStateColor(row.status.state)}>{stateLabel(row.status.state)}</Tag> },
            { title: '工具', width: 160, render: (_, row) => <Text code>{row.spec.tool_id}</Text> },
            {
              title: '策略',
              width: 130,
              render: (_, row) => row.spec.strategy?.type === 'sharded'
                ? <Tag color="geekblue">分片 {row.spec.strategy.shard_count}</Tag>
                : <Tag>单任务</Tag>,
            },
            { title: '当前 Attempt', width: 190, render: (_, row) => <Text copyable>{row.status.current_attempt_id || '-'}</Text> },
            {
              title: '当前任务',
              width: 180,
              render: (_, row) => row.status.current_task_id ? (
                <Button size="small" onClick={() => onOpenTask(row.status.current_task_id)}>{row.status.current_task_id}</Button>
              ) : '-',
            },
            { title: 'Checkpoint', width: 180, render: (_, row) => <Text copyable>{row.status.latest_checkpoint_id || '-'}</Text> },
            {
              title: 'Reducer',
              width: 170,
              render: (_, row) => row.status.reducer_task_id ? (
                <Button size="small" onClick={() => onOpenTask(row.status.reducer_task_id)}>{row.status.reducer_task_id}</Button>
              ) : '-',
            },
            { title: '最大尝试', width: 90, render: (_, row) => row.status.max_attempts },
            { title: '更新时间', width: 170, render: (_, row) => formatTime(row.metadata.updated_at) },
          ]}
          expandable={{
            expandedRowRender: (row) => (
              <Row gutter={16}>
                <Col span={12}>
                  <Text strong>Payload</Text>
                  <pre className="result-json">{JSON.stringify(row.spec.payload, null, 2)}</pre>
                </Col>
                <Col span={12}>
                  <Text strong>策略</Text>
                  <pre className="result-json">{JSON.stringify({
                    placement: row.spec.placement,
                    strategy: row.spec.strategy,
                    partition: row.spec.strategy?.partition,
                    reduce: row.spec.reduce,
                    retry_policy: row.spec.retry_policy,
                    checkpoint_policy: row.spec.checkpoint_policy,
                    idempotency: row.spec.idempotency,
                  }, null, 2)}</pre>
                </Col>
                {row.shards?.length ? (
                  <Col span={24}>
                    <Text strong>Shards</Text>
                    <Table
                      size="small"
                      rowKey={(shard) => shard.metadata.id}
                      dataSource={row.shards}
                      pagination={false}
                      columns={[
                        { title: '#', width: 70, render: (_, shard) => shard.spec.shard_index },
                        { title: '状态', width: 110, render: (_, shard) => <Tag color={workflowStateColor(shard.status.state)}>{stateLabel(shard.status.state)}</Tag> },
                        { title: '节点', width: 150, render: (_, shard) => shard.status.node_id || '-' },
                        { title: '任务', width: 220, render: (_, shard) => shard.status.current_task_id ? <Button size="small" onClick={() => onOpenTask(shard.status.current_task_id)}>{shard.status.current_task_id}</Button> : '-' },
                        { title: '完成时间', width: 170, render: (_, shard) => formatTime(shard.status.completed_at) },
                      ]}
                    />
                  </Col>
                ) : null}
                {row.status.result ? (
                  <Col span={24}>
                    <Text strong>最终结果</Text>
                    <pre className="result-json">{JSON.stringify(row.status.result, null, 2)}</pre>
                  </Col>
                ) : null}
              </Row>
            ),
          }}
        />
      </ProCard>
      <Modal
        title="提交集群 Job"
        open={open}
        onCancel={() => setOpen(false)}
        footer={[
          <Button key="cancel" onClick={() => setOpen(false)}>取消</Button>,
          <Button key="plan" loading={planning} onClick={plan}>预检</Button>,
          <Button key="submit" type="primary" loading={saving} onClick={() => form.submit()}>提交</Button>,
        ]}
        width={1040}
      >
        <Form form={form} layout="vertical" onFinish={submit} initialValues={{ title: 'hostname Job', tool_id: 'command.run', max_attempts: 3, shards: 1, max_parallelism: 1, reduce: 'summary' }}>
          <Row gutter={12}>
            <Col span={12}><Form.Item name="title" label="标题" rules={[{ required: true }]}><Input /></Form.Item></Col>
            <Col span={12}>
              <Form.Item name="tool_id" label="工具" rules={[{ required: true }]}>
                <Select showSearch options={tools.map((tool) => ({ value: tool.id, label: `${tool.name} / ${tool.id}` }))} />
              </Form.Item>
            </Col>
          </Row>
          <Row gutter={12}>
            <Col span={6}><Form.Item name="node_id" label="指定节点"><Select allowClear options={nodes.map((node) => ({ value: node.metadata.id, label: node.metadata.name }))} /></Form.Item></Col>
            <Col span={6}><Form.Item name="os" label="指定系统"><Select allowClear options={[{ value: 'linux', label: 'Linux' }, { value: 'mac', label: 'macOS' }, { value: 'windows', label: 'Windows' }]} /></Form.Item></Col>
            <Col span={6}><Form.Item name="max_attempts" label="最大尝试次数"><InputNumber min={1} max={20} className="full" /></Form.Item></Col>
            <Col span={6}><Form.Item name="idempotency_key" label="幂等键"><Input placeholder="可选，建议副作用任务填写" /></Form.Item></Col>
          </Row>
          <Row gutter={12}>
            <Col span={12}><Form.Item name="shards" label="分片数量"><InputNumber min={1} max={1024} className="full" /></Form.Item></Col>
            <Col span={12}><Form.Item name="max_parallelism" label="最大并行"><InputNumber min={1} max={1024} className="full" /></Form.Item></Col>
          </Row>
          <Form.Item name="reduce" label="汇总方式">
            <Select options={[
              { value: 'summary', label: 'Summary' },
              { value: 'stdout_concat', label: 'Stdout concat' },
              { value: 'json_array', label: 'JSON array' },
            ]} />
          </Form.Item>
          <Form.Item name="partition_type" label="分区方式" initialValue="none">
            <Select options={[
              { value: 'none', label: '固定分片' },
              { value: 'items', label: 'Items 分区' },
              { value: 'range', label: 'Range 分区' },
            ]} />
          </Form.Item>
          <Form.Item noStyle shouldUpdate={(prev, next) => prev.partition_type !== next.partition_type}>
            {({ getFieldValue }) => getFieldValue('partition_type') === 'items' ? (
              <Form.Item label="Items JSON">
                <Input.TextArea rows={4} className="json-editor" value={partitionItemsText} onChange={(event) => setPartitionItemsText(event.target.value)} />
              </Form.Item>
            ) : null}
          </Form.Item>
          <Form.Item noStyle shouldUpdate={(prev, next) => prev.partition_type !== next.partition_type}>
            {({ getFieldValue }) => getFieldValue('partition_type') === 'range' ? (
              <Row gutter={12}>
                <Col span={8}><Form.Item name="range_start" label="Range start"><InputNumber className="full" /></Form.Item></Col>
                <Col span={8}><Form.Item name="range_end" label="Range end"><InputNumber className="full" /></Form.Item></Col>
                <Col span={8}><Form.Item name="range_step" label="Range step" initialValue={1}><InputNumber min={1} className="full" /></Form.Item></Col>
              </Row>
            ) : null}
          </Form.Item>
          <Form.Item label="Payload JSON">
            <Input.TextArea rows={12} className="json-editor" value={payloadText} onChange={(event) => setPayloadText(event.target.value)} />
          </Form.Item>
        </Form>
        {jobPlan ? (
          <ProCard title="调度预检结果" bordered>
            <Row gutter={[12, 12]}>
              <Col span={6}><Metric title="可运行" value={jobPlan.can_run ? '是' : '否'} /></Col>
              <Col span={6}><Metric title="选中节点" value={jobPlan.selected_node_id || '-'} /></Col>
              <Col span={6}><Metric title="可用节点" value={jobPlan.eligible_nodes?.length || 0} /></Col>
              <Col span={6}><Metric title="预估 Attempt" value={jobPlan.execution_shape?.estimated_attempts || 1} /></Col>
              <Col span={24}>
                <Space wrap>
                  {(jobPlan.warnings || []).map((warning) => (
                    <Tag key={warning.code} color={warning.severity === 'high' ? 'red' : warning.severity === 'medium' ? 'orange' : 'blue'}>
                      {warning.message}
                    </Tag>
                  ))}
                </Space>
              </Col>
              <Col span={12}>
                <Text strong>候选节点</Text>
                <Table
                  size="small"
                  rowKey={(row) => row.node_id}
                  dataSource={jobPlan.eligible_nodes || []}
                  pagination={false}
                  columns={[
                    { title: '节点', render: (_, row) => row.node_name || row.node_id },
                    { title: '评分', render: (_, row) => Number(row.score || 0).toFixed(2) },
                    { title: '槽位', dataIndex: 'available_slots' },
                    { title: '可信', render: (_, row) => <Tag color={probeStateColor(row.trust?.state)}>{probeStateLabel(row.trust?.state)}</Tag> },
                  ]}
                />
              </Col>
              <Col span={12}>
                <Text strong>可靠性合约</Text>
                <pre className="result-json">{JSON.stringify(jobPlan.reliability || {}, null, 2)}</pre>
              </Col>
            </Row>
          </ProCard>
        ) : null}
      </Modal>
    </Space>
  );
}

const workflowTemplates = [
  {
    key: 'healthcheck',
    name: '节点健康巡检',
    summary: '先获取主机名，再检查磁盘，再检查运行时间。',
    nodes: [
      {
        id: 'hostname',
        title: '获取主机名',
        payload: { type: 'command', program: 'hostname', args: [], timeout_seconds: 30 },
        labels: ['compute', 'command'],
      },
      {
        id: 'disk',
        title: '检查磁盘空间',
        depends_on: ['hostname'],
        payload: { type: 'command', program: 'df', args: ['-h'], timeout_seconds: 30 },
        labels: ['compute', 'command'],
      },
      {
        id: 'uptime',
        title: '检查运行时间',
        depends_on: ['disk'],
        payload: { type: 'command', program: 'uptime', args: [], timeout_seconds: 30 },
        labels: ['compute', 'command'],
      },
    ],
  },
  {
    key: 'web_probe',
    name: 'HTTP 探测流水线',
    summary: '先请求入口，再把结果交给 AgentMessage 做协作记录。',
    nodes: [
      {
        id: 'fetch_home',
        title: '请求业务入口',
        payload: { type: 'http_request', method: 'GET', url: 'https://httpbin.org/get', headers: [], body: null, timeout_seconds: 30, max_response_bytes: 65536 },
        labels: ['compute', 'http_request'],
      },
      {
        id: 'notify_agent',
        title: '发送协作消息',
        depends_on: ['fetch_home'],
        payload: { type: 'agent_message', from: 'workflow-engine', to: ['review-agent'], message_type: 'workflow.probe.completed', subject: 'HTTP 探测完成', summary: '请查看工作流结果。', payload: {} },
        labels: ['compute', 'agentmessage'],
      },
    ],
  },
  {
    key: 'repo_ci',
    name: '仓库 CI 骨架',
    summary: '检查 Git 状态，再运行测试命令。路径需要改成节点上的真实目录。',
    nodes: [
      {
        id: 'git_status',
        title: '检查 Git 状态',
        payload: { type: 'git', operation: 'status', repo_dir: '/tmp/repo' },
        labels: ['compute', 'git'],
      },
      {
        id: 'run_tests',
        title: '运行测试',
        depends_on: ['git_status'],
        payload: { type: 'command', program: 'sh', args: ['-lc', 'cargo test'], working_dir: '/tmp/repo', timeout_seconds: 600 },
        labels: ['compute', 'command'],
      },
    ],
  },
];

function Workflows({ workflows, tasks, nodes, onOpenTask, onDone }) {
  const [creating, setCreating] = useState(false);
  const [selected, setSelected] = useState(null);
  const taskMap = Object.fromEntries(tasks.map((task) => [task.metadata.id, task]));

  const startWorkflow = async (workflow) => {
    try {
      await fetchJson(`/workflows/${workflow.metadata.id}/start`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ actor: 'architect-agent' }),
      });
      message.success('工作流已启动');
      onDone();
    } catch (error) {
      message.error(`启动失败：${error.message}`);
    }
  };

  const cancelWorkflow = async (workflow) => {
    try {
      await fetchJson(`/workflows/${workflow.metadata.id}/cancel`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ actor: 'architect-agent', reason: '总控台取消工作流' }),
      });
      message.success('工作流已取消');
      onDone();
    } catch (error) {
      message.error(`取消失败：${error.message}`);
    }
  };

  return (
    <Space direction="vertical" size={16} className="full">
      <Row gutter={16}>
        <Col span={6}><Metric title="工作流总数" value={workflows.length} /></Col>
        <Col span={6}><Metric title="运行中" value={workflows.filter((item) => item.status.state === 'running').length} /></Col>
        <Col span={6}><Metric title="已完成" value={workflows.filter((item) => item.status.state === 'done').length} /></Col>
        <Col span={6}><Metric title="可用节点" value={nodes.filter((node) => node.status.state === 'online').length} /></Col>
      </Row>
      <ProCard
        title="工作流总控"
        bordered
        extra={<Button type="primary" icon={<ForkOutlined />} onClick={() => setCreating(true)}>创建工作流</Button>}
      >
        <Table
          rowKey={(row) => row.metadata.id}
          dataSource={workflows}
          columns={[
            {
              title: '工作流',
              render: (_, row) => (
                <Space direction="vertical" size={1}>
                  <Text strong>{row.spec.name}</Text>
                  <Text type="secondary">{row.spec.summary || row.metadata.id}</Text>
                </Space>
              ),
            },
            { title: '状态', width: 110, render: (_, row) => <Tag color={workflowStateColor(row.status.state)}>{stateLabel(row.status.state)}</Tag> },
            { title: '进度', width: 160, render: (_, row) => <Progress percent={row.status.progress || 0} size="small" /> },
            { title: '节点数', width: 90, render: (_, row) => row.spec.nodes?.length || 0 },
            { title: '创建人', width: 140, render: (_, row) => row.metadata.created_by },
            { title: '更新时间', width: 180, render: (_, row) => formatTime(row.metadata.updated_at) },
            {
              title: '操作',
              width: 230,
              render: (_, row) => (
                <Space wrap>
                  <Button size="small" onClick={() => setSelected(row)}>详情</Button>
                  {['draft', 'failed', 'cancelled'].includes(row.status.state) && (
                    <Button size="small" type="primary" onClick={() => startWorkflow(row)}>启动</Button>
                  )}
                  {row.status.state === 'running' && (
                    <Button size="small" danger onClick={() => cancelWorkflow(row)}>取消</Button>
                  )}
                </Space>
              ),
            },
          ]}
        />
      </ProCard>
      <WorkflowCreateModal open={creating} onClose={() => setCreating(false)} onDone={() => { setCreating(false); onDone(); }} />
      <WorkflowDetailModal workflow={selected} taskMap={taskMap} onClose={() => setSelected(null)} onOpenTask={onOpenTask} />
    </Space>
  );
}

function WorkflowCreateModal({ open, onClose, onDone }) {
  const [form] = Form.useForm();
  const [templateKey, setTemplateKey] = useState(workflowTemplates[0].key);
  const [saving, setSaving] = useState(false);
  const template = workflowTemplates.find((item) => item.key === templateKey) || workflowTemplates[0];

  useEffect(() => {
    if (!open) return;
    form.setFieldsValue({
      name: template.name,
      summary: template.summary,
      nodes_json: JSON.stringify(template.nodes, null, 2),
      auto_start: true,
    });
  }, [open, template, form]);

  const submit = async (values) => {
    setSaving(true);
    try {
      const created = await fetchJson('/workflows', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          name: values.name,
          summary: values.summary,
          created_by: 'architect-agent',
          nodes: JSON.parse(values.nodes_json || '[]'),
        }),
      });
      if (values.auto_start) {
        await fetchJson(`/workflows/${created.item.metadata.id}/start`, {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify({ actor: 'architect-agent' }),
        });
      }
      message.success(values.auto_start ? '工作流已创建并启动' : '工作流已创建');
      onDone();
    } catch (error) {
      message.error(`创建失败：${error.message}`);
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal title="创建工作流" open={open} onCancel={onClose} onOk={() => form.submit()} confirmLoading={saving} width={920}>
      <Form form={form} layout="vertical" onFinish={submit}>
        <Form.Item label="模板">
          <Select value={templateKey} onChange={setTemplateKey} options={workflowTemplates.map((item) => ({ value: item.key, label: item.name }))} />
        </Form.Item>
        <Row gutter={12}>
          <Col span={12}><Form.Item name="name" label="工作流名称" rules={[{ required: true }]}><Input /></Form.Item></Col>
          <Col span={12}><Form.Item name="summary" label="说明"><Input /></Form.Item></Col>
        </Row>
        <Form.Item name="nodes_json" label="DAG 节点 JSON" rules={[{ required: true }]}>
          <Input.TextArea rows={18} className="json-editor" />
        </Form.Item>
        <Form.Item name="auto_start" label="启动方式">
          <Select
            options={[
              { value: true, label: '创建后立即启动' },
              { value: false, label: '只保存草稿' },
            ]}
          />
        </Form.Item>
      </Form>
    </Modal>
  );
}

function WorkflowDetailModal({ workflow, taskMap, onClose, onOpenTask }) {
  const runs = workflow?.spec.runs || [];
  const nodes = workflow?.spec.nodes || [];
  const runByNode = Object.fromEntries(runs.map((run) => [run.metadata.workflow_node_id, run]));

  return (
    <Modal
      title={workflow ? `工作流详情：${workflow.spec.name}` : '工作流详情'}
      open={Boolean(workflow)}
      onCancel={onClose}
      footer={null}
      width={1080}
    >
      {workflow ? (
        <Space direction="vertical" size={14} className="full">
          <Row gutter={12}>
            <Col span={6}><Card size="small"><Statistic title="状态" value={stateLabel(workflow.status.state)} /></Card></Col>
            <Col span={6}><Card size="small"><Statistic title="进度" value={workflow.status.progress || 0} suffix="%" /></Card></Col>
            <Col span={6}><Card size="small"><Statistic title="完成节点" value={workflow.status.done_count || 0} /></Card></Col>
            <Col span={6}><Card size="small"><Statistic title="失败节点" value={workflow.status.failed_count || 0} /></Card></Col>
          </Row>
          <ProCard title="DAG 节点执行情况" bordered>
            <Table
              rowKey={(row) => row.id}
              pagination={false}
              dataSource={nodes}
              columns={[
                { title: '节点', render: (_, row) => <Text strong>{row.title}</Text> },
                { title: '依赖', render: (_, row) => row.depends_on?.length ? row.depends_on.map((item) => <Tag key={item}>{item}</Tag>) : <Tag>入口</Tag> },
                { title: '任务类型', render: (_, row) => row.payload?.type || '-' },
                {
                  title: '状态',
                  render: (_, row) => {
                    const run = runByNode[row.id];
                    return <Tag color={workflowStateColor(run?.status.state)}>{stateLabel(run?.status.state || 'pending')}</Tag>;
                  },
                },
                {
                  title: '任务',
                  render: (_, row) => {
                    const run = runByNode[row.id];
                    const task = run?.metadata.task_id ? taskMap[run.metadata.task_id] : null;
                    if (!run?.metadata.task_id) return '-';
                    return (
                      <Button size="small" onClick={() => onOpenTask(run.metadata.task_id)}>
                        {task?.spec.title || run.metadata.task_id}
                      </Button>
                    );
                  },
                },
              ]}
              expandable={{
                expandedRowRender: (row) => (
                  <pre className="result-json">{JSON.stringify({ node: row, run: runByNode[row.id] || null }, null, 2)}</pre>
                ),
              }}
            />
          </ProCard>
          <ProCard title="工作流定义" bordered>
            <pre className="result-json">{JSON.stringify(workflow.spec.nodes || [], null, 2)}</pre>
          </ProCard>
        </Space>
      ) : (
        <Text type="secondary">未选择工作流</Text>
      )}
    </Modal>
  );
}

function ExecutionRecords({ tasks, workflows }) {
  const [recordType, setRecordType] = useState('task');
  const [recordId, setRecordId] = useState(tasks[0]?.metadata.id || '');
  const [record, setRecord] = useState(null);
  const [preview, setPreview] = useState(null);
  const [loadingRecord, setLoadingRecord] = useState(false);
  const options = recordType === 'task'
    ? tasks.map((task) => ({ value: task.metadata.id, label: task.spec.title || task.metadata.id }))
    : workflows.map((workflow) => ({ value: workflow.metadata.id, label: workflow.spec.name || workflow.metadata.id }));

  useEffect(() => {
    const first = options[0]?.value || '';
    if (!recordId || !options.some((item) => item.value === recordId)) setRecordId(first);
  }, [recordType, tasks.length, workflows.length]);

  const loadRecord = async () => {
    if (!recordId) return;
    setLoadingRecord(true);
    try {
      const path = recordType === 'task'
        ? `/execution-records/tasks/${recordId}`
        : `/execution-records/workflows/${recordId}`;
      const data = await fetchJson(path);
      setRecord(data.item);
    } catch (error) {
      message.error(`读取失败：${error.message}`);
    } finally {
      setLoadingRecord(false);
    }
  };

  useEffect(() => {
    setRecord(null);
    if (recordId) loadRecord();
  }, [recordType, recordId]);

  return (
    <Space direction="vertical" size={16} className="full">
      <ProCard title="执行档案" bordered>
        <Row gutter={12}>
          <Col span={5}>
            <Select
              className="full"
              value={recordType}
              onChange={(value) => { setRecordType(value); setRecordId(''); }}
              options={[
                { value: 'task', label: '任务档案' },
                { value: 'workflow', label: '工作流档案' },
              ]}
            />
          </Col>
          <Col span={14}>
            <Select showSearch className="full" value={recordId} onChange={setRecordId} options={options} />
          </Col>
          <Col span={5}>
            <Button type="primary" loading={loadingRecord} onClick={loadRecord}>刷新档案</Button>
          </Col>
        </Row>
      </ProCard>
      {record ? (
        <>
          <Row gutter={16}>
            <Col span={6}><Metric title="类型" value={record.record_type === 'workflow' ? '工作流' : '任务'} /></Col>
            <Col span={6}><Metric title="状态" value={stateLabel(record.summary?.state)} /></Col>
            <Col span={6}><Metric title="执行节点" value={record.summary?.leased_by_node_id || '-'} /></Col>
            <Col span={6}><Metric title="生成时间" value={formatTime(record.generated_at)} /></Col>
          </Row>
          <ProCard title="执行摘要" bordered>
            <Table
              size="small"
              pagination={false}
              rowKey={(row) => row.label}
              dataSource={executionRecordRows(record)}
              columns={[
                { title: '项目', width: 140, dataIndex: 'label' },
                { title: '内容', render: (_, row) => <Text>{row.value}</Text> },
              ]}
            />
          </ProCard>
          <ProCard title="执行结果" bordered>
            <Row gutter={12}>
              <Col span={12}>
                <Text strong>stdout / result</Text>
                <pre className="result-json live-log">{record.execution?.result?.stdout || resultText(record.execution?.result)}</pre>
              </Col>
              <Col span={12}>
                <Text strong>stderr / error</Text>
                <pre className="result-json live-log">{record.execution?.result?.stderr || resultText(record.execution?.error)}</pre>
              </Col>
            </Row>
          </ProCard>
          <ProCard title="证据产物" bordered>
            <EvidenceGrid artifacts={record.execution?.artifacts || []} onPreview={setPreview} />
          </ProCard>
          <ProCard title="审计链路" bordered>
            <Table
              size="small"
              pagination={{ pageSize: 8 }}
              rowKey={(row) => row.metadata.id}
              dataSource={record.audit || []}
              columns={[
                { title: '时间', width: 180, render: (_, row) => formatTime(row.metadata.created_at) },
                { title: '事件', width: 140, render: (_, row) => <Tag color={operationEventColor(row.spec.type)}>{operationEventLabel(row.spec.type)}</Tag> },
                { title: '操作者', width: 180, render: (_, row) => row.spec.actor || '-' },
                { title: '说明', render: (_, row) => row.spec.summary || '-' },
              ]}
            />
          </ProCard>
          <ProCard title="完整执行档案 JSON" bordered>
            <pre className="result-json execution-record-json">{JSON.stringify(record, null, 2)}</pre>
          </ProCard>
          <ArtifactPreview artifact={preview} onClose={() => setPreview(null)} />
        </>
      ) : (
        <ProCard bordered><Text type="secondary">请选择任务或工作流。</Text></ProCard>
      )}
    </Space>
  );
}

function executionRecordRows(record) {
  const summary = record.summary || {};
  const execution = record.execution || {};
  const result = execution.result || {};
  return [
    ['标题', summary.title || '-'],
    ['提交人', summary.created_by || '-'],
    ['负责人', summary.owner || '-'],
    ['优先级', summary.priority || '-'],
    ['开始时间', formatTime(summary.started_at) || '-'],
    ['完成时间', formatTime(summary.completed_at) || '-'],
    ['尝试次数', summary.attempts ?? '-'],
    ['退出码', result.exit_code ?? '-'],
    ['耗时', result.duration_ms != null ? `${result.duration_ms} ms` : '-'],
    ['产物', execution.artifacts?.length ? `${execution.artifacts.length} 个` : '无'],
  ].map(([label, value]) => ({ label, value }));
}

function Artifacts({ artifacts, tasks }) {
  const taskNames = Object.fromEntries(tasks.map((task) => [task.metadata.id, task.spec.title]));
  const [preview, setPreview] = useState(null);
  return (
    <ProCard title="任务产物" bordered>
      <Table
        rowKey={(row) => row.metadata.id}
        dataSource={artifacts}
        columns={[
          { title: '产物', render: (_, row) => <Text strong>{row.spec.name}</Text> },
          { title: '类型', render: (_, row) => <Tag>{artifactTypeLabel(row.spec.type)}</Tag> },
          { title: '任务', render: (_, row) => taskNames[row.spec.task_id] || row.spec.task_id },
          { title: '节点', render: (_, row) => row.spec.node_id || '-' },
          { title: '大小', render: (_, row) => formatBytes(row.spec.size_bytes) },
          { title: '预览', render: (_, row) => <Tag>{row.spec.v2?.preview?.kind || previewKind(row)}</Tag> },
          { title: '哈希', render: (_, row) => row.spec.v2?.sha256 ? <Text copyable code>{shortHash(row.spec.v2.sha256)}</Text> : '-' },
          { title: '创建时间', render: (_, row) => formatTime(row.metadata.created_at) },
          {
            title: '操作',
            render: (_, row) => (
              <Space>
                {isImageArtifact(row) && (
                  <Button size="small" onClick={() => setPreview(row)}>
                    预览
                  </Button>
                )}
                <Button
                  size="small"
                  icon={<DownloadOutlined />}
                  disabled={!row.spec.content_base64}
                  href={artifactDownloadUrl(row)}
                >
                  下载
                </Button>
              </Space>
            ),
          },
        ]}
        expandable={{
          expandedRowRender: (row) => (
            <pre className="result-json">{JSON.stringify({ v2: row.spec.v2, metadata: row.spec.metadata }, null, 2)}</pre>
          ),
        }}
      />
      <ArtifactPreview artifact={preview} onClose={() => setPreview(null)} />
    </ProCard>
  );
}

function RemoteTerminal({ nodes }) {
  const onlineNodes = nodes.filter((node) => node.status.state === 'online');
  const [nodeId, setNodeId] = useState(onlineNodes[0]?.metadata.id);
  const [socket, setSocket] = useState(null);
  const [connected, setConnected] = useState(false);
  const [terminalText, setTerminalText] = useState('');
  const [command, setCommand] = useState('');
  const currentNode = nodes.find((node) => node.metadata.id === nodeId);

  useEffect(() => {
    if (!nodeId && onlineNodes[0]) setNodeId(onlineNodes[0].metadata.id);
  }, [nodeId, onlineNodes]);

  useEffect(() => () => {
    if (socket) socket.close();
  }, [socket]);

  const appendTerminal = (text) => {
    setTerminalText((current) => `${current}${text}`.slice(-60000));
  };

  const connect = () => {
    if (!nodeId) {
      message.warning('请先选择一个在线节点');
      return;
    }
    if (socket) socket.close();
    setTerminalText('');
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const prefix = window.location.pathname.startsWith('/agentgrid') ? '/agentgrid/api' : '/api';
    const ws = new WebSocket(`${protocol}//${window.location.host}${prefix}/terminal/ws?node_id=${encodeURIComponent(nodeId)}`);
    ws.onopen = () => {
      setConnected(true);
      appendTerminal(`[AgentGrid] 正在连接 ${nodeId} ...\n`);
    };
    ws.onmessage = (event) => {
      const data = JSON.parse(event.data);
      if (data.type === 'terminal.ready') appendTerminal(`[AgentGrid] ${data.message}\n`);
      if (data.type === 'terminal.output') appendTerminal(data.data || '');
      if (data.type === 'terminal.error') appendTerminal(`\n[AgentGrid] ${data.message}\n`);
    };
    ws.onclose = () => {
      setConnected(false);
      appendTerminal('\n[AgentGrid] 终端连接已断开\n');
    };
    ws.onerror = () => {
      setConnected(false);
      appendTerminal('\n[AgentGrid] 终端连接失败\n');
    };
    setSocket(ws);
  };

  const disconnect = () => {
    if (socket) socket.close();
    setSocket(null);
    setConnected(false);
  };

  const sendCommand = () => {
    if (!socket || socket.readyState !== WebSocket.OPEN) {
      message.warning('终端还没有连接');
      return;
    }
    const input = command.endsWith('\n') ? command : `${command}\n`;
    socket.send(JSON.stringify({ type: 'terminal.input', data: input }));
    setCommand('');
  };

  return (
    <Space direction="vertical" size={16} className="full">
      <ProCard title="远程交互式终端" bordered>
        <Row gutter={16} align="bottom">
          <Col span={8}>
            <Text type="secondary">选择节点</Text>
            <Select
              className="full"
              value={nodeId}
              onChange={setNodeId}
              options={nodes.map((node) => ({
                value: node.metadata.id,
                disabled: node.status.state !== 'online',
                label: `${node.metadata.name} / ${stateLabel(node.status.state)} / ${node.spec.os}`,
              }))}
            />
          </Col>
          <Col span={8}>
            <Text type="secondary">主机信息</Text>
            <div>
              <Tag color={connected ? 'green' : 'default'}>{connected ? '终端已连接' : '未连接'}</Tag>
              <Text>{currentNode?.spec.address || '-'}</Text>
            </div>
          </Col>
          <Col span={8}>
            <Space>
              <Button type="primary" icon={<CodeOutlined />} disabled={connected} onClick={connect}>
                连接终端
              </Button>
              <Button icon={<DisconnectOutlined />} disabled={!connected} onClick={disconnect}>
                断开
              </Button>
            </Space>
          </Col>
        </Row>
      </ProCard>

      <ProCard bordered className="terminal-card">
        <pre className="terminal-screen">{terminalText || '选择在线节点后点击“连接终端”。'}</pre>
        <Input.TextArea
          value={command}
          onChange={(event) => setCommand(event.target.value)}
          onPressEnter={(event) => {
            if (!event.shiftKey) {
              event.preventDefault();
              sendCommand();
            }
          }}
          autoSize={{ minRows: 2, maxRows: 5 }}
          placeholder="输入命令，按 Enter 发送；Shift + Enter 换行"
          disabled={!connected}
        />
        <div className="terminal-actions">
          <Button type="primary" icon={<SendOutlined />} disabled={!connected || !command.trim()} onClick={sendCommand}>
            发送
          </Button>
          <Button onClick={() => setTerminalText('')}>清空屏幕</Button>
        </div>
      </ProCard>
    </Space>
  );
}

function TaskQueue({ tasks, nodes, onOpenTask, onDone }) {
  const queueTasks = tasks.filter((task) => ['assigned', 'todo', 'in_progress', 'stopping'].includes(task.status.state));
  const grouped = nodes.map((node) => ({
    node,
    tasks: queueTasks.filter((task) => task.status.leased_by_node_id === node.metadata.id || task.spec.labels?.includes(`node:${node.metadata.id}`)),
  }));
  const unassigned = queueTasks.filter((task) => !task.status.leased_by_node_id && !(task.spec.labels || []).some((label) => label.startsWith('node:')));

  return (
    <Space direction="vertical" size={16} className="full">
      <ProCard title="队列总览" bordered>
        <Row gutter={16}>
          <Col span={6}><Metric title="等待调度" value={queueTasks.filter((task) => task.status.state === 'assigned' || task.status.state === 'todo').length} /></Col>
          <Col span={6}><Metric title="执行中" value={queueTasks.filter((task) => task.status.state === 'in_progress').length} /></Col>
          <Col span={6}><Metric title="高优先级" value={queueTasks.filter((task) => ['p0', 'urgent', 'high'].includes(task.spec.priority)).length} /></Col>
          <Col span={6}><Metric title="未绑定节点" value={unassigned.length} /></Col>
        </Row>
      </ProCard>
      <ProCard title="全部队列" bordered>
        <TaskQueueTable tasks={queueTasks} nodes={nodes} onOpenTask={onOpenTask} onDone={onDone} />
      </ProCard>
      <Row gutter={16}>
        {grouped.map(({ node, tasks: nodeTasks }) => (
          <Col span={12} key={node.metadata.id}>
            <ProCard title={`${node.metadata.name} · ${stateLabel(node.status.state)}`} bordered>
              <TaskQueueTable tasks={nodeTasks} nodes={nodes} onOpenTask={onOpenTask} onDone={onDone} compact />
            </ProCard>
          </Col>
        ))}
      </Row>
    </Space>
  );
}

function TaskQueueTable({ tasks, nodes, onOpenTask, onDone, compact }) {
  const [editing, setEditing] = useState(null);
  const control = async (task, action, extra = {}) => {
    try {
      await fetchJson(`/tasks/${task.metadata.id}/control`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ action, actor: 'architect-agent', ...extra }),
      });
      message.success('操作已提交');
      onDone();
    } catch (error) {
      message.error(`操作失败：${error.message}`);
    }
  };
  return (
    <>
      <Table
        size={compact ? 'small' : 'middle'}
        rowKey={(row) => row.metadata.id}
        dataSource={tasks}
        pagination={compact ? false : { pageSize: 12 }}
        columns={[
          { title: '任务', render: (_, row) => <Text strong>{row.spec.title}</Text> },
          { title: '类型', render: (_, row) => taskType(row) },
          { title: '优先级', render: (_, row) => <Tag>{row.spec.priority}</Tag> },
          { title: '状态', render: (_, row) => <Tag>{stateLabel(row.status.state)}</Tag> },
          { title: '节点', render: (_, row) => row.status.leased_by_node_id || routeNode(row) || '-' },
          {
            title: '操作',
            render: (_, row) => (
              <Space wrap>
                <Button size="small" onClick={() => onOpenTask(row.metadata.id)}>详情</Button>
                {['assigned', 'todo'].includes(row.status.state) && (
                  <Button size="small" danger onClick={() => control(row, 'cancel', { reason: '总控台取消排队任务' })}>取消</Button>
                )}
                {row.status.state === 'in_progress' && (
                  <Button size="small" danger onClick={() => control(row, 'stop', { reason: '总控台请求停止任务' })}>停止</Button>
                )}
                <Button size="small" onClick={() => setEditing(row)}>调整</Button>
              </Space>
            ),
          },
        ]}
      />
      <TaskRoutingModal task={editing} nodes={nodes} onClose={() => setEditing(null)} onDone={() => { setEditing(null); onDone(); }} />
    </>
  );
}

function TaskRoutingModal({ task, nodes, onClose, onDone }) {
  const [form] = Form.useForm();
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    if (!task) return;
    form.setFieldsValue({
      priority: task.spec.priority || 'normal',
      node_id: routeNode(task),
      os: routeOs(task),
    });
  }, [form, task]);
  const submit = async (values) => {
    setSaving(true);
    try {
      await fetchJson(`/tasks/${task.metadata.id}/control`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ action: 'update_priority', actor: 'architect-agent', priority: values.priority }),
      });
      await fetchJson(`/tasks/${task.metadata.id}/control`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ action: 'update_routing', actor: 'architect-agent', node_id: values.node_id || undefined, os: values.os || undefined }),
      });
      message.success('队列调度参数已调整');
      onDone();
    } catch (error) {
      message.error(`调整失败：${error.message}`);
    } finally {
      setSaving(false);
    }
  };
  return (
    <Modal title="调整任务调度" open={Boolean(task)} onCancel={onClose} onOk={() => form.submit()} confirmLoading={saving}>
      <Form form={form} layout="vertical" onFinish={submit}>
        <Form.Item name="priority" label="优先级">
          <Select options={[
            { value: 'p0', label: 'P0 紧急' },
            { value: 'high', label: '高' },
            { value: 'normal', label: '普通' },
            { value: 'low', label: '低' },
          ]} />
        </Form.Item>
        <Form.Item name="node_id" label="指定节点">
          <Select allowClear options={nodes.map((node) => ({ value: node.metadata.id, label: `${node.metadata.name} / ${node.spec.os}` }))} />
        </Form.Item>
        <Form.Item name="os" label="指定系统">
          <Select allowClear options={[
            { value: 'linux', label: 'Linux' },
            { value: 'mac', label: 'macOS' },
            { value: 'windows', label: 'Windows' },
          ]} />
        </Form.Item>
      </Form>
    </Modal>
  );
}

function SchedulerConfig({ config, nodes, onDone }) {
  const [form] = Form.useForm();
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    form.setFieldsValue({
      high_load_score_limit: config.high_load_score_limit || 82,
      default_seconds: config.lease?.default_seconds || 120,
      max_seconds: config.lease?.max_seconds || 600,
      priority_order: (config.priority_order || []).join(', '),
    });
  }, [config, form]);

  const submit = async (values) => {
    setSaving(true);
    try {
      await fetchJson('/scheduler-config', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          high_load_score_limit: Number(values.high_load_score_limit || 82),
          priority_order: splitList(values.priority_order),
          lease: {
            default_seconds: Number(values.default_seconds || 120),
            max_seconds: Number(values.max_seconds || 600),
            recover_expired_leases: true,
          },
        }),
      });
      message.success('调度策略已保存');
      onDone();
    } catch (error) {
      message.error(`保存失败：${error.message}`);
    } finally {
      setSaving(false);
    }
  };

  return (
    <Space direction="vertical" size={16} className="full">
      <Row gutter={16}>
        <Col span={8}><Metric title="高负载阈值" value={config.high_load_score_limit || 82} /></Col>
        <Col span={8}><Metric title="默认租约秒数" value={config.lease?.default_seconds || 120} /></Col>
        <Col span={8}><Metric title="在线节点" value={nodes.filter((node) => node.status.state === 'online').length} /></Col>
      </Row>
      <ProCard title="调度策略配置" bordered>
        <Form form={form} layout="vertical" onFinish={submit}>
          <Row gutter={16}>
            <Col span={8}><Form.Item name="high_load_score_limit" label="高负载跳过阈值"><InputNumber min={1} max={100} className="full" /></Form.Item></Col>
            <Col span={8}><Form.Item name="default_seconds" label="默认租约秒数"><InputNumber min={10} max={600} className="full" /></Form.Item></Col>
            <Col span={8}><Form.Item name="max_seconds" label="最大租约秒数"><InputNumber min={10} max={1200} className="full" /></Form.Item></Col>
          </Row>
          <Form.Item name="priority_order" label="优先级顺序">
            <Input placeholder="p0, urgent, high, p1, normal, p2, low" />
          </Form.Item>
          <Button type="primary" htmlType="submit" loading={saving}>保存调度策略</Button>
        </Form>
      </ProCard>
      <ProCard title="节点调度评分参考" bordered>
        <Table
          rowKey={(row) => row.metadata.id}
          dataSource={nodes}
          pagination={false}
          columns={[
            { title: '节点', render: (_, row) => row.metadata.name },
            { title: '状态', render: (_, row) => <Tag>{stateLabel(row.status.state)}</Tag> },
            { title: '权重', render: (_, row) => row.spec.weight },
            { title: '槽位', render: (_, row) => `${row.status.running_jobs || 0}/${row.spec.max_concurrent_jobs || 1}` },
            { title: 'CPU', render: (_, row) => `${round(row.spec.cpu_usage_percent)}%` },
            { title: '内存', render: (_, row) => `${percent(row.spec.memory_used_mb, row.spec.memory_mb)}%` },
            { title: '能力', render: (_, row) => (row.spec.capabilities || []).map((item) => <Tag key={item}>{item}</Tag>) },
          ]}
        />
      </ProCard>
    </Space>
  );
}

function SystemSettings({ settings, auth, onDone }) {
  const [form] = Form.useForm();
  const [passwordForm] = Form.useForm();
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    form.setFieldsValue({
      hub_public_url: settings.hub_public_url || 'http://127.0.0.1:20181',
      registration_enabled: settings.registration_enabled !== false,
      smtp_host: settings.smtp?.host || 'smtp.example.com',
      smtp_port: settings.smtp?.port || 465,
      smtp_username: settings.smtp?.username || 'agentgrid@example.com',
      smtp_password: '',
      smtp_from: settings.smtp?.from || 'agentgrid@example.com',
    });
  }, [settings, form]);
  const submit = async (values) => {
    setSaving(true);
    try {
      await fetchJson('/settings', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          hub_public_url: values.hub_public_url,
          registration_enabled: values.registration_enabled,
          smtp: {
            host: values.smtp_host,
            port: Number(values.smtp_port || 465),
            username: values.smtp_username,
            password: values.smtp_password || '',
            from: values.smtp_from,
            enabled: true,
          },
        }),
      });
      message.success('系统设置已保存');
      form.setFieldValue('smtp_password', '');
      onDone();
    } catch (error) {
      message.error(`保存失败：${error.message}`);
    } finally {
      setSaving(false);
    }
  };
  const changePassword = async (values) => {
    setSaving(true);
    try {
      await fetchJson('/auth/change-password', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(values),
      });
      message.success('密码已修改');
      passwordForm.resetFields();
    } catch (error) {
      message.error(`修改失败：${error.message}`);
    } finally {
      setSaving(false);
    }
  };
  return (
    <Space direction="vertical" size={16} className="full">
      <ProCard title="Hub 系统设置" bordered>
        <Form form={form} layout="vertical" onFinish={submit}>
          <Form.Item name="hub_public_url" label="Hub 访问地址"><Input /></Form.Item>
          <Form.Item name="registration_enabled" label="允许邮箱注册">
            <Select options={[{ value: true, label: '允许' }, { value: false, label: '关闭' }]} />
          </Form.Item>
          <Row gutter={16}>
            <Col span={8}><Form.Item name="smtp_host" label="SMTP 服务器"><Input /></Form.Item></Col>
            <Col span={4}><Form.Item name="smtp_port" label="端口"><InputNumber className="full" /></Form.Item></Col>
            <Col span={12}><Form.Item name="smtp_username" label="邮箱账号"><Input /></Form.Item></Col>
          </Row>
          <Row gutter={16}>
            <Col span={12}><Form.Item name="smtp_password" label="SMTP 授权码"><Input.Password placeholder={settings.smtp?.password_hint || '保持原授权码'} /></Form.Item></Col>
            <Col span={12}><Form.Item name="smtp_from" label="发件人"><Input /></Form.Item></Col>
          </Row>
          <Button type="primary" htmlType="submit" loading={saving}>保存系统设置</Button>
        </Form>
      </ProCard>
      <ProCard title="修改管理员密码" bordered>
        <Form form={passwordForm} layout="vertical" onFinish={changePassword}>
          <Row gutter={16}>
            <Col span={8}><Form.Item name="email" label="邮箱" initialValue={auth?.user?.spec?.email} rules={[{ required: true }]}><Input /></Form.Item></Col>
            <Col span={8}><Form.Item name="old_password" label="旧密码" rules={[{ required: true }]}><Input.Password /></Form.Item></Col>
            <Col span={8}><Form.Item name="new_password" label="新密码" rules={[{ required: true, min: 8 }]}><Input.Password /></Form.Item></Col>
          </Row>
          <Button htmlType="submit" loading={saving}>修改密码</Button>
        </Form>
      </ProCard>
    </Space>
  );
}

function Users({ users, organization, onDone }) {
  const [editing, setEditing] = useState(null);
  const [form] = Form.useForm();
  const [saving, setSaving] = useState(false);
  const superAdmins = users.filter((user) => user.spec?.role === 'super_admin');

  useEffect(() => {
    if (!editing) return;
    form.setFieldsValue({
      name: editing.spec?.name,
      role: editing.spec?.role,
      status: editing.status?.state,
    });
  }, [editing, form]);

  const submit = async (values) => {
    setSaving(true);
    try {
      await fetchJson(`/users/${editing.metadata.id}`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(values),
      });
      message.success('用户档案已保存');
      setEditing(null);
      onDone();
    } catch (error) {
      message.error(`保存失败：${error.message}`);
    } finally {
      setSaving(false);
    }
  };

  return (
    <Space direction="vertical" size={16} className="full">
      <Row gutter={[16, 16]}>
        <Col xs={24} md={12} xl={6}><Metric title="组织" value={organization?.name || '默认组织'} prefix={<TeamOutlined />} /></Col>
        <Col xs={24} md={12} xl={6}><Metric title="用户数" value={users.length} /></Col>
        <Col xs={24} md={12} xl={6}><Metric title="超级管理员" value={superAdmins.length} /></Col>
        <Col xs={24} md={12} xl={6}><Metric title="启用用户" value={users.filter((user) => user.status?.state === 'active').length} /></Col>
      </Row>

      <Alert
        showIcon
        type="info"
        message="节点不登录后台"
        description="用户管理的是人和 AI 员工账号；节点加入集群走“纳管授权”：先生成入网凭证，Worker 上报机器码，管理员在节点管理里确认并授权。"
      />

      <ProCard title="Hub 用户管理" bordered>
        <Table
          rowKey={(row) => row.metadata.id}
          dataSource={users}
          tableLayout="fixed"
          scroll={{ x: 1040 }}
          columns={[
            {
              title: '用户',
              width: 260,
              render: (_, row) => (
                <Space direction="vertical" size={1}>
                  <Text strong>{row.spec?.name || row.spec?.email}</Text>
                  <Text copyable type="secondary">{row.spec?.email}</Text>
                </Space>
              ),
            },
            { title: '角色', width: 130, render: (_, row) => <Tag color={roleColor(row.spec?.role)}>{roleLabel(row.spec?.role)}</Tag> },
            { title: '状态', width: 110, render: (_, row) => <Tag color={row.status?.state === 'active' ? 'green' : 'default'}>{userStatusLabel(row.status?.state)}</Tag> },
            { title: '组织', width: 180, render: (_, row) => row.metadata?.organization_id || organization?.id || '-' },
            { title: '创建时间', width: 180, render: (_, row) => formatTime(row.metadata?.created_at) },
            { title: '更新时间', width: 180, render: (_, row) => formatTime(row.metadata?.updated_at) },
            {
              title: '操作',
              width: 110,
              fixed: 'right',
              render: (_, row) => <Button size="small" onClick={() => setEditing(row)}>编辑</Button>,
            },
          ]}
        />
      </ProCard>

      <Modal
        title={editing ? `编辑用户：${editing.spec?.email}` : '编辑用户'}
        open={Boolean(editing)}
        onCancel={() => setEditing(null)}
        onOk={() => form.submit()}
        confirmLoading={saving}
        width={620}
      >
        <Form form={form} layout="vertical" onFinish={submit}>
          <Form.Item name="name" label="姓名" rules={[{ required: true }]}><Input /></Form.Item>
          <Form.Item name="role" label="角色" rules={[{ required: true }]}>
            <Select
              options={[
                { value: 'super_admin', label: '超级管理员' },
                { value: 'admin', label: '管理员' },
                { value: 'member', label: '成员' },
              ]}
            />
          </Form.Item>
          <Form.Item name="status" label="状态" rules={[{ required: true }]}>
            <Select
              options={[
                { value: 'active', label: '启用' },
                { value: 'disabled', label: '停用' },
              ]}
            />
          </Form.Item>
          <Text type="secondary">规则：Hub 只能有一个超级管理员，且不能停用唯一超级管理员。</Text>
        </Form>
      </Modal>
    </Space>
  );
}

function NodeProvisioning({ plans, settings, onDone }) {
  const [form] = Form.useForm();
  const [creating, setCreating] = useState(false);
  const [selected, setSelected] = useState(null);

  const submit = async (values) => {
    setCreating(true);
    try {
      await fetchJson('/node-provisioning/plans', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          ...values,
          created_by: 'architect-agent',
          hub_url: values.hub_url || settings?.hub_public_url || `${window.location.origin}/agentgrid`,
        }),
      });
      message.success('节点纳管计划已生成');
      form.resetFields();
      onDone();
    } catch (error) {
      message.error(`生成失败：${error.message}`);
    } finally {
      setCreating(false);
    }
  };

  return (
    <Space direction="vertical" size={16} className="full">
      <Alert
        showIcon
        type="info"
        message="节点纳管授权流程"
        description="节点不是后台用户。管理员先在这里生成入网凭证，目标机器运行安装命令后会上报机器码，最后由管理员在节点管理里点击授权并绑定。"
      />
      <ProCard title="新增节点纳管计划" bordered>
        <Form form={form} layout="vertical" onFinish={submit} initialValues={{ ssh_user: 'root', os: 'linux', arch: 'x86_64' }}>
          <Row gutter={16}>
            <Col span={6}><Form.Item name="node_id" label="节点 ID" rules={[{ required: true }]}><Input placeholder="linux-worker-02" /></Form.Item></Col>
            <Col span={6}><Form.Item name="node_name" label="节点名称"><Input placeholder="华瑞子节点" /></Form.Item></Col>
            <Col span={6}><Form.Item name="ssh_host" label="SSH 主机" rules={[{ required: true }]}><Input placeholder="host.example.com" /></Form.Item></Col>
            <Col span={6}><Form.Item name="ssh_user" label="SSH 用户"><Input /></Form.Item></Col>
          </Row>
          <Row gutter={16}>
            <Col span={6}><Form.Item name="os" label="操作系统"><Input /></Form.Item></Col>
            <Col span={6}><Form.Item name="arch" label="CPU 架构"><Input /></Form.Item></Col>
            <Col span={12}><Form.Item name="hub_url" label="Hub 访问地址"><Input placeholder="默认使用当前入口 /agentgrid" /></Form.Item></Col>
          </Row>
          <Form.Item name="notes" label="备注"><Input /></Form.Item>
          <Button type="primary" htmlType="submit" loading={creating} icon={<DeploymentUnitOutlined />}>生成纳管计划</Button>
        </Form>
      </ProCard>

      <ProCard title="纳管计划" bordered>
        <Table
          rowKey={(row) => row.metadata.id}
          dataSource={plans}
          columns={[
            { title: '节点', render: (_, row) => <Text strong>{row.spec.node_name || row.spec.node_id}</Text> },
            { title: 'SSH', render: (_, row) => `${row.spec.ssh_user}@${row.spec.ssh_host}` },
            { title: '系统', render: (_, row) => `${row.spec.os} / ${row.spec.arch}` },
            { title: '状态', render: (_, row) => <Tag>{stateLabel(row.status.state)}</Tag> },
            { title: '授权', render: (_, row) => row.spec.join_token_hint ? <Tag color="blue">{row.spec.join_token_hint}</Tag> : '-' },
            { title: '创建时间', render: (_, row) => formatTime(row.metadata.created_at) },
            { title: '操作', render: (_, row) => <Button size="small" onClick={() => setSelected(row)}>查看步骤</Button> },
          ]}
        />
      </ProCard>
      <Modal title="节点纳管步骤" open={Boolean(selected)} onCancel={() => setSelected(null)} footer={null} width={960}>
        {selected ? (
          <Space direction="vertical" className="full" size={14}>
            <Text>目标：{selected.spec.ssh_user}@{selected.spec.ssh_host}</Text>
            <Steps
              direction="vertical"
              items={(selected.spec.steps || []).map((step) => ({
                title: step.name,
                description: (
                  <Space direction="vertical" className="full">
                    <Text type="secondary">{step.description}</Text>
                    {step.command && <pre className="result-json">{step.command}</pre>}
                    {step.content && <pre className="result-json">{step.content}</pre>}
                  </Space>
                ),
              }))}
            />
          </Space>
        ) : null}
      </Modal>
    </Space>
  );
}

function WorkflowTemplates({ templates, onDone }) {
  const [selected, setSelected] = useState(null);
  const [paramsText, setParamsText] = useState('{}');
  const [starting, setStarting] = useState(false);

  const openTemplate = (template) => {
    const defaults = Object.fromEntries((template.spec.parameters || []).map((item) => [item.name, item.default || '']));
    setSelected(template);
    setParamsText(JSON.stringify(defaults, null, 2));
  };

  const startTemplate = async () => {
    setStarting(true);
    try {
      await fetchJson(`/workflow-templates/${selected.metadata.id}/start`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ actor: 'architect-agent', parameters: JSON.parse(paramsText || '{}') }),
      });
      message.success('模板工作流已启动');
      setSelected(null);
      onDone();
    } catch (error) {
      message.error(`启动失败：${error.message}`);
    } finally {
      setStarting(false);
    }
  };

  return (
    <Space direction="vertical" size={16} className="full">
      <Row gutter={16}>
        <Col span={8}><Metric title="模板数量" value={templates.length} /></Col>
        <Col span={8}><Metric title="内置模板" value={templates.filter((item) => ['node-healthcheck', 'http-probe', 'repo-ci-skeleton'].includes(item.metadata.id)).length} /></Col>
        <Col span={8}><Metric title="可参数化" value={templates.filter((item) => item.spec.parameters?.length).length} /></Col>
      </Row>
      <ProCard title="工作流模板库" bordered>
        <Table
          rowKey={(row) => row.metadata.id}
          dataSource={templates}
          columns={[
            { title: '模板', render: (_, row) => <Space direction="vertical" size={1}><Text strong>{row.spec.name}</Text><Text type="secondary">{row.spec.summary}</Text></Space> },
            { title: '参数', render: (_, row) => (row.spec.parameters || []).map((item) => <Tag key={item.name}>{item.label || item.name}</Tag>) },
            { title: '节点数', render: (_, row) => row.spec.nodes?.length || 0 },
            { title: '更新时间', render: (_, row) => formatTime(row.metadata.updated_at) },
            { title: '操作', render: (_, row) => <Button type="primary" size="small" onClick={() => openTemplate(row)}>填写参数并启动</Button> },
          ]}
          expandable={{
            expandedRowRender: (row) => <pre className="result-json">{JSON.stringify(row.spec.nodes || [], null, 2)}</pre>,
          }}
        />
      </ProCard>
      <Modal
        title={selected ? `启动模板：${selected.spec.name}` : '启动模板'}
        open={Boolean(selected)}
        onCancel={() => setSelected(null)}
        onOk={startTemplate}
        confirmLoading={starting}
        width={760}
      >
        <Space direction="vertical" className="full">
          <Text type="secondary">参数 JSON 会替换模板里的 ${'{参数名}'} 占位符。</Text>
          <Input.TextArea rows={12} className="json-editor" value={paramsText} onChange={(event) => setParamsText(event.target.value)} />
        </Space>
      </Modal>
    </Space>
  );
}

function EventBus({ initialEvents }) {
  const [events, setEvents] = useState(initialEvents || []);
  const [connected, setConnected] = useState(false);

  useEffect(() => {
    setEvents(initialEvents || []);
  }, [initialEvents]);

  useEffect(() => {
    const source = new EventSource(`${apiBase}/events/stream?limit=200`);
    source.addEventListener('events.snapshot', (event) => {
      const data = JSON.parse(event.data);
      setEvents(data.items || []);
      setConnected(true);
    });
    source.onerror = () => setConnected(false);
    return () => source.close();
  }, []);

  return (
    <Space direction="vertical" size={16} className="full">
      <Row gutter={16}>
        <Col span={8}><Metric title="事件数量" value={events.length} /></Col>
        <Col span={8}><Metric title="连接状态" value={connected ? '已连接' : '重连中'} /></Col>
        <Col span={8}><Metric title="事件类型" value={new Set(events.map((item) => item.spec.type)).size} /></Col>
      </Row>
      <ProCard title="统一事件总线" bordered extra={<Badge status={connected ? 'processing' : 'default'} text={connected ? 'SSE 实时同步' : '等待连接'} />}>
        <Table
          rowKey={(row) => row.metadata.id}
          dataSource={events}
          pagination={{ pageSize: 14 }}
          columns={[
            { title: '时间', width: 180, render: (_, row) => formatTime(row.metadata.created_at) },
            { title: '事件类型', width: 220, render: (_, row) => <Tag>{row.spec.type}</Tag> },
            { title: '操作者', width: 150, dataIndex: ['spec', 'actor'] },
            { title: '对象', width: 220, render: (_, row) => row.spec.subject_id || '-' },
            { title: '摘要', dataIndex: ['spec', 'summary'] },
          ]}
          expandable={{
            expandedRowRender: (row) => <pre className="result-json">{JSON.stringify(row.spec.payload, null, 2)}</pre>,
          }}
        />
      </ProCard>
    </Space>
  );
}

function TaskTemplates({ templates, nodes, onDone }) {
  const [selected, setSelected] = useState(null);
  const [paramsText, setParamsText] = useState('{}');
  const [starting, setStarting] = useState(false);
  const activeTemplate = selected || templates[0];

  useEffect(() => {
    if (!selected && templates.length) setSelected(templates[0]);
  }, [templates, selected]);

  useEffect(() => {
    if (!activeTemplate) return;
    const defaults = Object.fromEntries((activeTemplate.spec.parameters || []).map((item) => [item.name, item.default || '']));
    setParamsText(JSON.stringify(defaults, null, 2));
  }, [activeTemplate?.metadata?.id]);

  const startTemplate = async (values) => {
    if (!activeTemplate) return;
    setStarting(true);
    try {
      await fetchJson(`/task-templates/${activeTemplate.metadata.id}/start`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          title: values.title || activeTemplate.spec.name,
          node_id: values.node_id || undefined,
          os: values.os || undefined,
          created_by: 'template-store-web',
          parameters: JSON.parse(paramsText || '{}'),
        }),
      });
      message.success('模板任务已启动');
      onDone();
    } catch (error) {
      message.error(`启动失败：${error.message}`);
    } finally {
      setStarting(false);
    }
  };

  return (
    <Row gutter={[16, 16]}>
      <Col xs={24} xl={7}>
        <ProCard title="任务模板" bordered>
          <Space direction="vertical" className="full">
            {templates.map((template) => (
              <Card
                key={template.metadata.id}
                size="small"
                className={activeTemplate?.metadata.id === template.metadata.id ? 'template-card active' : 'template-card'}
                onClick={() => setSelected(template)}
              >
                <Text strong>{template.spec.name}</Text>
                <br />
                <Text type="secondary">{template.spec.summary}</Text>
                <div className="template-meta">
                  <Tag>{template.spec.category}</Tag>
                  <Tag color="blue">{template.spec.tool_id}</Tag>
                </div>
              </Card>
            ))}
          </Space>
        </ProCard>
      </Col>
      <Col xs={24} xl={17}>
        <ProCard title={activeTemplate ? `启动模板：${activeTemplate.spec.name}` : '启动模板'} bordered>
          {activeTemplate ? (
            <Form layout="vertical" onFinish={startTemplate} initialValues={{ title: activeTemplate.spec.name }}>
              <Row gutter={16}>
                <Col span={10}><Form.Item name="title" label="任务标题"><Input /></Form.Item></Col>
                <Col span={7}>
                  <Form.Item name="node_id" label="指定节点">
                    <Select allowClear options={nodes.map((node) => ({ value: node.metadata.id, label: `${node.metadata.name} / ${node.spec.os}` }))} />
                  </Form.Item>
                </Col>
                <Col span={7}>
                  <Form.Item name="os" label="指定系统">
                    <Select allowClear options={[
                      { value: 'linux', label: 'Linux' },
                      { value: 'mac', label: 'macOS' },
                      { value: 'windows', label: 'Windows' },
                    ]} />
                  </Form.Item>
                </Col>
              </Row>
              <Row gutter={16}>
                <Col span={10}>
                  <ProCard title="参数" size="small" bordered>
                    {(activeTemplate.spec.parameters || []).map((item) => (
                      <div key={item.name} className="param-row">
                        <Text strong>{item.label || item.name}</Text>
                        <Text type="secondary">{item.name}</Text>
                        {item.required ? <Tag color="red">必填</Tag> : <Tag>可选</Tag>}
                      </div>
                    ))}
                    {!activeTemplate.spec.parameters?.length && <Text type="secondary">这个模板不需要参数。</Text>}
                  </ProCard>
                </Col>
                <Col span={14}>
                  <Form.Item label="参数 JSON">
                    <Input.TextArea rows={12} className="json-editor" value={paramsText} onChange={(event) => setParamsText(event.target.value)} />
                  </Form.Item>
                </Col>
              </Row>
              <ProCard title="标准 Payload" size="small" bordered className="section-card">
                <pre className="result-json">{JSON.stringify(activeTemplate.spec.payload, null, 2)}</pre>
              </ProCard>
              <Button type="primary" htmlType="submit" loading={starting}>启动模板任务</Button>
            </Form>
          ) : <Text type="secondary">暂无模板。</Text>}
        </ProCard>
      </Col>
    </Row>
  );
}

function Webhooks({ webhooks, deliveries, onDone }) {
  const [form] = Form.useForm();
  const [creating, setCreating] = useState(false);
  const createWebhook = async (values) => {
    setCreating(true);
    try {
      await fetchJson('/webhooks', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          name: values.name,
          url: values.url,
          events: values.events || ['task.completed', 'task.failed'],
          secret: values.secret || null,
          enabled: true,
          created_by: 'architect-agent',
        }),
      });
      message.success('Webhook 已创建');
      form.resetFields();
      onDone();
    } catch (error) {
      message.error(`创建失败：${error.message}`);
    } finally {
      setCreating(false);
    }
  };

  const deleteWebhook = async (id) => {
    await fetchJson(`/webhooks/${id}`, { method: 'DELETE' });
    message.success('Webhook 已停用');
    onDone();
  };

  return (
    <Space direction="vertical" size={16} className="full">
      <Row gutter={16}>
        <Col span={8}><Metric title="订阅数量" value={webhooks.length} /></Col>
        <Col span={8}><Metric title="启用订阅" value={webhooks.filter((item) => item.spec.enabled).length} /></Col>
        <Col span={8}><Metric title="投递记录" value={deliveries.length} /></Col>
      </Row>
      <ProCard title="新增任务回调" bordered>
        <Form form={form} layout="vertical" onFinish={createWebhook} initialValues={{ events: ['task.completed', 'task.failed'] }}>
          <Row gutter={16}>
            <Col span={6}><Form.Item name="name" label="名称" rules={[{ required: true }]}><Input placeholder="CI 回调" /></Form.Item></Col>
            <Col span={10}><Form.Item name="url" label="回调地址" rules={[{ required: true }]}><Input placeholder="https://example.com/webhook" /></Form.Item></Col>
            <Col span={8}>
              <Form.Item name="events" label="事件">
                <Select mode="tags" options={[
                  { value: 'task.completed', label: '任务完成' },
                  { value: 'task.failed', label: '任务失败' },
                  { value: '*', label: '全部事件' },
                ]} />
              </Form.Item>
            </Col>
          </Row>
          <Form.Item name="secret" label="签名密钥"><Input.Password placeholder="可选，后续用于签名校验" /></Form.Item>
          <Button type="primary" htmlType="submit" loading={creating}>创建回调</Button>
        </Form>
      </ProCard>
      <ProCard title="Webhook 订阅" bordered>
        <Table
          rowKey={(row) => row.metadata.id}
          dataSource={webhooks}
          columns={[
            { title: '名称', render: (_, row) => <Text strong>{row.spec.name}</Text> },
            { title: '地址', render: (_, row) => <Text copyable>{row.spec.url}</Text> },
            { title: '事件', render: (_, row) => (row.spec.events || []).map((item) => <Tag key={item}>{item}</Tag>) },
            { title: '状态', render: (_, row) => row.spec.enabled ? <Badge status="success" text="启用" /> : <Badge status="default" text="停用" /> },
            { title: '操作', render: (_, row) => <Button danger size="small" onClick={() => deleteWebhook(row.metadata.id)}>停用</Button> },
          ]}
        />
      </ProCard>
      <ProCard title="最近投递" bordered>
        <Table
          rowKey={(row) => row.metadata.id}
          dataSource={deliveries}
          columns={[
            { title: '时间', render: (_, row) => formatTime(row.metadata.created_at) },
            { title: '事件', render: (_, row) => <Tag>{row.spec.event_type}</Tag> },
            { title: '任务', render: (_, row) => row.spec.subject_id || '-' },
            { title: '状态', render: (_, row) => row.spec.status === 'delivered' ? <Tag color="green">成功</Tag> : <Tag color="red">失败</Tag> },
            { title: 'HTTP', render: (_, row) => row.spec.status_code || '-' },
            { title: '错误', render: (_, row) => row.spec.error || '-' },
          ]}
          expandable={{
            expandedRowRender: (row) => <pre className="result-json">{JSON.stringify(row.spec.payload, null, 2)}</pre>,
          }}
        />
      </ProCard>
    </Space>
  );
}

function Diagnostics({ diagnostics }) {
  const taskInfo = diagnostics.tasks || {};
  const nodeInfo = diagnostics.nodes || {};
  const logs = diagnostics.logs?.recent_audit || [];
  return (
    <Space direction="vertical" size={16} className="full">
      <Row gutter={16}>
        <Col span={6}><Metric title="在线节点" value={nodeInfo.online || 0} /></Col>
        <Col span={6}><Metric title="未知节点" value={nodeInfo.unknown || 0} /></Col>
        <Col span={6}><Metric title="过期租约" value={taskInfo.expired_leases || 0} /></Col>
        <Col span={6}><Metric title="失败任务" value={taskInfo.failed || 0} /></Col>
      </Row>
      <Row gutter={16}>
        <Col span={12}>
          <ProCard title="任务运行诊断" bordered>
            <Table
              pagination={false}
              dataSource={[
                { key: 'assigned', name: '等待调度', value: taskInfo.assigned || 0 },
                { key: 'in_progress', name: '执行中', value: taskInfo.in_progress || 0 },
                { key: 'done', name: '已完成', value: taskInfo.done || 0 },
                { key: 'failed', name: '失败', value: taskInfo.failed || 0 },
              ]}
              columns={[
                { title: '指标', dataIndex: 'name' },
                { title: '数量', dataIndex: 'value' },
              ]}
            />
          </ProCard>
        </Col>
        <Col span={12}>
          <ProCard title="最近失败" bordered>
            <Table
              rowKey={(row) => row.metadata.id}
              dataSource={taskInfo.recent_failures || []}
              pagination={false}
              columns={[
                { title: '任务', render: (_, row) => row.spec.title },
                { title: '节点', render: (_, row) => row.status.leased_by_node_id || '-' },
                { title: '原因', render: (_, row) => row.status.error?.message || row.status.blocked_reason || '-' },
              ]}
            />
          </ProCard>
        </Col>
      </Row>
      <ProCard title="最近运行日志" bordered>
        <Table
          rowKey={(row) => row.metadata.id}
          dataSource={logs}
          columns={[
            { title: '时间', render: (_, row) => formatTime(row.metadata.created_at) },
            { title: '类型', render: (_, row) => <Tag>{row.spec.type}</Tag> },
            { title: '对象', render: (_, row) => row.spec.subject_id || '-' },
            { title: '摘要', dataIndex: ['spec', 'summary'] },
          ]}
        />
      </ProCard>
    </Space>
  );
}

function AuditLog({ events }) {
  return (
    <ProCard title="审计日志" bordered>
      <Table
        rowKey={(row) => row.metadata.id}
        dataSource={events}
        columns={[
          { title: '时间', render: (_, row) => formatTime(row.metadata.created_at) },
          { title: '类型', render: (_, row) => <Tag>{row.spec.type}</Tag> },
          { title: '操作者', dataIndex: ['spec', 'actor'] },
          { title: '对象', render: (_, row) => row.spec.subject_id || '-' },
          { title: '摘要', dataIndex: ['spec', 'summary'] },
        ]}
        expandable={{
          expandedRowRender: (row) => (
            <pre className="result-json">{JSON.stringify(row.spec.payload, null, 2)}</pre>
          ),
        }}
      />
    </ProCard>
  );
}

function TaskDetailModal({ taskId, tasks, artifacts, auditEvents, onClose }) {
  const task = tasks.find((item) => item.metadata.id === taskId);
  const [live, setLive] = useState(null);
  const [preview, setPreview] = useState(null);
  const [schedulePreview, setSchedulePreview] = useState(null);
  const baseTimeline = auditEvents.filter((event) => event.spec.subject_id === taskId);
  const timeline = live?.events || baseTimeline;
  const leaseEvent = timeline.find((event) => event.spec.type === 'task.leased');
  const scheduler = leaseEvent?.spec.payload?.scheduler;
  const result = live?.result && live.result !== null ? live.result : task?.status.result;
  const error = live?.error && live.error !== null ? live.error : task?.status.error;
  const inputText = task?.spec.inputs?.join('\n\n') || '';
  const liveStdout = (live?.logs || []).filter((item) => item.spec.stream === 'stdout').map((item) => item.spec.line).join('');
  const liveStderr = (live?.logs || []).filter((item) => item.spec.stream === 'stderr').map((item) => item.spec.line).join('');
  const stdout = liveStdout || live?.stdout || result?.stdout || '';
  const stderr = liveStderr || live?.stderr || result?.stderr || error?.message || '';
  const verification = result?.verification;
  const state = live?.state || task?.status.state;
  const progress = live?.progress ?? task?.status.progress ?? 0;
  const leasedNode = live?.leased_by_node_id || task?.status.leased_by_node_id;
  const taskArtifacts = live?.artifacts || artifacts.filter((artifact) => artifact.spec.task_id === taskId);
  const inputPayload = parseTaskInputPayload(task);
  const taskOperation = taskOperationLabel(inputPayload);
  const actionLabel = task?.spec.labels?.find((label) => label.startsWith('action:'))?.replace('action:', '') || taskOperation;
  const workbenchId = task?.spec.labels?.find((label) => label.startsWith('workbench:'))?.replace('workbench:', '');
  const channelRole = schedulePreview?.decision?.required_channel_role
    || schedulePreview?.candidates?.find((item) => item.node_id === leasedNode)?.channel_role
    || (inputPayload?.type === 'desktop' ? 'desktop' : 'worker');
  const durationMs = result?.duration_ms ?? error?.result?.duration_ms;
  const exitCode = result?.exit_code ?? error?.result?.exit_code;
  const desktopTimeline = buildDesktopTimeline({
    task,
    inputPayload,
    result,
    error,
    artifacts: taskArtifacts,
    events: timeline,
  });
  const executionSummary = [
    ['提交人', task?.metadata.created_by || '-'],
    ['负责人', task?.spec.owner || '-'],
    ['执行节点', leasedNode || '-'],
    ['通道类型', channelLabel(channelRole)],
    ['执行动作', taskOperation],
    ['Workbench', workbenchId || '-'],
    ['调度原因', schedulePreview?.decision?.reason || scheduler?.reason || '暂无调度说明'],
    ['产物', taskArtifacts.length ? `${taskArtifacts.length} 个` : '无'],
    ['失败原因', error?.message || task?.status.blocked_reason || '-'],
  ].map(([label, value]) => ({ label, value }));

  useEffect(() => {
    setLive(null);
    setSchedulePreview(null);
    if (!taskId) return undefined;
    fetchJson(`/tasks/${taskId}/schedule-preview`)
      .then((data) => setSchedulePreview(data.item))
      .catch(() => setSchedulePreview(null));
    const source = new EventSource(`${apiBase}/tasks/${taskId}/events`);
    source.addEventListener('task.snapshot', (event) => {
      setLive(JSON.parse(event.data));
    });
    source.addEventListener('task.error', (event) => {
      setLive((current) => ({ ...(current || {}), error: JSON.parse(event.data).error }));
    });
    source.onerror = () => {
      source.close();
    };
    return () => source.close();
  }, [taskId]);

  return (
    <Modal
      title={task ? `任务详情：${task.spec.title}` : '任务详情'}
      open={Boolean(taskId)}
      onCancel={onClose}
      footer={null}
      width={1040}
    >
      {task ? (
        <Space direction="vertical" className="full" size={14}>
          <section className="task-hero">
            <div className="task-hero-main">
              <Space wrap>
                <Tag color={taskStateColor(state)}>{stateLabel(state)}</Tag>
                <Tag color={channelColor(channelRole, { status: { state: leasedNode ? 'online' : 'unknown' } })}>{channelLabel(channelRole)}</Tag>
                <Tag>{actionLabel}</Tag>
              </Space>
              <Title level={4}>{task.spec.title}</Title>
              <Text type="secondary">{task.spec.summary || 'AgentGrid 结构化任务'}</Text>
            </div>
            <div className="task-hero-side">
              <Progress type="circle" percent={taskStateProgress(state) || progress} size={74} />
              <Text type="secondary">进度 {progress}%</Text>
            </div>
          </section>

          <Row gutter={12}>
            <Col span={6}><Card size="small"><Statistic title="执行节点" value={leasedNode || '-'} /></Card></Col>
            <Col span={6}><Card size="small"><Statistic title="耗时" value={durationMs != null ? `${durationMs} ms` : '-'} /></Card></Col>
            <Col span={6}><Card size="small"><Statistic title="退出码" value={exitCode != null ? exitCode : '-'} /></Card></Col>
            <Col span={6}><Card size="small"><Statistic title="证据产物" value={taskArtifacts.length} suffix="个" /></Card></Col>
          </Row>

          <ProCard title="执行记录" bordered>
            <Table
              size="small"
              pagination={false}
              rowKey={(row) => row.label}
              dataSource={executionSummary}
              columns={[
                { title: '项目', width: 120, dataIndex: 'label' },
                { title: '内容', render: (_, row) => <Text>{row.value}</Text> },
              ]}
            />
          </ProCard>

          <ProCard title="实时执行日志" bordered extra={<Badge status={['in_progress', 'stopping'].includes(state) ? 'processing' : 'default'} text={live ? '已连接' : '等待事件'} />}>
            <Row gutter={12}>
              <Col span={12}>
                <Text strong>stdout</Text>
                <pre className="result-json live-log">{stdout || resultText(result)}</pre>
              </Col>
              <Col span={12}>
                <Text strong>stderr / 失败原因</Text>
                <pre className="result-json live-log">{stderr || resultText(error)}</pre>
              </Col>
            </Row>
          </ProCard>

          <ProCard title="调度决策" bordered>
            {schedulePreview ? (
              <SchedulingDecisionPanel preview={schedulePreview} leasedNode={leasedNode} scheduler={scheduler} />
            ) : scheduler ? (
              <Space direction="vertical" className="full">
                <Text>{scheduler.reason}</Text>
                <Text type="secondary">评分：{scheduler.score ?? '-'}</Text>
                <Table
                  size="small"
                  pagination={false}
                  rowKey={(row) => row.node_id}
                  dataSource={scheduler.candidates || []}
                  columns={[
                    { title: '候选节点', dataIndex: 'node_id' },
                    { title: '评分', render: (_, row) => Number(row.score || 0).toFixed(2) },
                    { title: '可用槽位', dataIndex: 'available_slots' },
                  ]}
                />
              </Space>
            ) : (
              <Text type="secondary">暂无调度审计记录</Text>
            )}
          </ProCard>

          <ProCard title="输入参数" bordered>
            <pre className="result-json">{inputText || '-'}</pre>
          </ProCard>

          <ProCard title="结果验收" bordered>
            {verification ? (
              <Space direction="vertical" className="full">
                <Space>
                  <VerificationTag verification={verification} />
                  <Text type="secondary">{verification.summary}</Text>
                  <Text type="secondary">{formatTime(verification.checked_at)}</Text>
                </Space>
                <Table
                  size="small"
                  pagination={false}
                  rowKey={(row, index) => `${row.path}-${row.op}-${index}`}
                  dataSource={verification.rules || []}
                  columns={[
                    { title: '状态', render: (_, row) => <Tag color={row.ok ? 'green' : 'red'}>{row.ok ? '通过' : '失败'}</Tag> },
                    { title: '路径', dataIndex: 'path' },
                    { title: '规则', dataIndex: 'op' },
                    { title: '期望', render: (_, row) => compactJson(row.expected) },
                    { title: '实际', render: (_, row) => compactJson(row.actual) },
                    { title: '说明', render: (_, row) => row.description || row.message || '-' },
                  ]}
                />
              </Space>
            ) : (
              <Text type="secondary">这个任务没有配置结果验收规则</Text>
            )}
          </ProCard>

          <ProCard title="结构化结果" bordered>
            <pre className="result-json">{JSON.stringify(result || error || {}, null, 2)}</pre>
          </ProCard>

          <ProCard title="证据产物" bordered extra={<Tag>{taskArtifacts.length} 个</Tag>}>
            <EvidenceGrid artifacts={taskArtifacts} onPreview={setPreview} />
          </ProCard>

          {desktopTimeline.length > 0 && (
            <ProCard title="桌面操作时间线" bordered>
              <Table
                className="desktop-timeline-table"
                size="small"
                pagination={false}
                rowKey={(row) => row.id}
                dataSource={desktopTimeline}
                columns={[
                  { title: '时间', width: 180, render: (_, row) => formatTime(row.time) },
                  { title: '操作', width: 150, render: (_, row) => <Tag color={row.kind === 'screenshot' ? 'blue' : 'purple'}>{desktopOperationLabel(row.kind)}</Tag> },
                  { title: '节点', width: 190, render: (_, row) => row.node || '-' },
                  { title: '说明', render: (_, row) => row.summary },
                  {
                    title: '产物',
                    width: 120,
                    render: (_, row) => row.artifact ? (
                      <Button size="small" onClick={() => setPreview(row.artifact)}>
                        查看截图
                      </Button>
                    ) : '-',
                  },
                ]}
                expandable={{
                  expandedRowRender: (row) => (
                    <pre className="result-json">{JSON.stringify(row.raw, null, 2)}</pre>
                  ),
                }}
              />
            </ProCard>
          )}

          <ProCard title="审计时间线" bordered>
            <Table
              size="small"
              pagination={false}
              rowKey={(row) => row.metadata.id}
              dataSource={timeline}
              columns={[
                { title: '时间', render: (_, row) => formatTime(row.metadata.created_at) },
                { title: '类型', render: (_, row) => <Tag>{row.spec.type}</Tag> },
                { title: '操作者', dataIndex: ['spec', 'actor'] },
                { title: '摘要', dataIndex: ['spec', 'summary'] },
              ]}
              expandable={{
                expandedRowRender: (row) => (
                  <pre className="result-json">{JSON.stringify(row.spec.payload, null, 2)}</pre>
                ),
              }}
            />
          </ProCard>
          <ArtifactPreview artifact={preview} onClose={() => setPreview(null)} />
        </Space>
      ) : (
        <Text type="secondary">未找到任务</Text>
      )}
    </Modal>
  );
}

function SchedulingDecisionPanel({ preview, leasedNode, scheduler }) {
  const candidates = preview.candidates || [];
  const selectedNodeId = preview.selected_node_id || preview.decision?.node_id || leasedNode;
  const selected = candidates.find((item) => item.node_id === selectedNodeId);
  const eligible = candidates.filter((item) => item.eligible);
  const skipped = candidates.filter((item) => !item.eligible);
  const requirements = preview.requirements || {};
  const required = selected?.task_requires || candidates[0]?.task_requires || {};
  const suggestions = schedulingSuggestions(candidates, preview);
  return (
    <Space direction="vertical" className="full" size={14}>
      <section className="schedule-summary">
        <div>
          <Text className="eyebrow">Placement Decision</Text>
          <Title level={5}>{selectedNodeId ? `选择 ${selectedNodeId}` : '暂无可用节点'}</Title>
          <Text type="secondary">{preview.decision?.reason || scheduler?.reason || '等待调度器写回决策原因'}</Text>
        </div>
        <div className="schedule-score">
          <Statistic title="调度评分" value={Number(preview.decision?.score ?? selected?.score ?? 0).toFixed(2)} />
          <Text type="secondary">分数越低越优</Text>
        </div>
      </section>

      <div className="schedule-requirements">
        <RequirementPill label="任务类型" value={required.task_type || preview.payload_type || '-'} />
        <RequirementPill label="目标通道" value={channelLabel(required.channel_role || selected?.required_channel_role || 'worker')} />
        <RequirementPill label="工具" value={required.tool_id || '-'} />
        <RequirementPill label="电脑" value={requirements.workbench_id || '不限'} />
        <RequirementPill label="指定节点" value={requirements.node_id || '不限'} />
        <RequirementPill label="操作系统" value={(requirements.os || []).join(', ') || '不限'} />
        <RequirementPill label="能力" value={(requirements.capabilities || []).join(', ') || '不限'} />
      </div>

      {selected && (
        <section className="selected-node-card">
          <div>
            <Space wrap>
              <Tag color="green">最终选择</Tag>
              <Tag color={channelColor(selected.channel_role, { status: { state: selected.state } })}>{channelLabel(selected.channel_role)}</Tag>
              <Tag>{selected.os}</Tag>
              <Tag color={probeStateColor(selected.trust?.state)}>{probeStateLabel(selected.trust?.state)}</Tag>
              <Tag color={riskColor(selected.trust?.risk)}>{riskLabel(selected.trust?.risk)}风险</Tag>
            </Space>
            <Title level={5}>{selected.node_name || selected.node_id}</Title>
            <Text type="secondary">{selected.channel_explanation}</Text>
          </div>
          <div className="selected-node-metrics">
            <Statistic title="可用槽位" value={selected.available_slots ?? 0} />
            <Statistic title="资源分" value={Number(selected.base_resource_score ?? selected.score ?? 0).toFixed(2)} />
          </div>
        </section>
      )}

      {!!suggestions.length && (
        <Alert
          type={selectedNodeId ? 'info' : 'warning'}
          showIcon
          message="调度建议"
          description={
            <Space direction="vertical" size={4}>
              {suggestions.map((item) => <Text key={item}>{item}</Text>)}
            </Space>
          }
        />
      )}

      <Row gutter={12}>
        <Col span={12}><Card size="small"><Statistic title="可调度节点" value={eligible.length} suffix="个" /></Card></Col>
        <Col span={12}><Card size="small"><Statistic title="被排除节点" value={skipped.length} suffix="个" /></Card></Col>
      </Row>

      <Table
        className="schedule-candidate-table"
        size="small"
        pagination={{ pageSize: 8 }}
        rowKey={(row) => row.node_id}
        dataSource={candidates}
        columns={[
          {
            title: '节点',
            width: 220,
            render: (_, row) => (
              <Space direction="vertical" size={1}>
                <Text strong>{row.node_name || row.node_id}</Text>
                <Text type="secondary">{row.node_id}</Text>
              </Space>
            ),
          },
          { title: '通道', width: 120, render: (_, row) => <Tag color={channelColor(row.channel_role, { status: { state: row.state } })}>{channelLabel(row.channel_role)}</Tag> },
          { title: '状态', width: 110, render: (_, row) => <Tag color={row.eligible ? 'green' : 'default'}>{row.eligible ? '可调度' : '跳过'}</Tag> },
          { title: '系统', width: 120, render: (_, row) => row.os || '-' },
          { title: '槽位', width: 80, dataIndex: 'available_slots' },
          { title: '评分', width: 95, render: (_, row) => Number(row.score || 0).toFixed(2) },
          { title: '资源分', width: 95, render: (_, row) => Number(row.base_resource_score || row.score || 0).toFixed(2) },
          { title: 'Probe', width: 110, render: (_, row) => <Tag color={probeStateColor(row.trust?.state)}>{probeStateLabel(row.trust?.state)}</Tag> },
          { title: '风险', width: 90, render: (_, row) => <Tag color={riskColor(row.trust?.risk)}>{riskLabel(row.trust?.risk)}</Tag> },
          { title: '原因', render: (_, row) => <CandidateReasons row={row} /> },
        ]}
        expandable={{
          expandedRowRender: (row) => (
            <pre className="result-json">{JSON.stringify({
              task_requires: row.task_requires,
              trust: row.trust,
              worker: row.worker,
              reasons: row.reasons,
            }, null, 2)}</pre>
          ),
        }}
      />
    </Space>
  );
}

function RequirementPill({ label, value }) {
  return (
    <div className="requirement-pill">
      <Text type="secondary">{label}</Text>
      <Text strong>{value}</Text>
    </div>
  );
}

function CandidateReasons({ row }) {
  const reasons = row.reasons || [];
  if (!reasons.length) return <Text type="secondary">暂无说明</Text>;
  const primary = reasons[0];
  return (
    <Space direction="vertical" size={2}>
      <Text>{primary}</Text>
      {reasons.slice(1, 3).map((reason) => <Text key={reason} type="secondary">{reason}</Text>)}
      {reasons.length > 3 && <Text type="secondary">还有 {reasons.length - 3} 条原因，展开查看</Text>}
    </Space>
  );
}

function schedulingSuggestions(candidates, preview) {
  const suggestions = new Set();
  if (!candidates.some((item) => item.eligible)) {
    suggestions.add('没有可调度节点：优先检查节点是否在线、能力是否注册、通道是否匹配。');
  }
  if (candidates.some((item) => item.reasons?.some((reason) => reason.includes('节点状态是 Offline')))) {
    suggestions.add('有节点离线：启动对应 Worker，或等待心跳恢复后再提交任务。');
  }
  if (candidates.some((item) => item.reasons?.some((reason) => reason.includes('缺少执行能力')))) {
    suggestions.add('有节点缺少能力：在 Worker 注册能力或安装对应插件/工具。');
  }
  if (candidates.some((item) => item.reasons?.some((reason) => reason.includes('通道')))) {
    suggestions.add('通道不匹配：后台命令走 Worker，截图/点击/输入走 Desktop Helper。');
  }
  if (candidates.some((item) => item.reasons?.some((reason) => reason.includes('Probe') || reason.includes('未通过运行时验证') || reason.includes('未验证')))) {
    suggestions.add('可信度较低：运行 Tool Probe，把节点工具状态从声明提升为已验证。');
  }
  if (preview?.requirements?.workbench_id) {
    suggestions.add('这个任务锁定了指定电脑/工位，其他电脑即使在线也不会被选中。');
  }
  return Array.from(suggestions).slice(0, 4);
}

function EvidenceGrid({ artifacts, onPreview }) {
  if (!artifacts.length) {
    return <div className="empty-panel">这个任务还没有产物。命令输出、截图、报告和文件会在这里归档。</div>;
  }
  return (
    <div className="evidence-grid">
      {artifacts.map((artifact) => {
        const isImage = isImageArtifact(artifact);
        const text = artifactTextPreview(artifact);
        return (
          <div key={artifact.metadata.id} className="evidence-card">
            <div className="evidence-head">
              <Space wrap>
                <Tag color={isImage ? 'blue' : 'purple'}>{artifactTypeLabel(artifact.spec.type)}</Tag>
                <Tag>{artifact.spec.v2?.preview?.kind || previewKind(artifact)}</Tag>
              </Space>
              <Text type="secondary">{formatBytes(artifact.spec.size_bytes)}</Text>
            </div>
            <Text strong className="evidence-title">{artifact.spec.name}</Text>
            <Text type="secondary" className="evidence-meta">
              {artifact.spec.node_id || '-'} · {formatTime(artifact.metadata.created_at)}
            </Text>
            {isImage ? (
              <button type="button" className="evidence-image" onClick={() => onPreview(artifact)}>
                <img src={artifactDataUrl(artifact)} alt={artifact.spec.name} />
              </button>
            ) : (
              <pre className="evidence-text">{text || '无内联预览'}</pre>
            )}
            <div className="evidence-actions">
              {artifact.spec.v2?.sha256 && <Text code>{shortHash(artifact.spec.v2.sha256)}</Text>}
              <Button size="small" icon={<DownloadOutlined />} disabled={!artifact.spec.content_base64} href={artifactDownloadUrl(artifact)}>
                下载
              </Button>
            </div>
          </div>
        );
      })}
    </div>
  );
}

function artifactTextPreview(artifact) {
  if (!artifact?.spec?.content_base64) return '';
  const contentType = artifact.spec.content_type || '';
  if (!contentType.startsWith('text/') && !contentType.includes('json')) return '';
  try {
    const decoded = decodeURIComponent(
      Array.from(window.atob(artifact.spec.content_base64))
        .map((char) => `%${char.charCodeAt(0).toString(16).padStart(2, '0')}`)
        .join('')
    );
    return decoded.length > 1600 ? `${decoded.slice(0, 1600)}\n...` : decoded;
  } catch {
    try {
      const decoded = window.atob(artifact.spec.content_base64);
      return decoded.length > 1600 ? `${decoded.slice(0, 1600)}\n...` : decoded;
    } catch {
      return '';
    }
  }
}

function CommandDocs({ doc, setDoc }) {
  const docPath = window.location.pathname.startsWith('/agentgrid')
    ? '/agentgrid/docs/agentgrid-command-reference.md'
    : '/docs/agentgrid-command-reference.md';
  const htmlDocPath = window.location.pathname.startsWith('/agentgrid')
    ? '/agentgrid/docs/agentgrid-command-reference.html'
    : '/docs/agentgrid-command-reference.html';

  useEffect(() => {
    if (doc) return;
    fetch(docPath)
      .then((response) => response.text())
      .then(setDoc)
      .catch((error) => setDoc(`文档加载失败：${error.message}`));
  }, [doc, docPath, setDoc]);

  return (
    <ProCard
      title="命令文档"
      bordered
      extra={(
        <Space>
          <Button href={htmlDocPath} target="_blank" icon={<FileTextOutlined />}>打开 HTML 文档</Button>
          <Button href={docPath} target="_blank">Markdown 给 AI</Button>
        </Space>
      )}
    >
      <div className="doc-layout">
        <div className="doc-summary">
          <Title level={4}>AgentGrid 操作手册</Title>
          <Text type="secondary">
            这份文档给人和 AI 共用，覆盖 CLI、REST API、HTTP 任务、命令任务、节点调度、安全策略和结果格式。
          </Text>
          <Space wrap className="doc-tags">
            <Tag color="blue">CLI</Tag>
            <Tag color="green">REST API</Tag>
            <Tag color="purple">Command Task</Tag>
            <Tag color="orange">Scheduler</Tag>
            <Tag>AI Readable</Tag>
          </Space>
        </div>
        <pre className="doc-markdown">{doc || '正在加载文档...'}</pre>
      </div>
    </ProCard>
  );
}

function Agents({ agents }) {
  return (
    <ProCard
      title="AI 员工档案"
      bordered
      extra={<Text type="secondary">身份、职责、凭据、节点范围、工具范围</Text>}
      className="agents-card"
    >
      <Table
        className="agents-table"
        size="middle"
        rowKey={(row) => row.metadata.id}
        dataSource={agents}
        pagination={{ pageSize: 8, showSizeChanger: false }}
        scroll={{ x: 1180 }}
        expandable={{
          expandedRowRender: (row) => (
            <Row gutter={[16, 16]}>
              <Col xs={24} lg={8}>
                <ProCard title="职责档案" size="small" bordered>
                  <DescriptionsList
                    rows={[
                      ['员工 ID', row.metadata.id],
                      ['账号', row.credentials?.account_username || '未设置'],
                      ['角色', row.spec.role],
                      ['责任', row.spec.responsibility],
                    ]}
                  />
                </ProCard>
              </Col>
              <Col xs={24} lg={8}>
                <ProCard title="凭据状态" size="small" bordered>
                  <DescriptionsList
                    rows={[
                      ['认证方式', row.credentials?.auth_type || 'bearer_token'],
                      ['Token', row.credentials?.token_configured ? `已配置 ${row.credentials?.token_hint || ''}` : '未配置'],
                      ['凭据状态', credentialLabel(row.credentials?.credential_status)],
                      ['凭据引用', stringifyShort(row.credentials?.credential_refs || {})],
                    ]}
                  />
                </ProCard>
              </Col>
              <Col xs={24} lg={8}>
                <ProCard title="授权范围" size="small" bordered>
                  <DescriptionsList
                    rows={[
                      ['节点范围', scopeLabel(row.access?.node_scope, 'node')],
                      ['工具范围', scopeLabel(row.access?.tool_scope, 'tool')],
                      ['技能', (row.spec.skills || []).join('、') || '未设置'],
                      ['权限', (row.spec.permissions || []).join('、') || '未设置'],
                    ]}
                  />
                </ProCard>
              </Col>
            </Row>
          ),
        }}
        columns={[
          {
            title: '身份',
            width: 240,
            fixed: 'left',
            render: (_, row) => (
              <Space direction="vertical" size={2} className="agent-identity">
                <Text strong>{row.metadata.name}</Text>
                <Text type="secondary">{row.metadata.id}</Text>
              </Space>
            ),
          },
          {
            title: '角色',
            dataIndex: ['spec', 'role'],
            width: 190,
            render: (value) => <Text>{value || '未设置'}</Text>,
          },
          {
            title: '凭据',
            width: 130,
            render: (_, row) => row.credentials?.token_configured
              ? <Tag color="green">Token 已配置</Tag>
              : <Tag>未配置 Token</Tag>,
          },
          {
            title: '节点范围',
            width: 170,
            render: (_, row) => <ScopeTag scope={row.access?.node_scope} type="node" />,
          },
          {
            title: '工具范围',
            width: 170,
            render: (_, row) => <ScopeTag scope={row.access?.tool_scope} type="tool" />,
          },
          {
            title: '责任',
            dataIndex: ['spec', 'responsibility'],
            width: 360,
            render: (value) => <Text className="agent-responsibility">{value || '未设置'}</Text>,
          },
          {
            title: '状态',
            width: 92,
            fixed: 'right',
            align: 'center',
            render: (_, row) => (
              <span className="agent-status">
                <Badge status={row.status.state === 'online' ? 'success' : 'default'} />
                <Text>{stateLabel(row.status.state)}</Text>
              </span>
            ),
          },
        ]}
      />
    </ProCard>
  );
}

function ScopeTag({ scope, type }) {
  const mode = scope?.mode || (type === 'node' ? 'none' : 'declared');
  const labels = {
    all: type === 'node' ? '全部节点' : '全部工具',
    none: type === 'node' ? '不挂节点' : '无工具',
    nodes: `指定节点 ${(scope?.nodes || []).length || ''}`.trim(),
    group: `节点组 ${(scope?.groups || []).join('、') || '未设置'}`,
    groups: `节点组 ${(scope?.groups || []).join('、') || '未设置'}`,
    os: `系统 ${(scope?.os || []).join('、') || '未设置'}`,
    tools: `指定工具 ${(scope?.tools || []).length || ''}`.trim(),
    declared: '按任务声明',
  };
  const colors = {
    all: 'green',
    nodes: 'blue',
    group: 'blue',
    groups: 'blue',
    os: 'purple',
    tools: 'geekblue',
    declared: 'gold',
    none: 'default',
  };
  return <Tag color={colors[mode] || 'default'}>{labels[mode] || mode}</Tag>;
}

function DescriptionsList({ rows }) {
  return (
    <div className="desc-list">
      {rows.map(([label, value]) => (
        <div className="desc-row" key={label}>
          <Text type="secondary">{label}</Text>
          <Text>{value || '未设置'}</Text>
        </div>
      ))}
    </div>
  );
}

function credentialLabel(value) {
  const labels = {
    active: '可用',
    not_configured: '未配置',
    disabled: '停用',
    expired: '已过期',
  };
  return labels[value] || value || '未配置';
}

function scopeLabel(scope, type) {
  const mode = scope?.mode || (type === 'node' ? 'none' : 'declared');
  if (mode === 'all') return type === 'node' ? '全部节点' : '全部工具';
  if (mode === 'none') return type === 'node' ? '不挂节点' : '不可调用工具';
  if (mode === 'nodes') return `指定节点：${(scope.nodes || []).join('、') || '未设置'}`;
  if (mode === 'group' || mode === 'groups') return `节点组：${(scope.groups || []).join('、') || '未设置'}`;
  if (mode === 'os') return `操作系统：${(scope.os || []).join('、') || '未设置'}`;
  if (mode === 'tools') return `指定工具：${(scope.tools || []).join('、') || '未设置'}`;
  if (mode === 'declared') return '按任务声明';
  return mode;
}

function stringifyShort(value) {
  const text = JSON.stringify(value || {});
  return text.length > 80 ? `${text.slice(0, 80)}...` : text;
}

function SubmitCommand({ nodes, workbenches = [], onDone, initialOverride }) {
  const [form] = Form.useForm();
  const [submitting, setSubmitting] = useState(false);
  const targetWorkbenches = useMemo(() => workbenchOptions(workbenches, nodes), [workbenches, nodes]);
  const initialValues = useMemo(() => ({
    title: '主机命令任务',
    taskType: 'command',
    program: 'hostname',
    operation: 'read',
    args: '',
    owner: 'worker-agent',
    targetMode: 'best',
    ...(initialOverride || {}),
  }), [initialOverride]);

  const submit = async (values) => {
    setSubmitting(true);
    try {
      const args = values.args
        ? values.args.split('\n').map((item) => item.trim()).filter(Boolean)
        : [];
      const taskType = values.taskType || 'command';
      const labels = ['compute', taskType === 'http_request' ? 'http_request' : taskType];
      if (values.targetMode === 'node' && values.node_id) labels.push(`node:${values.node_id}`);
      if (values.targetMode === 'workbench' && values.workbench_id) labels.push(`workbench:${values.workbench_id}`);
      if (values.targetMode === 'os' && values.os) labels.push(`os:${values.os}`);
      if (values.group) labels.push(`group:${values.group}`);
      if (values.prefer_node_id) labels.push(`prefer:${values.prefer_node_id}`);
      if (values.avoid_node_id) labels.push(`avoid:${values.avoid_node_id}`);
      const payload = buildTaskPayload(values, args);
      await fetchJson('/tasks', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          title: values.title,
          summary: '从 Web 总控台下发的执行任务。',
          created_by: 'architect-agent',
          owner: values.owner,
          assigned_to: [values.owner],
          labels,
          priority: values.priority || 'normal',
          inputs: [JSON.stringify(payload, null, 2)],
          outputs: ['结构化结果', '执行耗时', '执行日志'],
          acceptance_criteria: ['Hub 选择可执行节点', 'Worker 执行任务', '结果写回 Hub'],
          verify: buildVerifyFromForm(values),
        }),
      });
      message.success('任务已提交');
      onDone();
    } catch (error) {
      message.error(`提交失败：${error.message}`);
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <ProCard title="下发执行任务" bordered>
      <Form form={form} layout="vertical" initialValues={initialValues} onFinish={submit}>
        <Row gutter={16}>
          <Col span={10}><Form.Item name="title" label="任务标题" rules={[{ required: true }]}><Input /></Form.Item></Col>
          <Col span={6}><Form.Item name="owner" label="执行员工" rules={[{ required: true }]}><Input /></Form.Item></Col>
          <Col span={8}>
            <Form.Item name="targetMode" label="目标主机">
              <Select
                options={[
                  { value: 'best', label: '自动选择最优节点' },
                  { value: 'workbench', label: '指定电脑/工位' },
                  { value: 'node', label: '指定节点' },
                  { value: 'os', label: '指定操作系统' },
                ]}
              />
            </Form.Item>
          </Col>
        </Row>
        <Row gutter={16}>
          <Col span={6}>
            <Form.Item name="priority" label="任务优先级">
              <Select
                options={[
                  { value: 'normal', label: '普通' },
                  { value: 'high', label: '高' },
                  { value: 'p0', label: 'P0 紧急' },
                  { value: 'low', label: '低' },
                ]}
              />
            </Form.Item>
          </Col>
          <Col span={6}><Form.Item name="group" label="节点分组"><Input placeholder="linux / worker" /></Form.Item></Col>
          <Col span={6}>
            <Form.Item name="prefer_node_id" label="优先节点">
              <Select allowClear options={nodes.map((node) => ({ value: node.metadata.id, label: node.metadata.name }))} />
            </Form.Item>
          </Col>
          <Col span={6}>
            <Form.Item name="avoid_node_id" label="避开节点">
              <Select allowClear options={nodes.map((node) => ({ value: node.metadata.id, label: node.metadata.name }))} />
            </Form.Item>
          </Col>
        </Row>
        <Row gutter={16}>
          <Col span={8}>
            <Form.Item noStyle shouldUpdate>
              {({ getFieldValue }) => getFieldValue('targetMode') === 'workbench' ? (
                <Form.Item name="workbench_id" label="指定电脑/工位" rules={[{ required: true }]}>
                  <Select
                    showSearch
                    options={targetWorkbenches}
                    placeholder="选择一台电脑"
                    optionFilterProp="label"
                  />
                </Form.Item>
              ) : null}
            </Form.Item>
          </Col>
          <Col span={8}>
            <Form.Item noStyle shouldUpdate>
              {({ getFieldValue }) => getFieldValue('targetMode') === 'node' ? (
                <Form.Item name="node_id" label="指定节点">
                  <Select
                    options={nodes.map((node) => ({
                      value: node.metadata.id,
                      label: `${node.metadata.name} / ${node.spec.os}`,
                    }))}
                  />
                </Form.Item>
              ) : null}
            </Form.Item>
          </Col>
          <Col span={8}>
            <Form.Item noStyle shouldUpdate>
              {({ getFieldValue }) => getFieldValue('targetMode') === 'os' ? (
                <Form.Item name="os" label="操作系统">
                  <Select
                    options={[
                      { value: 'linux', label: 'Linux' },
                      { value: 'mac', label: 'macOS' },
                      { value: 'windows', label: 'Windows' },
                    ]}
                  />
                </Form.Item>
              ) : null}
            </Form.Item>
          </Col>
        </Row>
        <Row gutter={16}>
          <Col span={8}>
            <Form.Item name="taskType" label="任务类型">
              <Select
                options={[
                  { value: 'command', label: '命令' },
                  { value: 'file', label: '文件' },
                  { value: 'git', label: 'Git' },
                  { value: 'docker', label: 'Docker' },
                  { value: 'browser', label: '浏览器抓取' },
                  { value: 'session', label: '命令会话' },
                  { value: 'agentmessage', label: 'AgentMessage' },
                ]}
              />
            </Form.Item>
          </Col>
          <Col span={8}>
            <Form.Item name="operation" label="操作">
              <Input placeholder="read / list / status / ps / fetch" />
            </Form.Item>
          </Col>
          <Col span={8}><Form.Item name="timeout_seconds" label="超时秒数"><Input /></Form.Item></Col>
        </Row>
        <Row gutter={16}>
          <Col span={8}><Form.Item name="program" label="命令 / 路径 / URL / 镜像 / 仓库"><Input /></Form.Item></Col>
          <Col span={8}><Form.Item name="working_dir" label="工作目录"><Input /></Form.Item></Col>
          <Col span={8}><Form.Item name="extra" label="目标路径 / 分支 / 标题"><Input /></Form.Item></Col>
        </Row>
        <Row gutter={16}>
          <Col span={6}><Form.Item name="expect_exit_code" label="期望退出码"><InputNumber className="full" placeholder="0" /></Form.Item></Col>
          <Col span={9}><Form.Item name="expect_stdout_contains" label="stdout 必须包含"><Input /></Form.Item></Col>
          <Col span={9}><Form.Item name="expect_result_contains" label="结果文本必须包含"><Input /></Form.Item></Col>
        </Row>
        <Form.Item name="verify_json" label="高级验收 JSON">
          <Input.TextArea rows={4} placeholder='{"rules":[{"path":"result.exit_code","op":"eq","value":0}]}' />
        </Form.Item>
        <Form.Item name="args" label="参数或内容，每行一个"><Input.TextArea rows={6} /></Form.Item>
        <Button type="primary" htmlType="submit" loading={submitting}>提交执行任务</Button>
      </Form>
    </ProCard>
  );
}

function buildTaskPayload(values, args) {
  const timeout = Number(values.timeout_seconds || 30);
  switch (values.taskType) {
    case 'file':
      if (values.operation === 'upload') {
        return {
          type: 'file',
          operation: 'upload',
          path: values.program,
          content_base64: btoa(unescape(encodeURIComponent(values.args || ''))),
          create_dirs: true,
        };
      }
      if (values.operation === 'download') {
        return {
          type: 'file',
          operation: 'download',
          path: values.program,
          max_bytes: 5242880,
        };
      }
      if (values.operation === 'write') {
        return {
          type: 'file',
          operation: 'write',
          path: values.program,
          content: values.args || '',
          append: false,
          create_dirs: true,
        };
      }
      return {
        type: 'file',
        operation: values.operation || 'read',
        path: values.program,
        recursive: values.operation === 'list',
        max_bytes: 65536,
        max_entries: 200,
      };
    case 'git':
      return {
        type: 'git',
        operation: values.operation || 'status',
        repo: values.program,
        dest: values.extra,
        repo_dir: values.working_dir || values.program,
        branch: values.extra || null,
        reference: values.extra || null,
      };
    case 'docker':
      return {
        type: 'docker',
        operation: values.operation || 'ps',
        image: values.program,
        args,
        timeout_seconds: timeout,
      };
    case 'browser':
      if (values.operation === 'automate') {
        return {
          type: 'browser',
          operation: 'automate',
          url: values.program,
          actions: parseJsonOrDefault(values.args, []),
          screenshot_path: values.extra || null,
          timeout_seconds: timeout,
        };
      }
      return {
        type: 'browser',
        operation: 'fetch',
        url: values.program,
        selector: values.extra || null,
        timeout_seconds: timeout,
        max_response_bytes: 65536,
      };
    case 'session':
      return {
        type: 'session',
        operation: 'run',
        session_id: values.extra || null,
        program: values.program,
        args,
        working_dir: values.working_dir || null,
        timeout_seconds: timeout || 300,
      };
    case 'agentmessage':
      return {
        type: 'agent_message',
        from: 'architect-agent',
        to: args.length ? args : ['worker-agent'],
        message_type: 'broadcast.notice',
        subject: values.extra || values.title,
        summary: values.program || '',
        payload: {},
      };
    default:
      return {
        type: 'command',
        program: values.program,
        args,
        working_dir: values.working_dir || null,
        timeout_seconds: timeout,
      };
  }
}

function buildVerifyFromForm(values) {
  if (values.verify_json?.trim()) return parseJsonOrDefault(values.verify_json, null);
  const rules = [];
  if (values.expect_exit_code !== undefined && values.expect_exit_code !== null && values.expect_exit_code !== '') {
    rules.push({
      path: 'result.exit_code',
      op: 'eq',
      value: Number(values.expect_exit_code),
      description: '命令退出码符合预期',
    });
  }
  if (values.expect_stdout_contains) {
    rules.push({
      path: 'result.stdout',
      op: 'contains',
      value: values.expect_stdout_contains,
      description: 'stdout 包含预期文本',
    });
  }
  if (values.expect_result_contains) {
    rules.push({
      path: resultTextPath(values.taskType),
      op: 'contains',
      value: values.expect_result_contains,
      description: '结果文本包含预期内容',
    });
  }
  if (!rules.length) return null;
  return { rules };
}

function resultTextPath(taskType) {
  return {
    browser: 'result.text',
    http_request: 'result.body',
    file: 'result.content',
  }[taskType] || 'result.stdout';
}

function MessageList({ messages }) {
  if (!messages.length) {
    return <div className="empty-panel">暂无消息。任务、节点、Probe 和证据事件会出现在这里。</div>;
  }

  return (
    <div className="message-list">
      {messages.map((item) => (
        <div key={item.metadata.id} className="message-item">
          <span className="message-dot" />
          <div className="message-main">
            <Text strong className="message-subject">{item.spec.subject}</Text>
            <Text className="message-summary">{item.spec.summary}</Text>
            <Text type="secondary" className="message-meta">
              {`${item.metadata.from} -> ${(item.metadata.to || []).join(', ') || '-'} · ${formatTime(item.metadata.created_at)}`}
            </Text>
          </div>
          <Tag className="message-kind">{item.spec.type}</Tag>
        </div>
      ))}
    </div>
  );
}

function SubmitHttp({ onDone }) {
  const [form] = Form.useForm();
  const [submitting, setSubmitting] = useState(false);
  const initialValues = useMemo(() => ({
    title: 'HTTP 请求任务',
    method: 'GET',
    url: 'https://httpbin.org/get',
    owner: 'worker-agent',
  }), []);

  const submit = async (values) => {
    setSubmitting(true);
    try {
      const payload = {
        type: 'http_request',
        method: values.method,
        url: values.url,
        headers: [],
        body: values.body ? JSON.parse(values.body) : null,
        timeout_seconds: 30,
        max_response_bytes: 65536,
      };
      await fetchJson('/tasks', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          title: values.title,
          summary: '从 Web 总控台提交的 HTTP 请求任务。',
          created_by: 'architect-agent',
          owner: values.owner,
          assigned_to: [values.owner],
          labels: ['compute', 'http_request'],
          inputs: [JSON.stringify(payload, null, 2)],
          outputs: ['HTTP 状态码', '响应头', '响应体'],
          acceptance_criteria: ['Worker 执行请求', '结果写回 AgentGrid Hub'],
          verify: { presets: ['http.status_2xx'] },
        }),
      });
      message.success('任务已提交');
      onDone();
    } catch (error) {
      message.error(`提交失败：${error.message}`);
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <ProCard title="提交 HTTP 请求任务" bordered>
      <Form form={form} layout="vertical" initialValues={initialValues} onFinish={submit}>
        <Row gutter={16}>
          <Col span={12}><Form.Item name="title" label="任务标题" rules={[{ required: true }]}><Input /></Form.Item></Col>
          <Col span={4}><Form.Item name="method" label="方法" rules={[{ required: true }]}><Input /></Form.Item></Col>
          <Col span={8}><Form.Item name="owner" label="执行员工" rules={[{ required: true }]}><Input /></Form.Item></Col>
        </Row>
        <Form.Item name="url" label="URL" rules={[{ required: true }]}><Input prefix={<ApiOutlined />} /></Form.Item>
        <Form.Item name="body" label="请求体 JSON"><Input.TextArea rows={6} /></Form.Item>
        <Button type="primary" htmlType="submit" loading={submitting}>提交任务</Button>
      </Form>
    </ProCard>
  );
}

function VerificationTag({ verification }) {
  if (!verification) return <Tag>未配置</Tag>;
  const state = verification.state || (verification.passed ? 'passed' : 'failed');
  return <Tag color={verificationColor(state)}>{verificationLabel(state)}</Tag>;
}

function ArtifactPreview({ artifact, onClose }) {
  return (
    <Modal
      title={artifact?.spec?.name || '产物预览'}
      open={Boolean(artifact)}
      onCancel={onClose}
      footer={null}
      width={1040}
    >
      {artifact && (
        <div className="artifact-preview">
          <img src={artifactDataUrl(artifact)} alt={artifact.spec.name} />
        </div>
      )}
    </Modal>
  );
}

createRoot(document.getElementById('root')).render(<App />);
