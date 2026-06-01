use agentgrid_core::{validate_task_transition, CoreError};
use agentgrid_protocol::{AgentTask, AgentTaskState};
use agentgrid_store::{AgentGridStore, StoreError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ControlError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error("task not found: {0}")]
    TaskNotFound(String),
}

pub type Result<T> = std::result::Result<T, ControlError>;

pub struct ControlPlane {
    store: AgentGridStore,
}

impl ControlPlane {
    pub fn new(store: AgentGridStore) -> Self {
        Self { store }
    }

    pub fn submit_task(&self, task: &AgentTask) -> Result<()> {
        validate_task_transition(AgentTaskState::Todo, task.status.state)?;
        self.store.create_task(task)?;
        Ok(())
    }

    pub fn list_tasks(&self, project_id: &str) -> Result<Vec<AgentTask>> {
        Ok(self.store.list_tasks(project_id)?)
    }
}
