use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub api_version: String,
    pub kind: String,
    pub metadata: AgentMetadata,
    pub spec: AgentSpec,
    pub status: AgentStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetadata {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSpec {
    pub role: String,
    pub skills: Vec<String>,
    pub permissions: Vec<String>,
    pub responsibility: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatus {
    pub state: AgentState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentState {
    Online,
    Offline,
    Busy,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub api_version: String,
    pub kind: String,
    pub metadata: AgentMessageMetadata,
    pub spec: AgentMessageSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessageMetadata {
    pub id: String,
    pub project_id: String,
    pub from: String,
    pub to: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessageSpec {
    #[serde(rename = "type")]
    pub message_type: AgentMessageType,
    pub subject: String,
    pub summary: String,
    pub priority: Priority,
    pub requires_ack: bool,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMessageType {
    #[serde(rename = "broadcast.notice")]
    BroadcastNotice,
    #[serde(rename = "task.assigned")]
    TaskAssigned,
    #[serde(rename = "task.started")]
    TaskStarted,
    #[serde(rename = "task.progress")]
    TaskProgress,
    #[serde(rename = "task.blocked")]
    TaskBlocked,
    #[serde(rename = "task.completed")]
    TaskCompleted,
    #[serde(rename = "contract.changed")]
    ContractChanged,
    #[serde(rename = "review.requested")]
    ReviewRequested,
    #[serde(rename = "review.comment")]
    ReviewComment,
    #[serde(rename = "review.approved")]
    ReviewApproved,
    #[serde(rename = "test.failed")]
    TestFailed,
    #[serde(rename = "test.passed")]
    TestPassed,
    #[serde(rename = "decision.proposed")]
    DecisionProposed,
    #[serde(rename = "decision.accepted")]
    DecisionAccepted,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    P0,
    P1,
    P2,
    Low,
    Normal,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    pub api_version: String,
    pub kind: String,
    pub metadata: AgentTaskMetadata,
    pub spec: AgentTaskSpec,
    pub status: AgentTaskStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskMetadata {
    pub id: String,
    pub project_id: String,
    pub created_by: String,
    pub assigned_to: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub correlation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskSpec {
    pub title: String,
    pub summary: String,
    pub owner: Option<String>,
    pub priority: Priority,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub acceptance_criteria: Vec<String>,
    pub labels: Vec<String>,
    pub depends_on: Vec<String>,
    pub due_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskStatus {
    pub state: AgentTaskState,
    pub progress: u8,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub blocked_reason: Option<String>,
    pub last_message_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTaskState {
    Todo,
    Assigned,
    InProgress,
    Blocked,
    Review,
    Testing,
    Done,
    Cancelled,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;

    #[test]
    fn serializes_agent_message_type_with_dotted_name() {
        let message = AgentMessage {
            api_version: "agentmessage/v1".to_string(),
            kind: "AgentMessage".to_string(),
            metadata: AgentMessageMetadata {
                id: "msg-1".to_string(),
                project_id: "agentgrid".to_string(),
                from: "pm-agent".to_string(),
                to: vec!["backend-agent".to_string()],
                created_at: Utc::now(),
            },
            spec: AgentMessageSpec {
                message_type: AgentMessageType::TaskAssigned,
                subject: "实现任务接口".to_string(),
                summary: "补齐任务创建和状态更新接口".to_string(),
                priority: Priority::P1,
                requires_ack: true,
                payload: json!({ "task_id": "task-1" }),
            },
        };

        let value = serde_json::to_value(message).expect("message should serialize");

        assert_eq!(value["spec"]["type"], "task.assigned");
        assert_eq!(value["spec"]["priority"], "p1");
    }

    #[test]
    fn deserializes_unknown_message_type() {
        let value = json!({
            "api_version": "agentmessage/v1",
            "kind": "AgentMessage",
            "metadata": {
                "id": "msg-1",
                "project_id": "agentgrid",
                "from": "agent-a",
                "to": ["agent-b"],
                "created_at": Utc::now()
            },
            "spec": {
                "type": "future.event",
                "subject": "future",
                "summary": "unknown future event",
                "priority": "normal",
                "requires_ack": false,
                "payload": {}
            }
        });

        let message: AgentMessage =
            serde_json::from_value(value).expect("unknown type should be accepted");

        assert_eq!(message.spec.message_type, AgentMessageType::Unknown);
    }
}
