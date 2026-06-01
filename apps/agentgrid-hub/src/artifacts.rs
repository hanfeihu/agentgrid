use axum::{routing::get, Router};

use crate::{download_artifact, list_artifacts, AppState};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/artifacts", get(list_artifacts))
        .route("/api/artifacts/{id}/download", get(download_artifact))
}
