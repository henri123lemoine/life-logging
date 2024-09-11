use axum::{
    routing::get,
    Router,
};
use std::sync::Arc;
use crate::app_state::AppState;
use super::handlers::{health_check, get_audio, visualize_audio};

pub fn create_router(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(|| async { "Audio Recording Server" }))
        .route("/health", get(health_check))
        .route("/get_audio", get(get_audio))
        .route("/visualize_audio", get(visualize_audio))
        .with_state(app_state)
}
