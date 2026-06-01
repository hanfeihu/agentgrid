use axum::{
    routing::{get, post},
    Router,
};

use crate::{
    agent_runtime_get_task, agent_runtime_manifest, agent_runtime_submit_task,
    complete_worker_task, control_task, create_task, fail_worker_task, get_task, get_task_template,
    lease_tasks, list_task_templates, list_tasks, renew_worker_task, start_task_template,
    task_events, task_schedule_preview, task_snapshot, update_task, worker_reconcile,
    worker_task_control, worker_task_log, AppState,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/agent-runtime/manifest", get(agent_runtime_manifest))
        .route("/api/agent-runtime/tasks", post(agent_runtime_submit_task))
        .route("/api/agent-runtime/tasks/{id}", get(agent_runtime_get_task))
        .route("/api/agent-runtime/tasks/{id}/events", get(task_events))
        .route("/api/task-templates", get(list_task_templates))
        .route("/api/task-templates/{id}", get(get_task_template))
        .route("/api/task-templates/{id}/start", post(start_task_template))
        .route("/api/tasks", get(list_tasks).post(create_task))
        .route("/api/tasks/{id}", get(get_task))
        .route("/api/tasks/{id}/snapshot", get(task_snapshot))
        .route(
            "/api/tasks/{id}/schedule-preview",
            get(task_schedule_preview),
        )
        .route("/api/tasks/{id}/events", get(task_events))
        .route("/api/tasks/{id}/control", post(control_task))
        .route("/api/tasks/{id}/{action}", post(update_task))
        .route("/api/worker/lease", post(lease_tasks))
        .route("/api/worker/reconcile", post(worker_reconcile))
        .route("/api/worker/tasks/{id}/control", get(worker_task_control))
        .route("/api/worker/tasks/{id}/renew", post(renew_worker_task))
        .route("/api/worker/tasks/{id}/logs", post(worker_task_log))
        .route(
            "/api/worker/tasks/{id}/complete",
            post(complete_worker_task),
        )
        .route("/api/worker/tasks/{id}/fail", post(fail_worker_task))
}
