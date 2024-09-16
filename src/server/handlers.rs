use axum::{
    extract::State,
    response::IntoResponse,
    http::{header, StatusCode},
    Json,
};
use serde_json;
use std::sync::Arc;
use tracing::{info_span, info, error, Instrument};
use crate::app_state::AppState;
use crate::config::CONFIG_MANAGER;

pub async fn health_check(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let uptime = state.start_time.elapsed().unwrap_or_default();
    let response = serde_json::json!({
        "status": "ok",
        "uptime": format!("{}s", uptime.as_secs()),
        "message": "Audio Recording Server is running"
    });
    tracing::debug!("Health check response: {:?}", response);
    Json(response)
}

pub async fn get_audio(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>
) -> impl IntoResponse {
    let format = params.get("format").map(|s| s.to_lowercase()).unwrap_or_else(|| "wav".to_string());
    
    match state.encoder_factory.get_encoder(&format) {
        Some(encoder) => {
            let audio_data = state.audio_buffer.read();
            let sample_rate = state.audio_buffer.sample_rate();
            match encoder.encode(&audio_data, sample_rate) {
                Ok(encoded_data) => {
                    tracing::info!("Successfully encoded {} bytes of {} audio", encoded_data.len(), format);
                    (
                        StatusCode::OK,
                        [
                            (header::CONTENT_TYPE, encoder.mime_type()),
                            (header::CONTENT_DISPOSITION, encoder.content_disposition()),
                        ],
                        encoded_data,
                    ).into_response()
                },
                Err(e) => {
                    tracing::error!("Failed to encode audio: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        [(header::CONTENT_TYPE, "application/json")],
                        Json(serde_json::json!({"error": format!("Failed to encode {}: {}", format.to_uppercase(), e)})).to_string(),
                    ).into_response()
                },
            }
        },
        None => (
            StatusCode::BAD_REQUEST,
            [(header::CONTENT_TYPE, "application/json")],
            Json(serde_json::json!({"error": "Unsupported audio format"})).to_string(),
        ).into_response(),
    }
}

pub async fn visualize_audio(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let width = 800;
    let height = 400;
    let image_data = state.audio_buffer.visualize(width, height);

    tracing::info!("Generated audio visualization image");

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CONTENT_DISPOSITION, "inline"),
        ],
        image_data,
    )
}

pub async fn reload_config() -> impl IntoResponse {
    let reload_span = info_span!("config_reload");
    
    async {
        info!("Configuration reload initiated");
        
        match CONFIG_MANAGER.reload().await {
            Ok(_) => {
                info!("Configuration reloaded successfully");
                let response = serde_json::json!({
                    "status": "ok",
                    "message": "Configuration reloaded successfully"
                });
                (StatusCode::OK, Json(response))
            },
            Err(e) => {
                error!(error = %e, "Failed to reload configuration");
                let response = serde_json::json!({
                    "status": "error",
                    "message": format!("Failed to reload configuration: {}", e)
                });
                (StatusCode::INTERNAL_SERVER_ERROR, Json(response))
            }
        }
    }.instrument(reload_span).await
}
