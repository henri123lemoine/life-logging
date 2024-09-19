use std::sync::Arc;
use axum::{
    routing::get,
    Router,
};
use super::handlers;
use crate::app_state::AppState;

pub fn create_router(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(|| async { "Audio Recording Server" }))
        .route("/health", get(handlers::health_check))
        .route("/get_audio", get(handlers::get_audio))
        .route("/visualize_audio", get(handlers::visualize_audio))
        .route("/list_devices", get(handlers::list_audio_devices))
        .with_state(app_state)
}
