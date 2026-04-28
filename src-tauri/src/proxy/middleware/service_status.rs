use axum::{
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
    http::StatusCode,
};
use crate::proxy::server::AppState;

pub async fn service_status_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();
    
    // Always allow Admin API, Auth callback, health checks, and internal endpoints.
    // /internal/* is loopback-only (e.g. warmup self-call from quota.rs); the auth
    // middleware already exempts it, so the service-status gate must too — otherwise
    // the warmup scheduler 503's itself before is_running ever flips on a fresh start.
    if path.starts_with("/api/")
        || path.starts_with("/internal/")
        || path == "/auth/callback"
        || path == "/health"
    {
        return next.run(request).await;
    }

    let running = {
        let r = state.is_running.read().await;
        *r
    };

    if !running {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "Proxy service is currently disabled".to_string(),
        )
            .into_response();
    }

    next.run(request).await
}
