#!/usr/bin/env python3
import argparse
import json
import os
import sqlite3
import time
import uuid
from datetime import datetime, timezone
from html import escape
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse


API_VERSION = "agentmessage.io/v1"
DEFAULT_PROJECT_ID = "agentgrid"
DEFAULT_PROJECT_NAME = "AgentGrid"

AGENT_NAME_FALLBACKS = {
    "architect-agent": "项目负责人",
    "protocol-agent": "协议工程师",
    "control-agent": "控制层工程师",
    "store-agent": "存储工程师",
    "worker-agent": "节点工程师",
    "executor-agent": "执行器工程师",
    "scheduler-agent": "调度工程师",
    "policy-agent": "安全策略工程师",
    "api-agent": "接口工程师",
    "cli-agent": "命令行工程师",
    "platform-agent": "跨平台工程师",
    "qa-agent": "测试工程师",
    "docs-agent": "文档工程师",
    "review-agent": "代码审查工程师",
}

MESSAGE_TYPE_LABELS = {
    "broadcast.notice": "公告",
    "task.assigned": "任务分配",
    "task.started": "任务开始",
    "task.progress": "任务进展",
    "task.blocked": "遇到阻塞",
    "task.completed": "任务完成",
    "contract.change_requested": "申请改接口",
    "contract.changed": "接口已变更",
    "review.requested": "请求审查",
    "review.comment": "审查意见",
    "review.approved": "审查通过",
    "review.changes_requested": "需要修改",
    "test.started": "测试开始",
    "test.passed": "测试通过",
    "test.failed": "测试失败",
    "decision.proposed": "提出决策",
    "decision.accepted": "决策通过",
    "decision.rejected": "决策拒绝",
    "agent.status_changed": "员工状态变化",
}

MESSAGE_TYPE_OPTIONS = [
    "broadcast.notice",
    "task.assigned",
    "task.progress",
    "task.blocked",
    "task.completed",
    "contract.changed",
    "review.requested",
    "review.comment",
    "test.failed",
    "test.passed",
    "decision.proposed",
    "decision.accepted",
]

SEED_AGENTS = [
    {
        "id": "architect-agent",
        "name": "项目负责人",
        "role": "项目负责人 / 架构负责人",
        "skills": ["总体架构", "模块边界", "版本路线", "任务拆解"],
        "permissions": ["创建任务", "调整优先级", "批准架构决策", "发送消息"],
        "responsibility": "负责 AgentGrid 的整体方向、模块边界、里程碑、技术取舍和跨团队协调。",
    },
    {
        "id": "protocol-agent",
        "name": "协议工程师",
        "role": "Protocol 工程师",
        "skills": ["AgentGrid 协议", "AgentMessage 协议", "JSON Schema", "API 契约"],
        "permissions": ["编辑协议", "创建 Schema", "发起接口变更", "发送消息"],
        "responsibility": "负责 Job、Node、Result、Policy、AgentMessage 等标准对象和协议兼容性。",
    },
    {
        "id": "control-agent",
        "name": "控制层工程师",
        "role": "Control Plane 工程师",
        "skills": ["任务队列", "节点注册", "心跳", "状态机", "租约"],
        "permissions": ["编辑控制层", "处理任务状态", "发送消息"],
        "responsibility": "负责 Control Plane：接收任务、管理节点、调度触发、结果接收和租约过期处理。",
    },
    {
        "id": "store-agent",
        "name": "存储工程师",
        "role": "Store 工程师",
        "skills": ["SQLite", "迁移", "事务", "数据模型"],
        "permissions": ["编辑存储层", "设计数据库表", "发送消息"],
        "responsibility": "负责 jobs、nodes、attempts、leases、events、messages 等数据表和持久化逻辑。",
    },
    {
        "id": "worker-agent",
        "name": "节点工程师",
        "role": "Worker 工程师",
        "skills": ["Worker 运行时", "资源上报", "任务拉取", "租约续期"],
        "permissions": ["编辑 Worker", "执行测试任务", "发送消息"],
        "responsibility": "负责每台机器上的 Worker：注册、心跳、领取任务、执行任务、回传结果。",
    },
    {
        "id": "executor-agent",
        "name": "执行器工程师",
        "role": "Executor 工程师",
        "skills": ["HTTP 执行器", "Command 执行器", "超时控制", "输出限制"],
        "permissions": ["编辑执行器", "运行本地命令测试", "发送消息"],
        "responsibility": "负责具体任务执行，包括 HTTP 请求、命令执行、stdout/stderr、错误映射和结果结构化。",
    },
    {
        "id": "scheduler-agent",
        "name": "调度工程师",
        "role": "Scheduler 工程师",
        "skills": ["资源匹配", "标签匹配", "优先级", "负载评分"],
        "permissions": ["编辑调度器", "调整调度策略", "发送消息"],
        "responsibility": "负责决定任务派给哪台机器，处理 capability、tag、CPU、内存、负载和优先级。",
    },
    {
        "id": "policy-agent",
        "name": "安全策略工程师",
        "role": "Policy / Security 工程师",
        "skills": ["权限策略", "URL 白名单", "命令白名单", "Secret 管理", "审计"],
        "permissions": ["编辑安全策略", "阻止高风险任务", "发送消息"],
        "responsibility": "负责 allow/deny/ask_user、安全边界、日志脱敏、secret_ref、审计事件和高风险操作治理。",
    },
    {
        "id": "api-agent",
        "name": "接口工程师",
        "role": "API 工程师",
        "skills": ["HTTP API", "认证", "错误码", "路由设计"],
        "permissions": ["编辑 API", "维护接口文档", "发送消息"],
        "responsibility": "负责 Hub 和 Compute 的 HTTP API、鉴权、错误码映射、请求响应格式和版本化。",
    },
    {
        "id": "cli-agent",
        "name": "命令行工程师",
        "role": "CLI 工程师",
        "skills": ["命令行体验", "JSON 输出", "开发者工具"],
        "permissions": ["编辑 CLI", "运行调试命令", "发送消息"],
        "responsibility": "负责 agentgrid 命令行，包括 submit、status、result、logs、cancel、nodes 和 --json 输出。",
    },
    {
        "id": "platform-agent",
        "name": "跨平台工程师",
        "role": "Platform 工程师",
        "skills": ["Linux", "macOS", "Windows", "服务安装", "资源检测"],
        "permissions": ["编辑平台层", "维护部署脚本", "发送消息"],
        "responsibility": "负责跨平台路径、后台服务、资源检测、打包和 Linux/macOS/Windows 差异处理。",
    },
    {
        "id": "qa-agent",
        "name": "测试工程师",
        "role": "QA / Integration 工程师",
        "skills": ["集成测试", "CI", "回归测试", "测试夹具"],
        "permissions": ["运行测试", "创建缺陷", "发送消息"],
        "responsibility": "负责单机和多 Worker 集成测试、HTTP Job、Command Job、租约过期和调度正确性验证。",
    },
    {
        "id": "review-agent",
        "name": "代码审查工程师",
        "role": "Review 工程师",
        "skills": ["代码审查", "架构一致性", "风险识别"],
        "permissions": ["审查变更", "阻止合并", "发送消息"],
        "responsibility": "负责审查模块边界、协议兼容性、安全风险和回归风险，必要时要求修改。",
    },
    {
        "id": "docs-agent",
        "name": "文档工程师",
        "role": "文档 / 开发者体验工程师",
        "skills": ["README", "快速开始", "示例", "开发者文档"],
        "permissions": ["编辑文档", "维护示例", "发送消息"],
        "responsibility": "负责需求文档、协议文档、API 文档、快速开始、示例任务和贡献说明。",
    },
]


def utc_now():
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def new_id(prefix):
    return f"{prefix}_{uuid.uuid4().hex}"


class HubStore:
    def __init__(self, db_path):
        self.db_path = db_path
        Path(db_path).parent.mkdir(parents=True, exist_ok=True)
        self.migrate()
        self.ensure_seed_data()

    def connect(self):
        conn = sqlite3.connect(self.db_path)
        conn.row_factory = sqlite3.Row
        return conn

    def migrate(self):
        with self.connect() as conn:
            conn.executescript(
                """
                CREATE TABLE IF NOT EXISTS projects (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    created_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS agents (
                    id TEXT PRIMARY KEY,
                    project_id TEXT NOT NULL,
                    name TEXT NOT NULL,
                    role TEXT NOT NULL,
                    skills_json TEXT NOT NULL,
                    permissions_json TEXT NOT NULL,
                    status TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS nodes (
                    id TEXT PRIMARY KEY,
                    project_id TEXT NOT NULL,
                    name TEXT NOT NULL,
                    os TEXT NOT NULL,
                    arch TEXT NOT NULL,
                    address TEXT NOT NULL DEFAULT '',
                    tags_json TEXT NOT NULL,
                    capabilities_json TEXT NOT NULL,
                    cpu_cores INTEGER NOT NULL DEFAULT 0,
                    memory_mb INTEGER NOT NULL DEFAULT 0,
                    running_jobs INTEGER NOT NULL DEFAULT 0,
                    status TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    last_heartbeat_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS agent_tasks (
                    id TEXT PRIMARY KEY,
                    project_id TEXT NOT NULL,
                    title TEXT NOT NULL,
                    summary TEXT NOT NULL DEFAULT '',
                    created_by TEXT NOT NULL DEFAULT 'architect-agent',
                    owner_agent_id TEXT,
                    status TEXT NOT NULL,
                    priority TEXT NOT NULL,
                    inputs_json TEXT NOT NULL,
                    outputs_json TEXT NOT NULL,
                    acceptance_criteria_json TEXT NOT NULL,
                    progress INTEGER NOT NULL DEFAULT 0,
                    blocked_reason TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS agent_task_events (
                    id TEXT PRIMARY KEY,
                    task_id TEXT NOT NULL,
                    project_id TEXT NOT NULL,
                    event_type TEXT NOT NULL,
                    from_state TEXT,
                    to_state TEXT,
                    progress INTEGER,
                    actor_agent_id TEXT NOT NULL,
                    message_id TEXT,
                    summary TEXT NOT NULL DEFAULT '',
                    payload_json TEXT NOT NULL DEFAULT '{}',
                    created_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS agent_messages (
                    id TEXT PRIMARY KEY,
                    project_id TEXT NOT NULL,
                    from_agent_id TEXT NOT NULL,
                    to_agents_json TEXT NOT NULL,
                    message_type TEXT NOT NULL,
                    subject TEXT NOT NULL,
                    summary TEXT NOT NULL,
                    priority TEXT NOT NULL,
                    requires_ack INTEGER NOT NULL,
                    payload_json TEXT NOT NULL,
                    created_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS agent_message_task_links (
                    message_id TEXT NOT NULL,
                    task_id TEXT NOT NULL,
                    relation TEXT NOT NULL DEFAULT 'related',
                    created_at TEXT NOT NULL,
                    PRIMARY KEY (message_id, task_id, relation)
                );

                CREATE INDEX IF NOT EXISTS idx_agent_messages_project_created
                    ON agent_messages(project_id, created_at DESC);

                CREATE INDEX IF NOT EXISTS idx_nodes_project_status
                    ON nodes(project_id, status, updated_at DESC);

                CREATE INDEX IF NOT EXISTS idx_agent_tasks_project_status
                    ON agent_tasks(project_id, status, updated_at DESC);

                CREATE INDEX IF NOT EXISTS idx_agent_tasks_owner_status
                    ON agent_tasks(owner_agent_id, status, updated_at DESC);

                CREATE INDEX IF NOT EXISTS idx_agent_task_events_task_created
                    ON agent_task_events(task_id, created_at);

                CREATE INDEX IF NOT EXISTS idx_agent_message_task_links_task
                    ON agent_message_task_links(task_id, created_at);
                """
            )
            self.ensure_column(conn, "agent_tasks", "summary", "TEXT NOT NULL DEFAULT ''")
            self.ensure_column(conn, "agent_tasks", "created_by", "TEXT NOT NULL DEFAULT 'architect-agent'")
            self.ensure_column(conn, "agent_tasks", "progress", "INTEGER NOT NULL DEFAULT 0")
            self.ensure_column(conn, "agent_tasks", "blocked_reason", "TEXT")
            self.ensure_column(conn, "agent_tasks", "assigned_to_json", "TEXT NOT NULL DEFAULT '[]'")
            self.ensure_column(conn, "agent_tasks", "labels_json", "TEXT NOT NULL DEFAULT '[]'")
            self.ensure_column(conn, "agent_tasks", "depends_on_json", "TEXT NOT NULL DEFAULT '[]'")
            self.ensure_column(conn, "agent_tasks", "due_at", "TEXT")
            self.ensure_column(conn, "agent_tasks", "started_at", "TEXT")
            self.ensure_column(conn, "agent_tasks", "completed_at", "TEXT")
            self.ensure_column(conn, "agent_tasks", "assignment_message_id", "TEXT")
            self.ensure_column(conn, "agent_tasks", "last_message_id", "TEXT")
            self.ensure_column(conn, "agent_tasks", "correlation_id", "TEXT")
            self.ensure_column(conn, "nodes", "address", "TEXT NOT NULL DEFAULT ''")

    def ensure_column(self, conn, table, column, definition):
        rows = conn.execute(f"PRAGMA table_info({table})").fetchall()
        if column not in {row["name"] for row in rows}:
            conn.execute(f"ALTER TABLE {table} ADD COLUMN {column} {definition}")

    def ensure_seed_data(self):
        with self.connect() as conn:
            now = utc_now()
            conn.execute(
                "INSERT OR IGNORE INTO projects (id, name, created_at) VALUES (?, ?, ?)",
                (DEFAULT_PROJECT_ID, DEFAULT_PROJECT_NAME, now),
            )
            for agent in SEED_AGENTS:
                conn.execute(
                    """
                    INSERT OR IGNORE INTO agents (
                        id, project_id, name, role, skills_json, permissions_json, status, created_at, updated_at
                    ) VALUES (?, ?, ?, ?, ?, ?, 'online', ?, ?)
                    """,
                    (
                        agent["id"],
                        DEFAULT_PROJECT_ID,
                        agent["name"],
                        agent["role"],
                        json.dumps(agent["skills"], ensure_ascii=False),
                        json.dumps(agent["permissions"], ensure_ascii=False),
                        now,
                        now,
                    ),
                )
                conn.execute(
                    """
                    UPDATE agents
                    SET name = ?, role = ?, skills_json = ?, permissions_json = ?, updated_at = ?
                    WHERE id = ?
                    """,
                    (
                        agent["name"],
                        agent["role"],
                        json.dumps(agent["skills"], ensure_ascii=False),
                        json.dumps(agent["permissions"], ensure_ascii=False),
                        now,
                        agent["id"],
                    ),
                )
            row = conn.execute(
                "SELECT COUNT(*) AS count FROM agent_messages WHERE project_id = ?",
                (DEFAULT_PROJECT_ID,),
            ).fetchone()
            if int(row["count"]) == 0:
                conn.execute(
                    """
                    INSERT INTO agent_messages (
                        id, project_id, from_agent_id, to_agents_json, message_type, subject,
                        summary, priority, requires_ack, payload_json, created_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    """,
                    (
                        new_id("msg"),
                        DEFAULT_PROJECT_ID,
                        "architect-agent",
                        json.dumps(["protocol-agent", "worker-agent", "qa-agent"]),
                        "broadcast.notice",
                        "AgentGrid Hub MVP online",
                        "AgentMessage stream is ready. Use this space for structured AI collaboration.",
                        "normal",
                        0,
                        json.dumps({"version": "0.1.0"}),
                        now,
                    ),
                )
            node_count = conn.execute(
                "SELECT COUNT(*) AS count FROM nodes WHERE project_id = ?",
                (DEFAULT_PROJECT_ID,),
            ).fetchone()
            if int(node_count["count"]) == 0:
                conn.execute(
                    """
                    INSERT INTO nodes (
                        id, project_id, name, os, arch, address, tags_json, capabilities_json,
                        cpu_cores, memory_mb, running_jobs, status, created_at, updated_at, last_heartbeat_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    """,
                    (
                        "hub-linux-01",
                        DEFAULT_PROJECT_ID,
                        "Hub Linux node",
                        "linux",
                        "unknown",
                        "hub.example.com",
                        json.dumps(["server", "linux"], ensure_ascii=False),
                        json.dumps(["http", "command", "agentmessage"], ensure_ascii=False),
                        0,
                        0,
                        0,
                        "online",
                        now,
                        now,
                        now,
                    ),
                )

    def message_count(self, project_id):
        with self.connect() as conn:
            row = conn.execute(
                "SELECT COUNT(*) AS count FROM agent_messages WHERE project_id = ?",
                (project_id,),
            ).fetchone()
            return int(row["count"])

    def list_agents(self, project_id=DEFAULT_PROJECT_ID):
        with self.connect() as conn:
            rows = conn.execute(
                """
                SELECT * FROM agents
                WHERE project_id = ?
                ORDER BY name ASC
                """,
                (project_id,),
            ).fetchall()
        return [agent_to_protocol(row) for row in rows]

    def upsert_agent(self, data):
        now = utc_now()
        agent_id = data.get("id") or new_id("agent")
        project_id = data.get("project_id") or DEFAULT_PROJECT_ID
        with self.connect() as conn:
            conn.execute(
                """
                INSERT INTO agents (
                    id, project_id, name, role, skills_json, permissions_json, status, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    role = excluded.role,
                    skills_json = excluded.skills_json,
                    permissions_json = excluded.permissions_json,
                    status = excluded.status,
                    updated_at = excluded.updated_at
                """,
                (
                    agent_id,
                    project_id,
                    required_str(data, "name"),
                    required_str(data, "role"),
                    json.dumps(data.get("skills", [])),
                    json.dumps(data.get("permissions", [])),
                    data.get("status", "online"),
                    now,
                    now,
                ),
            )
        return self.get_agent(agent_id)

    def get_agent(self, agent_id):
        with self.connect() as conn:
            row = conn.execute("SELECT * FROM agents WHERE id = ?", (agent_id,)).fetchone()
        if row is None:
            return None
        return agent_to_protocol(row)

    def list_nodes(self, project_id=DEFAULT_PROJECT_ID):
        with self.connect() as conn:
            rows = conn.execute(
                """
                SELECT * FROM nodes
                WHERE project_id = ?
                ORDER BY status ASC, updated_at DESC
                """,
                (project_id,),
            ).fetchall()
        return [node_to_protocol(row) for row in rows]

    def upsert_node(self, data):
        now = utc_now()
        node_id = optional_str(data, "id") or new_id("node")
        project_id = data.get("project_id") or DEFAULT_PROJECT_ID
        with self.connect() as conn:
            conn.execute(
                """
                INSERT INTO nodes (
                    id, project_id, name, os, arch, address, tags_json, capabilities_json,
                    cpu_cores, memory_mb, running_jobs, status, created_at, updated_at, last_heartbeat_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    os = excluded.os,
                    arch = excluded.arch,
                    address = excluded.address,
                    tags_json = excluded.tags_json,
                    capabilities_json = excluded.capabilities_json,
                    cpu_cores = excluded.cpu_cores,
                    memory_mb = excluded.memory_mb,
                    running_jobs = excluded.running_jobs,
                    status = excluded.status,
                    updated_at = excluded.updated_at,
                    last_heartbeat_at = excluded.last_heartbeat_at
                """,
                (
                    node_id,
                    project_id,
                    required_str(data, "name"),
                    optional_str(data, "os", "unknown") or "unknown",
                    optional_str(data, "arch", "unknown") or "unknown",
                    optional_str(data, "address"),
                    json.dumps(list_field(data, "tags"), ensure_ascii=False),
                    json.dumps(list_field(data, "capabilities"), ensure_ascii=False),
                    int(data.get("cpu_cores", 0) or 0),
                    int(data.get("memory_mb", 0) or 0),
                    int(data.get("running_jobs", 0) or 0),
                    data.get("status", "online"),
                    now,
                    now,
                    data.get("last_heartbeat_at") or now,
                ),
            )
        return self.get_node(node_id)

    def get_node(self, node_id):
        with self.connect() as conn:
            row = conn.execute("SELECT * FROM nodes WHERE id = ?", (node_id,)).fetchone()
        if row is None:
            return None
        return node_to_protocol(row)

    def delete_node(self, node_id):
        with self.connect() as conn:
            row = conn.execute("SELECT id FROM nodes WHERE id = ?", (node_id,)).fetchone()
            if row is None:
                raise KeyError(f"node not found: {node_id}")
            conn.execute("DELETE FROM nodes WHERE id = ?", (node_id,))
        return {"ok": True, "deleted": node_id}

    def list_messages(self, project_id=DEFAULT_PROJECT_ID, limit=100):
        with self.connect() as conn:
            rows = conn.execute(
                """
                SELECT * FROM agent_messages
                WHERE project_id = ?
                ORDER BY created_at DESC
                LIMIT ?
                """,
                (project_id, limit),
            ).fetchall()
        return [message_to_protocol(row) for row in rows]

    def create_message(self, data):
        now = utc_now()
        message_id = data.get("id") or new_id("msg")
        project_id = data.get("project_id") or DEFAULT_PROJECT_ID
        from_agent = data.get("from") or data.get("from_agent_id")
        if not from_agent:
            raise ValueError("from is required")
        message_type = required_str(data, "type")
        subject = required_str(data, "subject")
        summary = required_str(data, "summary")
        to_agents = data.get("to", [])
        if isinstance(to_agents, str):
            to_agents = [item.strip() for item in to_agents.split(",") if item.strip()]
        with self.connect() as conn:
            conn.execute(
                """
                INSERT INTO agent_messages (
                    id, project_id, from_agent_id, to_agents_json, message_type, subject,
                    summary, priority, requires_ack, payload_json, created_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    message_id,
                    project_id,
                    from_agent,
                    json.dumps(to_agents),
                    message_type,
                    subject,
                    summary,
                    data.get("priority", "normal"),
                    1 if data.get("requires_ack", False) else 0,
                    json.dumps(data.get("payload", {})),
                    now,
                ),
            )
        return self.get_message(message_id)

    def create_task(self, data):
        now = utc_now()
        task_id = optional_str(data, "id") or optional_str(data, "task_id") or new_id("task")
        project_id = data.get("project_id") or DEFAULT_PROJECT_ID
        created_by = required_str(data, "created_by")
        owner = optional_str(data, "owner", None)
        assigned_to = list_field(data, "assigned_to")
        if owner and owner not in assigned_to:
            assigned_to.insert(0, owner)
        status = "assigned" if assigned_to or owner else "todo"
        progress = int_field(data, "progress", 0)
        message_id = None
        with self.connect() as conn:
            existing = conn.execute("SELECT id FROM agent_tasks WHERE id = ?", (task_id,)).fetchone()
            if existing is not None:
                raise ValueError(f"task id already exists: {task_id}")
            conn.execute(
                """
                INSERT INTO agent_tasks (
                    id, project_id, title, summary, created_by, owner_agent_id, status, priority,
                    inputs_json, outputs_json, acceptance_criteria_json, progress, blocked_reason,
                    created_at, updated_at, assigned_to_json, labels_json, depends_on_json, due_at,
                    started_at, completed_at, assignment_message_id, last_message_id, correlation_id
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?, ?, ?, ?, ?, NULL, NULL, NULL, NULL, ?)
                """,
                (
                    task_id,
                    project_id,
                    required_str(data, "title"),
                    optional_str(data, "summary"),
                    created_by,
                    owner,
                    status,
                    data.get("priority", "normal"),
                    json.dumps(list_field(data, "inputs"), ensure_ascii=False),
                    json.dumps(list_field(data, "outputs"), ensure_ascii=False),
                    json.dumps(list_field(data, "acceptance_criteria"), ensure_ascii=False),
                    progress,
                    now,
                    now,
                    json.dumps(assigned_to, ensure_ascii=False),
                    json.dumps(list_field(data, "labels"), ensure_ascii=False),
                    json.dumps(list_field(data, "depends_on"), ensure_ascii=False),
                    data.get("due_at"),
                    data.get("correlation_id"),
                ),
            )
            self.insert_task_event(
                conn,
                task_id,
                project_id,
                "task.created",
                None,
                status,
                progress,
                created_by,
                None,
                optional_str(data, "summary"),
                data,
                now,
            )
            if status == "assigned":
                message_id = self.insert_message_row(
                    conn,
                    {
                        "project_id": project_id,
                        "from": created_by,
                        "to": assigned_to,
                        "type": "task.assigned",
                        "subject": f"任务：{required_str(data, 'title')}",
                        "summary": optional_str(data, "summary") or required_str(data, "title"),
                        "priority": data.get("priority", "normal"),
                        "requires_ack": data.get("requires_ack", True),
                        "payload": {
                            "task_id": task_id,
                            "title": required_str(data, "title"),
                            "owner": owner,
                            "inputs": list_field(data, "inputs"),
                            "outputs": list_field(data, "outputs"),
                            "acceptance_criteria": list_field(data, "acceptance_criteria"),
                        },
                    },
                    now,
                )
                self.link_message_task(conn, message_id, task_id, "assignment", now)
                conn.execute(
                    "UPDATE agent_tasks SET assignment_message_id = ?, last_message_id = ? WHERE id = ?",
                    (message_id, message_id, task_id),
                )
                self.insert_task_event(
                    conn,
                    task_id,
                    project_id,
                    "task.assigned",
                    None,
                    "assigned",
                    progress,
                    created_by,
                    message_id,
                    optional_str(data, "summary"),
                    data,
                    now,
                )
        return {"item": self.get_task(task_id), "message_id": message_id}

    def list_tasks(self, filters):
        project_id = filters.get("project_id", [DEFAULT_PROJECT_ID])[0]
        limit = min(int(filters.get("limit", ["50"])[0]), 200)
        clauses = ["project_id = ?"]
        values = [project_id]
        if filters.get("owner"):
            clauses.append("owner_agent_id = ?")
            values.append(filters["owner"][0])
        if filters.get("state"):
            clauses.append("status = ?")
            values.append(filters["state"][0])
        if filters.get("priority"):
            clauses.append("priority = ?")
            values.append(filters["priority"][0])
        if filters.get("updated_after"):
            clauses.append("updated_at > ?")
            values.append(filters["updated_after"][0])
        values.append(limit)
        with self.connect() as conn:
            rows = conn.execute(
                f"""
                SELECT * FROM agent_tasks
                WHERE {' AND '.join(clauses)}
                ORDER BY updated_at DESC
                LIMIT ?
                """,
                values,
            ).fetchall()
        return [task_to_protocol(row) for row in rows]

    def get_task(self, task_id):
        with self.connect() as conn:
            row = conn.execute("SELECT * FROM agent_tasks WHERE id = ?", (task_id,)).fetchone()
        if row is None:
            return None
        return task_to_protocol(row)

    def update_task_lifecycle(self, task_id, action, data):
        actor = required_str(data, "actor")
        now = utc_now()
        with self.connect() as conn:
            row = conn.execute("SELECT * FROM agent_tasks WHERE id = ?", (task_id,)).fetchone()
            if row is None:
                raise KeyError(f"task not found: {task_id}")
            current_state = row["status"]
            project_id = row["project_id"]
            notify = list_field(data, "notify") or [row["created_by"]]
            summary = optional_str(data, "summary") or self.default_task_summary(action, task_id)
            if action == "accept":
                if current_state in {"done", "cancelled"}:
                    raise ValueError(f"invalid transition from {current_state} to in_progress")
                next_state = "in_progress"
                progress = max(int(row["progress"]), int_field(data, "progress", 1), 1)
                message_type = "task.started"
                relation = "status_update"
                updates = "status = ?, progress = ?, blocked_reason = NULL, started_at = COALESCE(started_at, ?), updated_at = ?, last_message_id = ?"
            elif action == "progress":
                next_state = data.get("state") or current_state
                if next_state not in {"in_progress", "blocked", "review", "testing"}:
                    raise ValueError("state must be in_progress, blocked, review, or testing")
                progress = int_field(data, "progress", row["progress"])
                message_type = "task.progress"
                relation = "status_update"
                updates = "status = ?, progress = ?, updated_at = ?, last_message_id = ?"
            elif action == "block":
                next_state = "blocked"
                progress = int_field(data, "progress", row["progress"])
                message_type = "task.blocked"
                relation = "blocker"
                updates = "status = ?, progress = ?, blocked_reason = ?, updated_at = ?, last_message_id = ?"
            elif action == "complete":
                next_state = "done" if data.get("accepted", False) else "review"
                progress = 100
                message_type = "task.completed"
                relation = "completion"
                updates = "status = ?, progress = ?, blocked_reason = NULL, completed_at = ?, updated_at = ?, last_message_id = ?"
            else:
                raise ValueError(f"unknown task action: {action}")
            payload = {
                "task_id": task_id,
                "state": next_state,
                "progress": progress,
            }
            for key in ("reason", "needs", "files", "outputs", "checks"):
                if key in data:
                    payload[key] = data[key]
            message_id = self.insert_message_row(
                conn,
                {
                    "project_id": project_id,
                    "from": actor,
                    "to": notify,
                    "type": message_type,
                    "subject": data.get("subject") or self.default_task_subject(action, task_id, progress),
                    "summary": summary,
                    "priority": data.get("priority", row["priority"]),
                    "requires_ack": data.get("requires_ack", action == "complete"),
                    "payload": payload,
                },
                now,
            )
            self.link_message_task(conn, message_id, task_id, relation, now)
            if action == "accept":
                conn.execute(f"UPDATE agent_tasks SET {updates} WHERE id = ?", (next_state, progress, now, now, message_id, task_id))
            elif action == "progress":
                conn.execute(f"UPDATE agent_tasks SET {updates} WHERE id = ?", (next_state, progress, now, message_id, task_id))
            elif action == "block":
                reason = required_str(data, "reason")
                conn.execute(f"UPDATE agent_tasks SET {updates} WHERE id = ?", (next_state, progress, reason, now, message_id, task_id))
            elif action == "complete":
                completed_at = now if next_state == "done" else None
                conn.execute(f"UPDATE agent_tasks SET {updates} WHERE id = ?", (next_state, progress, completed_at, now, message_id, task_id))
            self.insert_task_event(
                conn,
                task_id,
                project_id,
                message_type,
                current_state,
                next_state,
                progress,
                actor,
                message_id,
                summary,
                payload,
                now,
            )
        return {"item": self.get_task(task_id), "message_id": message_id}

    def insert_message_row(self, conn, data, now):
        message_id = data.get("id") or new_id("msg")
        to_agents = data.get("to", [])
        if isinstance(to_agents, str):
            to_agents = [item.strip() for item in to_agents.split(",") if item.strip()]
        conn.execute(
            """
            INSERT INTO agent_messages (
                id, project_id, from_agent_id, to_agents_json, message_type, subject,
                summary, priority, requires_ack, payload_json, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                message_id,
                data.get("project_id") or DEFAULT_PROJECT_ID,
                data.get("from") or data.get("from_agent_id"),
                json.dumps(to_agents, ensure_ascii=False),
                data["type"],
                data["subject"],
                data["summary"],
                data.get("priority", "normal"),
                1 if data.get("requires_ack", False) else 0,
                json.dumps(data.get("payload", {}), ensure_ascii=False),
                now,
            ),
        )
        return message_id

    def link_message_task(self, conn, message_id, task_id, relation, now):
        conn.execute(
            """
            INSERT OR IGNORE INTO agent_message_task_links (message_id, task_id, relation, created_at)
            VALUES (?, ?, ?, ?)
            """,
            (message_id, task_id, relation, now),
        )

    def insert_task_event(self, conn, task_id, project_id, event_type, from_state, to_state, progress, actor, message_id, summary, payload, now):
        conn.execute(
            """
            INSERT INTO agent_task_events (
                id, task_id, project_id, event_type, from_state, to_state, progress,
                actor_agent_id, message_id, summary, payload_json, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                new_id("evt"),
                task_id,
                project_id,
                event_type,
                from_state,
                to_state,
                progress,
                actor,
                message_id,
                summary,
                json.dumps(payload, ensure_ascii=False),
                now,
            ),
        )

    def default_task_subject(self, action, task_id, progress):
        if action == "accept":
            return f"开始处理 {task_id}"
        if action == "progress":
            return f"{task_id} 进展 {progress}%"
        if action == "block":
            return f"{task_id} 遇到阻塞"
        if action == "complete":
            return f"{task_id} 已完成"
        return task_id

    def default_task_summary(self, action, task_id):
        labels = {
            "accept": "已开始处理任务。",
            "progress": "任务已有进展。",
            "block": "任务当前被阻塞。",
            "complete": "任务输出已完成。",
        }
        return labels.get(action, task_id)

    def get_message(self, message_id):
        with self.connect() as conn:
            row = conn.execute("SELECT * FROM agent_messages WHERE id = ?", (message_id,)).fetchone()
        if row is None:
            return None
        return message_to_protocol(row)

def required_str(data, key):
    value = data.get(key)
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"{key} is required")
    return value.strip()


def optional_str(data, key, default=""):
    value = data.get(key, default)
    if value is None:
        return default
    if not isinstance(value, str):
        raise ValueError(f"{key} must be a string")
    return value.strip()


def list_field(data, key):
    value = data.get(key, [])
    if value is None:
        return []
    if isinstance(value, str):
        return [item.strip() for item in value.split(",") if item.strip()]
    if isinstance(value, list):
        return value
    raise ValueError(f"{key} must be an array")


def int_field(data, key, default=0):
    value = data.get(key, default)
    try:
        parsed = int(value)
    except (TypeError, ValueError):
        raise ValueError(f"{key} must be an integer")
    if parsed < 0 or parsed > 100:
        raise ValueError(f"{key} must be between 0 and 100")
    return parsed


def json_loads(value, fallback):
    try:
        return json.loads(value)
    except Exception:
        return fallback


def agent_to_protocol(row):
    responsibility = next(
        (agent["responsibility"] for agent in SEED_AGENTS if agent["id"] == row["id"]),
        "",
    )
    return {
        "api_version": API_VERSION,
        "kind": "Agent",
        "metadata": {
            "id": row["id"],
            "project_id": row["project_id"],
            "name": row["name"],
            "created_at": row["created_at"],
            "updated_at": row["updated_at"],
        },
        "spec": {
            "role": row["role"],
            "skills": json_loads(row["skills_json"], []),
            "permissions": json_loads(row["permissions_json"], []),
            "responsibility": responsibility,
        },
        "status": {"state": row["status"]},
    }


def message_to_protocol(row):
    return {
        "api_version": API_VERSION,
        "kind": "AgentMessage",
        "metadata": {
            "id": row["id"],
            "project_id": row["project_id"],
            "from": row["from_agent_id"],
            "to": json_loads(row["to_agents_json"], []),
            "created_at": row["created_at"],
        },
        "spec": {
            "type": row["message_type"],
            "subject": row["subject"],
            "summary": row["summary"],
            "priority": row["priority"],
            "requires_ack": bool(row["requires_ack"]),
            "payload": json_loads(row["payload_json"], {}),
        },
    }


def node_to_protocol(row):
    return {
        "api_version": API_VERSION,
        "kind": "Node",
        "metadata": {
            "id": row["id"],
            "project_id": row["project_id"],
            "name": row["name"],
            "created_at": row["created_at"],
            "updated_at": row["updated_at"],
        },
        "spec": {
            "os": row["os"],
            "arch": row["arch"],
            "address": row["address"],
            "tags": json_loads(row["tags_json"], []),
            "capabilities": json_loads(row["capabilities_json"], []),
            "cpu_cores": row["cpu_cores"],
            "memory_mb": row["memory_mb"],
        },
        "status": {
            "state": row["status"],
            "running_jobs": row["running_jobs"],
            "last_heartbeat_at": row["last_heartbeat_at"],
        },
    }


def task_to_protocol(row):
    assigned_to = json_loads(row["assigned_to_json"], [])
    owner = row["owner_agent_id"]
    if owner and owner not in assigned_to:
        assigned_to = [owner] + assigned_to
    return {
        "api_version": API_VERSION,
        "kind": "AgentTask",
        "metadata": {
            "id": row["id"],
            "project_id": row["project_id"],
            "created_by": row["created_by"],
            "assigned_to": assigned_to,
            "created_at": row["created_at"],
            "updated_at": row["updated_at"],
            "correlation_id": row["correlation_id"],
        },
        "spec": {
            "title": row["title"],
            "summary": row["summary"],
            "owner": owner,
            "priority": row["priority"],
            "inputs": json_loads(row["inputs_json"], []),
            "outputs": json_loads(row["outputs_json"], []),
            "acceptance_criteria": json_loads(row["acceptance_criteria_json"], []),
            "labels": json_loads(row["labels_json"], []),
            "depends_on": json_loads(row["depends_on_json"], []),
            "due_at": row["due_at"],
        },
        "status": {
            "state": row["status"],
            "progress": row["progress"],
            "started_at": row["started_at"],
            "completed_at": row["completed_at"],
            "blocked_reason": row["blocked_reason"],
            "last_message_id": row["last_message_id"],
        },
    }


class HubHandler(BaseHTTPRequestHandler):
    store = None

    def log_message(self, fmt, *args):
        print("%s - - [%s] %s" % (self.address_string(), self.log_date_time_string(), fmt % args))

    def do_GET(self):
        try:
            path = urlparse(self.path).path
            if path == "/":
                self.respond_html(render_home(self.store))
            elif path == "/api/health":
                self.respond_json({"ok": True, "service": "agentgrid-hub", "time": utc_now()})
            elif path == "/api/agents":
                self.respond_json({"items": self.store.list_agents()})
            elif path == "/api/nodes":
                self.respond_json({"ok": True, "items": self.store.list_nodes()})
            elif path == "/api/messages":
                query = parse_qs(urlparse(self.path).query)
                limit = int(query.get("limit", ["100"])[0])
                self.respond_json({"items": self.store.list_messages(limit=min(limit, 500))})
            elif path == "/api/tasks":
                query = parse_qs(urlparse(self.path).query)
                self.respond_json({"ok": True, "items": self.store.list_tasks(query), "next_cursor": None})
            elif path.startswith("/api/tasks/"):
                task_id = path.removeprefix("/api/tasks/").strip("/")
                task = self.store.get_task(task_id)
                if task is None:
                    self.respond_json({"ok": False, "error": {"code": "not_found", "message": "Task not found"}}, HTTPStatus.NOT_FOUND)
                else:
                    self.respond_json({"ok": True, "item": task})
            else:
                self.respond_json({"ok": False, "error": {"code": "not_found", "message": "Not found"}}, HTTPStatus.NOT_FOUND)
        except Exception as error:
            self.respond_error(error)

    def do_POST(self):
        try:
            path = urlparse(self.path).path
            data = self.read_json()
            if path == "/api/agents":
                self.respond_json(self.store.upsert_agent(data), HTTPStatus.CREATED)
            elif path == "/api/nodes":
                self.respond_json({"ok": True, "item": self.store.upsert_node(data)}, HTTPStatus.CREATED)
            elif path == "/api/messages":
                self.respond_json(self.store.create_message(data), HTTPStatus.CREATED)
            elif path == "/api/tasks":
                self.respond_json({"ok": True, **self.store.create_task(data)}, HTTPStatus.CREATED)
            elif path.startswith("/api/tasks/"):
                parts = path.strip("/").split("/")
                if len(parts) == 4 and parts[0] == "api" and parts[1] == "tasks":
                    _, _, task_id, action = parts
                    if action not in {"accept", "progress", "block", "complete"}:
                        self.respond_json({"ok": False, "error": {"code": "not_found", "message": "Not found"}}, HTTPStatus.NOT_FOUND)
                    else:
                        self.respond_json({"ok": True, **self.store.update_task_lifecycle(task_id, action, data)})
                else:
                    self.respond_json({"ok": False, "error": {"code": "not_found", "message": "Not found"}}, HTTPStatus.NOT_FOUND)
            else:
                self.respond_json({"ok": False, "error": {"code": "not_found", "message": "Not found"}}, HTTPStatus.NOT_FOUND)
        except Exception as error:
            self.respond_error(error)

    def do_DELETE(self):
        try:
            path = urlparse(self.path).path
            if path.startswith("/api/nodes/"):
                node_id = path.removeprefix("/api/nodes/").strip("/")
                self.respond_json(self.store.delete_node(node_id))
            else:
                self.respond_json({"ok": False, "error": {"code": "not_found", "message": "Not found"}}, HTTPStatus.NOT_FOUND)
        except Exception as error:
            self.respond_error(error)

    def read_json(self):
        length = int(self.headers.get("content-length", "0"))
        if length <= 0:
            return {}
        return json.loads(self.rfile.read(length).decode("utf-8"))

    def respond_html(self, body, status=HTTPStatus.OK):
        encoded = body.encode("utf-8")
        self.send_response(status)
        self.send_header("content-type", "text/html; charset=utf-8")
        self.send_header("content-length", str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)

    def respond_json(self, body, status=HTTPStatus.OK):
        encoded = json.dumps(body, ensure_ascii=False, indent=2).encode("utf-8")
        self.send_response(status)
        self.send_header("content-type", "application/json; charset=utf-8")
        self.send_header("content-length", str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)

    def respond_error(self, error):
        if isinstance(error, KeyError):
            status = HTTPStatus.NOT_FOUND
            code = "not_found"
        elif isinstance(error, (ValueError, json.JSONDecodeError)):
            status = HTTPStatus.BAD_REQUEST
            code = "bad_request"
        else:
            status = HTTPStatus.INTERNAL_SERVER_ERROR
            code = "internal_error"
        self.respond_json(
            {
                "ok": False,
                "error": {
                    "code": code,
                    "message": str(error),
                }
            },
            status,
        )


def render_home(store):
    agents = store.list_agents()
    messages = store.list_messages(limit=100)
    tasks = store.list_tasks({})
    stats = build_dashboard_stats(messages, agents, tasks)
    agent_options = "\n".join(
        f'<option value="{escape(agent["metadata"]["id"])}">{escape(display_agent_name(agent))} · {escape(display_role(agent["spec"]["role"]))}</option>'
        for agent in agents
    )
    type_options = "\n".join(
        f'<option value="{escape(value)}">{escape(message_type_label(value))}</option>'
        for value in MESSAGE_TYPE_OPTIONS
    )
    agent_cards = "\n".join(
        f"""
        <article class="agent">
          <div class="agent-top">
            <strong>{escape(display_agent_name(agent))}</strong>
            <span class="status">在线</span>
          </div>
          <span>{escape(display_role(agent["spec"]["role"]))}</span>
          <small>{escape(agent["metadata"]["id"])}</small>
          <details>
            <summary>职责</summary>
            <p class="responsibility">{escape(agent["spec"].get("responsibility", ""))}</p>
          </details>
        </article>
        """
        for agent in agents
    )
    message_cards = "\n".join(render_message_card(message) for message in messages)
    task_cards = "\n".join(render_task_card(task) for task in tasks[:12])
    return f"""<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>AgentGrid 协作中心</title>
  <link href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.3/dist/css/bootstrap.min.css" rel="stylesheet">
  <style>
    :root {{
      color-scheme: light;
      --bg: #f8fafc;
      --panel: #ffffff;
      --text: #111827;
      --muted: #64748b;
      --line: #e2e8f0;
      --accent: #2563eb;
      --accent-dark: #1d4ed8;
      --soft: #eff6ff;
      --good: #059669;
      --bad: #dc2626;
    }}
    * {{ box-sizing: border-box; }}
    body {{
      margin: 0;
      background: var(--bg);
      color: var(--text);
      font: 14px/1.5 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }}
    .topbar {{
      background: #0f172a;
      color: #fff;
      padding: 14px 22px;
      position: sticky;
      top: 0;
      z-index: 10;
      box-shadow: 0 1px 0 rgba(15, 23, 42, .08);
    }}
    .topbar h1 {{
      font-size: 18px;
      margin: 0;
      letter-spacing: 0;
      font-weight: 700;
    }}
    .topbar .subtitle {{ color: #cbd5e1; font-size: 13px; }}
    .shell {{
      display: grid;
      grid-template-columns: 280px minmax(0, 1fr);
      gap: 16px;
      padding: 16px;
      max-width: 1500px;
      margin: 0 auto;
    }}
    .panel {{
      background: var(--panel);
      border: 1px solid var(--line);
      border-radius: 12px;
      box-shadow: 0 1px 2px rgba(15, 23, 42, .04);
    }}
    .stat {{
      border: 1px solid var(--line);
      border-radius: 12px;
      padding: 14px 16px;
      background: #fff;
      min-height: 86px;
    }}
    .stat strong {{
      display: block;
      font-size: 28px;
      line-height: 1.1;
      margin-bottom: 3px;
    }}
    .stat span {{ color: var(--muted); font-size: 13px; }}
    .sidebar {{
      height: calc(100vh - 96px);
      overflow: hidden;
      position: sticky;
      top: 78px;
    }}
    .agent-list {{
      height: calc(100vh - 160px);
      overflow: auto;
      padding: 8px;
    }}
    .agent {{
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 8px;
      padding: 10px 12px;
      border-radius: 10px;
      border: 1px solid transparent;
    }}
    .agent:hover {{
      background: #f8fafc;
      border-color: var(--line);
    }}
    .agent-name {{ font-weight: 700; }}
    .agent-role, .agent-id, .meta, .payload {{ color: var(--muted); }}
    .agent-role, .agent-id {{ font-size: 12px; }}
    .dot {{
      width: 8px;
      height: 8px;
      border-radius: 999px;
      background: var(--good);
      flex: 0 0 auto;
    }}
    details summary {{
      cursor: pointer;
      color: var(--muted);
      font-size: 12px;
      user-select: none;
    }}
    .composer-wrap {{
      padding: 0;
      overflow: hidden;
      margin-bottom: 16px;
    }}
    .composer-wrap > summary {{
      list-style: none;
      padding: 14px 16px;
      color: var(--text);
      font-size: 14px;
      font-weight: 700;
      border-bottom: 1px solid transparent;
    }}
    .composer-wrap[open] > summary {{
      border-bottom-color: var(--line);
    }}
    .composer {{
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 12px;
      padding: 16px;
    }}
    .composer label {{ display: grid; gap: 5px; color: var(--muted); }}
    .composer .full {{ grid-column: 1 / -1; }}
    input, select, textarea {{
      width: 100%;
      border: 1px solid var(--line);
      border-radius: 6px;
      padding: 9px 10px;
      font: inherit;
      background: #fff;
      color: var(--text);
    }}
    textarea {{ min-height: 84px; resize: vertical; }}
    button {{
      border: 0;
      border-radius: 10px;
      padding: 10px 14px;
      background: var(--accent);
      color: #fff;
      font-weight: 600;
      cursor: pointer;
    }}
    button:hover {{ background: var(--accent-dark); }}
    .stream {{ display: grid; gap: 10px; }}
    .message {{
      border: 1px solid var(--line);
      border-left: 4px solid #cbd5e1;
      border-radius: 12px;
      padding: 14px 16px;
      background: #fff;
    }}
    .message.work {{ border-left-color: var(--accent); }}
    .message.done {{ border-left-color: var(--good); }}
    .message.blocking {{ border-left-color: var(--bad); }}
    .task-board {{
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 10px;
    }}
    .task-card {{
      border: 1px solid var(--line);
      border-radius: 12px;
      padding: 14px;
      background: #fff;
    }}
    .progress {{
      height: 7px;
      background: #e2e8f0;
    }}
    .progress-bar {{
      background: var(--accent);
    }}
    .message-header {{
      display: flex;
      justify-content: space-between;
      gap: 12px;
      margin-bottom: 6px;
    }}
    .type {{
      display: inline-flex;
      align-items: center;
      border-radius: 999px;
      background: var(--soft);
      color: var(--accent-dark);
      padding: 3px 10px;
      font-size: 12px;
      white-space: nowrap;
      font-weight: 600;
    }}
    .type.blocking {{ background: #fef2f2; color: var(--bad); }}
    .type.done {{ background: #ecfdf5; color: var(--good); }}
    .type.work {{ background: #eff6ff; color: var(--accent); }}
    .summary {{ margin: 6px 0 0; }}
    .empty-help {{
      border: 1px dashed var(--line);
      border-radius: 6px;
      color: var(--muted);
      padding: 14px;
      background: #fafafa;
    }}
    .payload {{
      margin-top: 8px;
      white-space: pre-wrap;
      word-break: break-word;
      font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
      font-size: 12px;
    }}
    @media (max-width: 980px) {{
      .shell {{ grid-template-columns: 1fr; padding: 12px; }}
      .sidebar {{ height: auto; position: static; }}
      .agent-list {{ height: auto; max-height: 360px; }}
      .task-board {{ grid-template-columns: 1fr; }}
      .composer {{ grid-template-columns: 1fr; }}
    }}
  </style>
</head>
<body>
  <header class="topbar">
    <div class="d-flex align-items-center justify-content-between gap-3">
      <div>
      <h1>AgentGrid 协作中心</h1>
      <div class="subtitle">项目调度、员工进展、审查意见和阻塞项。</div>
      </div>
      <div class="badge rounded-pill text-bg-light text-dark">项目：AgentGrid</div>
    </div>
  </header>
  <main class="shell">
    <aside class="panel sidebar">
      <div class="p-3 border-bottom">
        <div class="fw-bold fs-5">AI 员工</div>
        <div class="text-secondary small">当前协作成员</div>
      </div>
      <div class="agent-list">
        <div class="agents">{agent_cards}</div>
      </div>
    </aside>
    <div>
      <section class="row g-3 mb-3" aria-label="项目状态">
        <div class="col-6 col-lg"><div class="stat"><strong>{stats["agents"]}</strong><span>在线员工</span></div></div>
        <div class="col-6 col-lg"><div class="stat"><strong>{stats["assigned"]}</strong><span>已分配任务</span></div></div>
        <div class="col-6 col-lg"><div class="stat"><strong>{stats["completed"]}</strong><span>完成汇报</span></div></div>
        <div class="col-6 col-lg"><div class="stat"><strong>{stats["review"]}</strong><span>审查相关</span></div></div>
        <div class="col-6 col-lg"><div class="stat"><strong>{stats["blocked"]}</strong><span>阻塞/失败</span></div></div>
      </section>
      <details class="composer-wrap panel">
        <summary>发送协作消息</summary>
        <form class="composer" method="post" action="api/messages" onsubmit="return sendMessage(event)">
          <label>发送者
            <select name="from">{agent_options}</select>
          </label>
          <label>消息类型
            <select name="type">{type_options}</select>
          </label>
          <label class="full">接收者，逗号分隔
            <input name="to" placeholder="例如：protocol-agent, worker-agent；不填表示给所有人看">
          </label>
          <label class="full">主题
            <input name="subject" required placeholder="例如：HTTP 请求任务接口已经确定">
          </label>
          <label class="full">内容
            <textarea name="summary" required placeholder="用普通中文写清楚：发生了什么、谁需要处理、下一步是什么"></textarea>
          </label>
          <button type="submit">发送消息</button>
        </form>
      </details>
      <section class="panel p-3 mb-3">
        <div class="d-flex align-items-center justify-content-between mb-3">
          <div>
            <h2 class="fs-5 m-0">任务看板</h2>
            <div class="text-secondary small">正式 AgentTask 对象，最多显示最近 12 个</div>
          </div>
          <span class="badge text-bg-primary">{stats["tasks"]} 个任务</span>
        </div>
        <div class="task-board">{task_cards or '<div class="empty-help">还没有正式任务。</div>'}</div>
      </section>
      <section class="panel p-3">
        <div class="d-flex align-items-center justify-content-between mb-3">
          <div>
            <h2 class="fs-5 m-0">项目沟通记录</h2>
            <div class="text-secondary small">按时间倒序展示最近 100 条消息</div>
          </div>
        </div>
        <div class="stream">{message_cards or '<div class="empty-help">还没有消息。</div>'}</div>
      </section>
    </div>
  </main>
  <script>
    async function sendMessage(event) {{
      event.preventDefault();
      const form = event.target;
      const data = Object.fromEntries(new FormData(form).entries());
      data.to = data.to ? data.to.split(',').map(x => x.trim()).filter(Boolean) : [];
      data.priority = 'normal';
      data.requires_ack = false;
      data.payload = {{ source: 'web' }};
      const res = await fetch('api/messages', {{
        method: 'POST',
        headers: {{ 'content-type': 'application/json' }},
        body: JSON.stringify(data)
      }});
      if (!res.ok) {{
        alert(await res.text());
        return false;
      }}
      location.reload();
      return false;
    }}
  </script>
</body>
</html>"""


def render_message_card(message):
    metadata = message["metadata"]
    spec = message["spec"]
    payload = json.dumps(spec.get("payload", {}), ensure_ascii=False, indent=2)
    to_text = ", ".join(display_agent_id(agent_id) for agent_id in metadata.get("to", [])) or "所有员工"
    type_class = message_type_class(spec["type"], spec.get("payload", {}))
    return f"""
    <article class="message">
      <div class="message-header">
        <strong>{escape(spec["subject"])}</strong>
        <span class="type {type_class}">{escape(message_type_label(spec["type"]))}</span>
      </div>
      <div class="meta">{escape(display_agent_id(metadata["from"]))} 发给 {escape(to_text)} · {escape(format_time(metadata["created_at"]))}</div>
      <p class="summary">{escape(spec["summary"])}</p>
      {render_payload(payload)}
    </article>
    """


def render_task_card(task):
    metadata = task["metadata"]
    spec = task["spec"]
    status = task["status"]
    state = status["state"]
    owner = spec.get("owner") or "-"
    return f"""
    <article class="task-card">
      <div class="d-flex align-items-start justify-content-between gap-2">
        <div>
          <strong>{escape(spec["title"])}</strong>
          <div class="meta">{escape(display_agent_id(owner))} · {escape(metadata["id"])}</div>
        </div>
        <span class="type {message_type_class('task.blocked' if state == 'blocked' else 'task.completed' if state in {'review', 'done'} else 'task.progress', {})}">{escape(task_state_label(state))}</span>
      </div>
      <p class="summary">{escape(spec.get("summary", ""))}</p>
      <div class="progress" role="progressbar" aria-valuenow="{int(status.get("progress") or 0)}" aria-valuemin="0" aria-valuemax="100">
        <div class="progress-bar" style="width: {int(status.get("progress") or 0)}%"></div>
      </div>
    </article>
    """


def render_payload(payload):
    if payload == "{}":
        return ""
    return f'<details><summary>查看结构化数据</summary><div class="payload">{escape(payload)}</div></details>'


def message_type_label(value):
    return MESSAGE_TYPE_LABELS.get(value, value)


def message_type_class(value, payload):
    if value in {"task.blocked", "test.failed"} or payload.get("blocking"):
        return "blocking"
    if value in {"task.completed", "test.passed", "review.approved", "decision.accepted"}:
        return "done"
    if value.startswith("task.") or value in {"review.requested", "contract.changed"}:
        return "work"
    return ""


def build_dashboard_stats(messages, agents, tasks):
    stats = {
        "agents": len(agents),
        "tasks": len(tasks),
        "assigned": 0,
        "completed": 0,
        "review": 0,
        "blocked": 0,
    }
    for message in messages:
        spec = message["spec"]
        msg_type = spec["type"]
        payload = spec.get("payload", {})
        if msg_type == "task.assigned":
            stats["assigned"] += 1
        if msg_type == "task.completed":
            stats["completed"] += 1
        if msg_type.startswith("review."):
            stats["review"] += 1
        if msg_type in {"task.blocked", "test.failed"} or payload.get("blocking"):
            stats["blocked"] += 1
    return stats


def task_state_label(state):
    return {
        "todo": "待分配",
        "assigned": "已分配",
        "in_progress": "处理中",
        "blocked": "阻塞",
        "review": "待审查",
        "testing": "测试中",
        "done": "已完成",
        "cancelled": "已取消",
    }.get(state, state)


def display_agent_id(agent_id):
    return AGENT_NAME_FALLBACKS.get(agent_id, agent_id)


def display_agent_name(agent):
    return AGENT_NAME_FALLBACKS.get(agent["metadata"]["id"], agent["metadata"]["name"])


def display_role(role):
    labels = {
        "Project Lead": "项目负责人",
        "Protocol Engineer": "协议工程师",
        "Worker Engineer": "节点工程师",
        "QA Engineer": "测试工程师",
    }
    return labels.get(role, role)


def format_time(value):
    if value.endswith("Z"):
        value = value[:-1] + "+00:00"
    try:
        dt = datetime.fromisoformat(value).astimezone()
        return dt.strftime("%Y-%m-%d %H:%M:%S")
    except Exception:
        return value


def main():
    parser = argparse.ArgumentParser(description="AgentGrid Hub MVP")
    parser.add_argument("--host", default=os.environ.get("AGENTGRID_HUB_HOST", "127.0.0.1"))
    parser.add_argument("--port", type=int, default=int(os.environ.get("AGENTGRID_HUB_PORT", "20080")))
    parser.add_argument("--db", default=os.environ.get("AGENTGRID_HUB_DB", "./data/agentgrid-hub.db"))
    args = parser.parse_args()

    HubHandler.store = HubStore(args.db)
    server = ThreadingHTTPServer((args.host, args.port), HubHandler)
    print(f"AgentGrid Hub listening on http://{args.host}:{args.port}")
    print(f"Database: {args.db}")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
