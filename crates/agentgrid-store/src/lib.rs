use std::path::Path;

use agentgrid_protocol::{
    AgentTask, AgentTaskMetadata, AgentTaskSpec, AgentTaskState, AgentTaskStatus, Priority,
    AGENTMESSAGE_V1,
};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("time parse error: {0}")]
    Time(#[from] chrono::ParseError),
}

pub type Result<T> = std::result::Result<T, StoreError>;

pub struct AgentGridStore {
    conn: Connection,
}

impl AgentGridStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    pub fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS agent_tasks (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                title TEXT NOT NULL,
                summary TEXT NOT NULL,
                created_by TEXT NOT NULL,
                owner_agent_id TEXT,
                status TEXT NOT NULL,
                priority TEXT NOT NULL,
                inputs_json TEXT NOT NULL,
                outputs_json TEXT NOT NULL,
                acceptance_criteria_json TEXT NOT NULL,
                progress INTEGER NOT NULL,
                blocked_reason TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_agent_tasks_status
                ON agent_tasks(project_id, status, updated_at);
            ",
        )?;
        Ok(())
    }

    pub fn create_task(&self, task: &AgentTask) -> Result<()> {
        self.conn.execute(
            "INSERT INTO agent_tasks (
                id, project_id, title, summary, created_by, owner_agent_id, status, priority,
                inputs_json, outputs_json, acceptance_criteria_json, progress, blocked_reason,
                created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                task.metadata.id,
                task.metadata.project_id,
                task.spec.title,
                task.spec.summary,
                task.metadata.created_by,
                task.spec.owner,
                format!("{:?}", task.status.state).to_lowercase(),
                format!("{:?}", task.spec.priority).to_lowercase(),
                serde_json::to_string(&task.spec.inputs)?,
                serde_json::to_string(&task.spec.outputs)?,
                serde_json::to_string(&task.spec.acceptance_criteria)?,
                task.status.progress,
                task.status.blocked_reason,
                task.metadata.created_at.to_rfc3339(),
                task.metadata.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn get_task(&self, id: &str) -> Result<Option<AgentTask>> {
        self.conn
            .query_row(
                "SELECT * FROM agent_tasks WHERE id = ?1",
                params![id],
                row_to_task,
            )
            .optional()
            .map_err(StoreError::Database)
    }

    pub fn list_tasks(&self, project_id: &str) -> Result<Vec<AgentTask>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM agent_tasks WHERE project_id = ?1 ORDER BY updated_at DESC")?;
        let rows = stmt.query_map(params![project_id], row_to_task)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(StoreError::Database)
    }
}

fn row_to_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentTask> {
    let created_at = parse_time(row.get::<_, String>("created_at")?.as_str())?;
    let updated_at = parse_time(row.get::<_, String>("updated_at")?.as_str())?;
    let state = parse_task_state(&row.get::<_, String>("status")?);
    let priority = parse_priority(&row.get::<_, String>("priority")?);
    let owner: Option<String> = row.get("owner_agent_id")?;

    Ok(AgentTask {
        api_version: AGENTMESSAGE_V1.to_string(),
        kind: "AgentTask".to_string(),
        metadata: AgentTaskMetadata {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            created_by: row.get("created_by")?,
            assigned_to: owner.clone().into_iter().collect(),
            created_at,
            updated_at,
            correlation_id: None,
        },
        spec: AgentTaskSpec {
            title: row.get("title")?,
            summary: row.get("summary")?,
            owner,
            priority,
            inputs: json_column(row, "inputs_json")?,
            outputs: json_column(row, "outputs_json")?,
            acceptance_criteria: json_column(row, "acceptance_criteria_json")?,
            labels: Vec::new(),
            depends_on: Vec::new(),
            due_at: None,
        },
        status: AgentTaskStatus {
            state,
            progress: row.get::<_, i64>("progress")? as u8,
            started_at: None,
            completed_at: None,
            blocked_reason: row.get("blocked_reason")?,
            last_message_id: None,
        },
    })
}

fn json_column<T: serde::de::DeserializeOwned>(
    row: &rusqlite::Row<'_>,
    name: &str,
) -> rusqlite::Result<T> {
    let raw: String = row.get(name)?;
    serde_json::from_str(&raw).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn parse_time(value: &str) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|time| time.with_timezone(&Utc))
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })
}

fn parse_task_state(value: &str) -> AgentTaskState {
    match value {
        "todo" => AgentTaskState::Todo,
        "assigned" => AgentTaskState::Assigned,
        "inprogress" | "in_progress" => AgentTaskState::InProgress,
        "blocked" => AgentTaskState::Blocked,
        "review" => AgentTaskState::Review,
        "testing" => AgentTaskState::Testing,
        "done" => AgentTaskState::Done,
        "cancelled" => AgentTaskState::Cancelled,
        _ => AgentTaskState::Todo,
    }
}

fn parse_priority(value: &str) -> Priority {
    match value {
        "p0" => Priority::P0,
        "p1" => Priority::P1,
        "p2" => Priority::P2,
        "low" => Priority::Low,
        "high" => Priority::High,
        _ => Priority::Normal,
    }
}
