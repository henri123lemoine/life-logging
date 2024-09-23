use super::handlers;
use crate::app_state::AppState;
use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use std::time::Instant;
use tracing::info;

async fn logging_middleware(req: Request, next: Next) -> Response {
    let path = req.uri().path().to_owned();
    let method = req.method().clone();
    let start = Instant::now();

    info!("Request: {} {}", method, path);

    let response = next.run(req).await;

    let duration = start.elapsed();
    let status = response.status();

    info!(
        "Response: {} {} - status: {}, duration: {:?}",
        method, path, status, duration
    );

    response
}

pub fn create_router(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(|| async { "Audio Recording Server" }))
        .route("/health", get(handlers::health_check))
        .route("/get_audio", get(handlers::get_audio))
        .route("/visualize_audio", get(handlers::visualize_audio))
        .route("/list_devices", get(handlers::list_audio_devices))
        .route("/change_device", post(handlers::change_audio_device))
        .layer(axum::middleware::from_fn(logging_middleware))
        .with_state(app_state)
}
