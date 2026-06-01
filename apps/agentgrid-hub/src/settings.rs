use axum::{routing::get, Router};

use crate::{get_system_settings, update_system_settings, AppState};

pub(crate) fn router() -> Router<AppState> {
    Router::new().route(
        "/api/settings",
        get(get_system_settings).post(update_system_settings),
    )
}
