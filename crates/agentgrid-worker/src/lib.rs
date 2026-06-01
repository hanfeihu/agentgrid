use agentgrid_executor::{execute, ExecutorError};
use agentgrid_protocol::{JobPayload, JobResult};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkerError {
    #[error(transparent)]
    Executor(#[from] ExecutorError),
}

pub type Result<T> = std::result::Result<T, WorkerError>;

pub struct WorkerRuntime;

impl WorkerRuntime {
    pub fn run_payload(payload: &JobPayload) -> Result<JobResult> {
        Ok(execute(payload)?)
    }
}
