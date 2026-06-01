use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use directories::ProjectDirs;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, SchedulerError>;

#[derive(Debug, Error)]
pub enum SchedulerError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("time parse error: {0}")]
    TimeParse(#[from] chrono::ParseError),
    #[error("task not found: {0}")]
    TaskNotFound(String),
    #[error("could not resolve application data directory")]
    DataDirUnavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub schedule: Schedule,
    pub action: Action,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub next_run_at: Option<DateTime<Utc>>,
    pub last_run_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Schedule {
    Once { run_at: DateTime<Utc> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Command {
        program: String,
        args: Vec<String>,
        working_dir: Option<PathBuf>,
        timeout_seconds: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewTask {
    pub title: String,
    pub description: Option<String>,
    pub schedule: Schedule,
    pub action: Action,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRun {
    pub id: String,
    pub task_id: String,
    pub status: TaskStatus,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub error_message: Option<String>,
}

#[derive(Debug)]
pub struct ExecutionResult {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub error_message: Option<String>,
}

impl Task {
    pub fn new(input: NewTask) -> Self {
        let now = Utc::now();
        let next_run_at = input.schedule.next_run_at();

        Self {
            id: format!("task_{}", Uuid::new_v4().simple()),
            title: input.title,
            description: input.description,
            schedule: input.schedule,
            action: input.action,
            status: TaskStatus::Pending,
            created_at: now,
            updated_at: now,
            next_run_at,
            last_run_at: None,
        }
    }
}

impl Schedule {
    pub fn next_run_at(&self) -> Option<DateTime<Utc>> {
        match self {
            Schedule::Once { run_at } => Some(*run_at),
        }
    }
}

pub fn default_db_path() -> Result<PathBuf> {
    let dirs = ProjectDirs::from("dev", "ai-task-scheduler", "AI Task Scheduler")
        .ok_or(SchedulerError::DataDirUnavailable)?;
    let data_dir = dirs.data_dir();
    fs::create_dir_all(data_dir)?;
    Ok(data_dir.join("scheduler.db"))
}

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_default() -> Result<Self> {
        Self::open(default_db_path()?)
    }

    pub fn create_task(&self, input: NewTask) -> Result<Task> {
        let task = Task::new(input);
        self.insert_task(&task)?;
        Ok(task)
    }

    pub fn list_tasks(&self) -> Result<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, schedule_json, action_json, status, created_at,
                    updated_at, next_run_at, last_run_at
             FROM tasks
             ORDER BY created_at DESC",
        )?;

        let rows = stmt.query_map([], row_to_task)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(SchedulerError::Database)
    }

    pub fn get_task(&self, id: &str) -> Result<Task> {
        self.conn
            .query_row(
                "SELECT id, title, description, schedule_json, action_json, status, created_at,
                        updated_at, next_run_at, last_run_at
                 FROM tasks
                 WHERE id = ?1",
                params![id],
                row_to_task,
            )
            .optional()?
            .ok_or_else(|| SchedulerError::TaskNotFound(id.to_string()))
    }

    pub fn cancel_task(&self, id: &str) -> Result<()> {
        let changed = self.conn.execute(
            "UPDATE tasks
             SET status = 'cancelled', updated_at = ?2, next_run_at = NULL
             WHERE id = ?1 AND status IN ('pending', 'running')",
            params![id, Utc::now().to_rfc3339()],
        )?;

        if changed == 0 {
            self.get_task(id)?;
        }

        Ok(())
    }

    pub fn find_due_tasks(&self, now: DateTime<Utc>) -> Result<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, schedule_json, action_json, status, created_at,
                    updated_at, next_run_at, last_run_at
             FROM tasks
             WHERE status = 'pending'
               AND next_run_at IS NOT NULL
               AND next_run_at <= ?1
             ORDER BY next_run_at ASC",
        )?;

        let rows = stmt.query_map(params![now.to_rfc3339()], row_to_task)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(SchedulerError::Database)
    }

    pub fn list_runs(&self, task_id: &str) -> Result<Vec<TaskRun>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, task_id, status, started_at, finished_at, exit_code, stdout, stderr,
                    error_message
             FROM task_runs
             WHERE task_id = ?1
             ORDER BY started_at DESC",
        )?;

        let rows = stmt.query_map(params![task_id], row_to_task_run)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(SchedulerError::Database)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT,
                schedule_json TEXT NOT NULL,
                action_json TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                next_run_at TEXT,
                last_run_at TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_tasks_due
                ON tasks(status, next_run_at);

            CREATE TABLE IF NOT EXISTS task_runs (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                status TEXT NOT NULL,
                started_at TEXT NOT NULL,
                finished_at TEXT NOT NULL,
                exit_code INTEGER,
                stdout TEXT NOT NULL,
                stderr TEXT NOT NULL,
                error_message TEXT,
                FOREIGN KEY(task_id) REFERENCES tasks(id)
            );
            ",
        )?;
        Ok(())
    }

    fn insert_task(&self, task: &Task) -> Result<()> {
        self.conn.execute(
            "INSERT INTO tasks (
                id, title, description, schedule_json, action_json, status, created_at,
                updated_at, next_run_at, last_run_at
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                task.id,
                task.title,
                task.description,
                serde_json::to_string(&task.schedule)?,
                serde_json::to_string(&task.action)?,
                status_to_str(task.status),
                task.created_at.to_rfc3339(),
                task.updated_at.to_rfc3339(),
                task.next_run_at.map(|time| time.to_rfc3339()),
                task.last_run_at.map(|time| time.to_rfc3339()),
            ],
        )?;
        Ok(())
    }

    fn mark_running(&self, id: &str, now: DateTime<Utc>) -> Result<()> {
        self.conn.execute(
            "UPDATE tasks
             SET status = 'running', updated_at = ?2
             WHERE id = ?1 AND status = 'pending'",
            params![id, now.to_rfc3339()],
        )?;
        Ok(())
    }

    fn finish_task(
        &self,
        task_id: &str,
        started_at: DateTime<Utc>,
        result: ExecutionResult,
    ) -> Result<TaskRun> {
        let finished_at = Utc::now();
        let status = if result.exit_code == Some(0) && result.error_message.is_none() {
            TaskStatus::Succeeded
        } else {
            TaskStatus::Failed
        };

        let run = TaskRun {
            id: format!("run_{}", Uuid::new_v4().simple()),
            task_id: task_id.to_string(),
            status,
            started_at,
            finished_at,
            exit_code: result.exit_code,
            stdout: result.stdout,
            stderr: result.stderr,
            error_message: result.error_message,
        };

        self.conn.execute(
            "INSERT INTO task_runs (
                id, task_id, status, started_at, finished_at, exit_code, stdout, stderr,
                error_message
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                run.id,
                run.task_id,
                status_to_str(run.status),
                run.started_at.to_rfc3339(),
                run.finished_at.to_rfc3339(),
                run.exit_code,
                run.stdout,
                run.stderr,
                run.error_message,
            ],
        )?;

        self.conn.execute(
            "UPDATE tasks
             SET status = ?2, updated_at = ?3, last_run_at = ?3, next_run_at = NULL
             WHERE id = ?1",
            params![task_id, status_to_str(status), finished_at.to_rfc3339()],
        )?;

        Ok(run)
    }
}

pub fn run_due_tasks_once(store: &Store) -> Result<Vec<TaskRun>> {
    let due_tasks = store.find_due_tasks(Utc::now())?;
    let mut runs = Vec::with_capacity(due_tasks.len());

    for task in due_tasks {
        let started_at = Utc::now();
        store.mark_running(&task.id, started_at)?;
        let result = execute_action(&task.action)?;
        runs.push(store.finish_task(&task.id, started_at, result)?);
    }

    Ok(runs)
}

pub fn execute_action(action: &Action) -> Result<ExecutionResult> {
    match action {
        Action::Command {
            program,
            args,
            working_dir,
            timeout_seconds,
        } => execute_command(program, args, working_dir.as_deref(), *timeout_seconds),
    }
}

fn execute_command(
    program: &str,
    args: &[String],
    working_dir: Option<&Path>,
    timeout_seconds: u64,
) -> Result<ExecutionResult> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(working_dir) = working_dir {
        command.current_dir(working_dir);
    }

    let mut child = command.spawn()?;
    let started = Instant::now();
    let timeout = Duration::from_secs(timeout_seconds.max(1));

    loop {
        if child.try_wait()?.is_some() {
            let output = child.wait_with_output()?;
            return Ok(ExecutionResult {
                exit_code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                error_message: None,
            });
        }

        if started.elapsed() >= timeout {
            child.kill()?;
            let output = child.wait_with_output()?;
            return Ok(ExecutionResult {
                exit_code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                error_message: Some(format!("task timed out after {timeout_seconds} seconds")),
            });
        }

        thread::sleep(Duration::from_millis(100));
    }
}

fn row_to_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    let schedule_json: String = row.get(3)?;
    let action_json: String = row.get(4)?;
    let status: String = row.get(5)?;
    let created_at: String = row.get(6)?;
    let updated_at: String = row.get(7)?;
    let next_run_at: Option<String> = row.get(8)?;
    let last_run_at: Option<String> = row.get(9)?;

    Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        schedule: serde_json::from_str(&schedule_json).map_err(json_to_sql_error)?,
        action: serde_json::from_str(&action_json).map_err(json_to_sql_error)?,
        status: parse_status(&status).map_err(json_to_sql_error)?,
        created_at: parse_datetime_sql(&created_at)?,
        updated_at: parse_datetime_sql(&updated_at)?,
        next_run_at: parse_optional_datetime_sql(next_run_at)?,
        last_run_at: parse_optional_datetime_sql(last_run_at)?,
    })
}

fn row_to_task_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskRun> {
    let status: String = row.get(2)?;
    let started_at: String = row.get(3)?;
    let finished_at: String = row.get(4)?;

    Ok(TaskRun {
        id: row.get(0)?,
        task_id: row.get(1)?,
        status: parse_status(&status).map_err(json_to_sql_error)?,
        started_at: parse_datetime_sql(&started_at)?,
        finished_at: parse_datetime_sql(&finished_at)?,
        exit_code: row.get(5)?,
        stdout: row.get(6)?,
        stderr: row.get(7)?,
        error_message: row.get(8)?,
    })
}

fn parse_datetime_sql(value: &str) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|time| time.with_timezone(&Utc))
        .map_err(json_to_sql_error)
}

fn parse_optional_datetime_sql(value: Option<String>) -> rusqlite::Result<Option<DateTime<Utc>>> {
    value.as_deref().map(parse_datetime_sql).transpose()
}

fn status_to_str(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "pending",
        TaskStatus::Running => "running",
        TaskStatus::Succeeded => "succeeded",
        TaskStatus::Failed => "failed",
        TaskStatus::Cancelled => "cancelled",
    }
}

fn parse_status(value: &str) -> std::result::Result<TaskStatus, serde_json::Error> {
    serde_json::from_value(serde_json::Value::String(value.to_string()))
}

fn json_to_sql_error(error: impl std::error::Error + Send + Sync + 'static) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}
