use axum::{
    extract::State,
    response::IntoResponse,
    http::{header, StatusCode},
    Json,
};
use serde_json;
use std::sync::Arc;
use crate::app_state::AppState;

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
            tracing::debug!("Retrieved {} audio samples for encoding", audio_data.len());
            match encoder.encode(&audio_data, state.config.sample_rate) {
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
