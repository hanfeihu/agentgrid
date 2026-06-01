use agentgrid_protocol::{AgentTaskState, JobState};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid transition from {from:?} to {to:?}")]
    InvalidTaskTransition {
        from: AgentTaskState,
        to: AgentTaskState,
    },
    #[error("invalid job transition from {from:?} to {to:?}")]
    InvalidJobTransition { from: JobState, to: JobState },
}

pub type Result<T> = std::result::Result<T, CoreError>;

pub fn validate_task_transition(from: AgentTaskState, to: AgentTaskState) -> Result<()> {
    use AgentTaskState::*;
    let allowed = matches!(
        (from, to),
        (Todo, Assigned)
            | (Assigned, InProgress)
            | (Assigned, Blocked)
            | (InProgress, Blocked)
            | (InProgress, Review)
            | (InProgress, Testing)
            | (Blocked, InProgress)
            | (Review, Done)
            | (Review, InProgress)
            | (Testing, Done)
            | (Testing, InProgress)
            | (_, Cancelled)
    );

    if allowed || from == to {
        Ok(())
    } else {
        Err(CoreError::InvalidTaskTransition { from, to })
    }
}

pub fn validate_job_transition(from: JobState, to: JobState) -> Result<()> {
    use JobState::*;
    let allowed = matches!(
        (from, to),
        (Queued, Scheduled)
            | (Queued, Blocked)
            | (Scheduled, Dispatching)
            | (Dispatching, Running)
            | (Running, Succeeded)
            | (Running, Failed)
            | (Running, Lost)
            | (Failed, Retrying)
            | (Retrying, Queued)
            | (Lost, Queued)
            | (_, Cancelled)
    );

    if allowed || from == to {
        Ok(())
    } else {
        Err(CoreError::InvalidJobTransition { from, to })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_expected_task_flow() {
        assert!(validate_task_transition(AgentTaskState::Todo, AgentTaskState::Assigned).is_ok());
        assert!(
            validate_task_transition(AgentTaskState::Assigned, AgentTaskState::InProgress).is_ok()
        );
        assert!(
            validate_task_transition(AgentTaskState::InProgress, AgentTaskState::Review).is_ok()
        );
        assert!(validate_task_transition(AgentTaskState::Review, AgentTaskState::Done).is_ok());
    }

    #[test]
    fn rejects_skipping_task_review() {
        let result = validate_task_transition(AgentTaskState::InProgress, AgentTaskState::Done);
        assert!(matches!(
            result,
            Err(CoreError::InvalidTaskTransition {
                from: AgentTaskState::InProgress,
                to: AgentTaskState::Done
            })
        ));
    }

    #[test]
    fn allows_expected_job_flow() {
        assert!(validate_job_transition(JobState::Queued, JobState::Scheduled).is_ok());
        assert!(validate_job_transition(JobState::Scheduled, JobState::Dispatching).is_ok());
        assert!(validate_job_transition(JobState::Dispatching, JobState::Running).is_ok());
        assert!(validate_job_transition(JobState::Running, JobState::Succeeded).is_ok());
    }

    #[test]
    fn rejects_running_back_to_queued() {
        let result = validate_job_transition(JobState::Running, JobState::Queued);
        assert!(matches!(
            result,
            Err(CoreError::InvalidJobTransition {
                from: JobState::Running,
                to: JobState::Queued
            })
        ));
    }
}
