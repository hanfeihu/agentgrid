use agentgrid_protocol::{Job, Node, NodeState};

#[derive(Debug, Clone)]
pub struct ScheduleDecision {
    pub node_id: Option<String>,
    pub reason: String,
    pub score: Option<f64>,
    pub candidates: Vec<NodeScore>,
}

#[derive(Debug, Clone)]
pub struct NodeScore {
    pub node_id: String,
    pub score: f64,
    pub available_slots: u16,
}

pub fn choose_node(job: &Job, nodes: &[Node]) -> ScheduleDecision {
    let mut candidates: Vec<&Node> = nodes
        .iter()
        .filter(|node| node.status == NodeState::Online)
        .filter(|node| {
            job.spec
                .requirements
                .node_id
                .as_ref()
                .map(|required| required == &node.id)
                .unwrap_or(true)
        })
        .filter(|node| {
            job.spec
                .requirements
                .os
                .iter()
                .all(|os| os_matches(&node.os, os))
        })
        .filter(|node| {
            job.spec
                .requirements
                .groups
                .iter()
                .all(|group| node.groups.contains(group))
        })
        .filter(|node| {
            job.spec
                .requirements
                .tags
                .iter()
                .all(|tag| node.tags.contains(tag))
        })
        .filter(|node| {
            job.spec
                .requirements
                .capabilities
                .iter()
                .all(|capability| node.capabilities.contains(capability))
        })
        .filter(|node| {
            job.spec
                .requirements
                .cpu_cores
                .map(|required| node.cpu_cores >= required)
                .unwrap_or(true)
        })
        .filter(|node| {
            job.spec
                .requirements
                .memory_mb
                .map(|required| node.memory_mb >= required)
                .unwrap_or(true)
        })
        .filter(|node| {
            job.spec
                .requirements
                .disk_free_mb
                .map(|required| node.disk_free_mb >= required)
                .unwrap_or(true)
        })
        .collect();

    candidates.retain(|node| !job.spec.requirements.avoid_node_ids.contains(&node.id));

    candidates.sort_by(|left, right| {
        score_node_for_job(left, job)
            .partial_cmp(&score_node_for_job(right, job))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    match candidates.first() {
        Some(node) => ScheduleDecision {
            node_id: Some(node.id.clone()),
            reason: format!(
                "selected eligible node {} with resource score {:.2}",
                node.id,
                score_node_for_job(node, job)
            ),
            score: Some(score_node_for_job(node, job)),
            candidates: candidates
                .iter()
                .map(|candidate| NodeScore {
                    node_id: candidate.id.clone(),
                    score: score_node_for_job(candidate, job),
                    available_slots: candidate
                        .max_concurrent_jobs
                        .saturating_sub(candidate.running_jobs),
                })
                .collect(),
        },
        None => ScheduleDecision {
            node_id: None,
            reason: "no eligible online node matched job requirements".to_string(),
            score: None,
            candidates: Vec::new(),
        },
    }
}

pub fn score_node(node: &Node) -> f64 {
    let cpu_score = node.cpu_usage_percent.clamp(0.0, 100.0) as f64;
    let memory_score = if node.memory_mb == 0 {
        100.0
    } else {
        (node.memory_used_mb as f64 / node.memory_mb as f64 * 100.0).clamp(0.0, 100.0)
    };
    let disk_score = if node.disk_total_mb == 0 {
        100.0
    } else {
        let used = node.disk_total_mb.saturating_sub(node.disk_free_mb);
        (used as f64 / node.disk_total_mb as f64 * 100.0).clamp(0.0, 100.0)
    };
    let slot_pressure = if node.max_concurrent_jobs == 0 {
        100.0
    } else {
        (node.running_jobs as f64 / node.max_concurrent_jobs as f64 * 100.0).clamp(0.0, 100.0)
    };
    let success_penalty = (100.0 - node.success_rate.clamp(0.0, 100.0)) * 0.2;
    let weight = node.weight.max(0.1);

    (cpu_score * 0.38
        + memory_score * 0.26
        + disk_score * 0.12
        + slot_pressure * 0.18
        + success_penalty)
        / weight
}

pub fn score_node_for_job(node: &Node, job: &Job) -> f64 {
    let mut score = score_node(node);
    if job
        .spec
        .requirements
        .preferred_node_ids
        .iter()
        .any(|preferred| preferred == &node.id)
    {
        score *= 0.8;
    }
    score
}

fn os_matches(reported: &str, required: &str) -> bool {
    let reported = reported.to_ascii_lowercase();
    let required = required.to_ascii_lowercase();
    if reported.contains(&required) {
        return true;
    }
    match required.as_str() {
        "linux" => [
            "ubuntu",
            "debian",
            "centos",
            "rhel",
            "fedora",
            "almalinux",
            "rocky",
            "arch",
        ]
        .iter()
        .any(|alias| reported.contains(alias)),
        "mac" | "macos" | "darwin" => {
            reported.contains("darwin") || reported.contains("macos") || reported.contains("mac")
        }
        "windows" | "win" => reported.contains("windows") || reported.contains("win"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentgrid_protocol::{
        JobMetadata, JobPayload, JobRequirements, JobSpec, JobState, JobStatus, Priority,
    };
    use chrono::Utc;
    use serde_json::json;

    #[test]
    fn selects_least_busy_matching_online_node() {
        let job = job_with_requirements(JobRequirements {
            tags: vec!["gpu".to_string()],
            capabilities: vec!["http".to_string()],
            cpu_cores: Some(4),
            memory_mb: Some(4096),
            ..JobRequirements::default()
        });
        let nodes = vec![
            node("busy", NodeState::Online, 8, 8192, 7000, 20, 5),
            node("best", NodeState::Online, 8, 8192, 1000, 10, 1),
            node("offline", NodeState::Offline, 16, 32768, 1000, 5, 0),
        ];

        let decision = choose_node(&job, &nodes);

        assert_eq!(decision.node_id.as_deref(), Some("best"));
    }

    #[test]
    fn reports_no_node_when_requirements_do_not_match() {
        let job = job_with_requirements(JobRequirements {
            capabilities: vec!["browser".to_string()],
            ..JobRequirements::default()
        });
        let nodes = vec![node("http-only", NodeState::Online, 8, 8192, 1000, 5, 0)];

        let decision = choose_node(&job, &nodes);

        assert_eq!(decision.node_id, None);
        assert!(decision.reason.contains("no eligible"));
    }

    #[test]
    fn treats_required_node_id_as_hard_constraint() {
        let job = job_with_requirements(JobRequirements {
            node_id: Some("target-windows".to_string()),
            ..JobRequirements::default()
        });
        let nodes = vec![
            node("jia-node", NodeState::Online, 32, 65536, 1000, 1, 0),
            node("target-windows", NodeState::Online, 8, 8192, 7000, 80, 7),
        ];

        let decision = choose_node(&job, &nodes);

        assert_eq!(decision.node_id.as_deref(), Some("target-windows"));
        assert_eq!(decision.candidates.len(), 1);
        assert_eq!(decision.candidates[0].node_id, "target-windows");
    }

    fn job_with_requirements(requirements: JobRequirements) -> Job {
        Job {
            api_version: "agentmessage/v1".to_string(),
            kind: "Job".to_string(),
            metadata: JobMetadata {
                id: "job-1".to_string(),
                project_id: "agentgrid".to_string(),
                client_id: "client-1".to_string(),
                created_at: Utc::now(),
            },
            spec: JobSpec {
                name: "test job".to_string(),
                priority: Priority::Normal,
                requirements,
                payload: JobPayload::Custom {
                    name: "noop".to_string(),
                    value: json!({}),
                },
            },
            status: JobStatus {
                state: JobState::Queued,
                assigned_node_id: None,
                started_at: None,
                finished_at: None,
                result: None,
            },
        }
    }

    fn node(
        id: &str,
        status: NodeState,
        cpu_cores: u16,
        memory_mb: u64,
        memory_used_mb: u64,
        cpu_usage_percent: u8,
        running_jobs: u16,
    ) -> Node {
        Node {
            id: id.to_string(),
            name: id.to_string(),
            os: "macos".to_string(),
            arch: "aarch64".to_string(),
            tags: vec!["gpu".to_string()],
            capabilities: vec!["http".to_string()],
            cpu_cores,
            memory_mb,
            cpu_usage_percent: cpu_usage_percent as f32,
            memory_used_mb,
            disk_total_mb: 100_000,
            disk_free_mb: 80_000,
            running_jobs,
            max_concurrent_jobs: 8,
            weight: 1.0,
            groups: vec!["default".to_string()],
            success_rate: 100.0,
            status,
            last_heartbeat_at: Utc::now(),
        }
    }
}
