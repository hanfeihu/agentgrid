use axum::{
    routing::{get, post},
    Router,
};

use crate::{
    create_ingress_event, create_job, create_job_checkpoint, get_job, job_events, job_execution,
    job_recovery_scan, job_reliability, list_jobs, plan_job, AppState,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/jobs", get(list_jobs).post(create_job))
        .route("/api/jobs/plan", post(plan_job))
        .route("/api/jobs/recovery/scan", post(job_recovery_scan))
        .route("/api/jobs/reliability", get(job_reliability))
        .route("/api/jobs/{id}", get(get_job))
        .route("/api/jobs/{id}/checkpoints", post(create_job_checkpoint))
        .route("/api/jobs/{id}/execution", get(job_execution))
        .route("/api/jobs/{id}/events", get(job_events))
        .route("/api/events/ingress", post(create_ingress_event))
}
