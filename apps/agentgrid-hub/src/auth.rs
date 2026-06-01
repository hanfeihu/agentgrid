use axum::{
    routing::{get, post},
    Router,
};

use crate::{
    auth_me, change_password, create_super_admin, get_bootstrap_status, login_user, register_user,
    request_register_code, AppState,
};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/bootstrap", get(get_bootstrap_status))
        .route("/api/bootstrap/admin", post(create_super_admin))
        .route("/api/auth/me", get(auth_me))
        .route("/api/auth/login", post(login_user))
        .route(
            "/api/auth/register/request-code",
            post(request_register_code),
        )
        .route("/api/auth/register", post(register_user))
        .route("/api/auth/change-password", post(change_password))
}
