use super::handlers;
use crate::app_state::AppState;
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

pub fn create_router(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(|| async { "Audio Recording Server" }))
        .route("/health", get(handlers::health_check))
        .route("/get_audio", get(handlers::get_audio))
        .route("/visualize_audio", get(handlers::visualize_audio))
        .route("/list_devices", get(handlers::list_audio_devices))
        .route("/change_device", post(handlers::change_audio_device))
        .with_state(app_state)
}
