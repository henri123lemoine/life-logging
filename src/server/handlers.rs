use axum::{
    extract::State,
    response::IntoResponse,
    http::{header, StatusCode},
    Json,
};
use serde_json;
use std::sync::Arc;
use crate::app_state::AppState;
use crate::audio::encoder::{WavEncoder, FlacEncoder};

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
    
    match format.as_str() {
        "pcm" => {
            let pcm_data = state.audio_buffer.read();
            // Convert f32 samples to bytes
            let byte_data: Vec<u8> = pcm_data.iter()
                .flat_map(|&sample| sample.to_le_bytes().to_vec())
                .collect();
            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, "audio/pcm"),
                    (header::CONTENT_DISPOSITION, "attachment; filename=\"audio.pcm\""),
                ],
                byte_data,
            ).into_response()
        },
        "wav" => {
            let wav_encoder = WavEncoder;
            match state.audio_buffer.encode(wav_encoder) {
                Ok(wav_data) => (
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, "audio/wav"),
                        (header::CONTENT_DISPOSITION, "attachment; filename=\"audio.wav\""),
                    ],
                    wav_data,
                ).into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(header::CONTENT_TYPE, "application/json")],
                    Json(serde_json::json!({"error": format!("Failed to encode WAV: {}", e)})).to_string(),
                ).into_response(),
            }
        },
        "flac" => {
            let flac_encoder = FlacEncoder;
            match state.audio_buffer.encode(flac_encoder) {
                Ok(flac_data) => (
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, "audio/flac"),
                        (header::CONTENT_DISPOSITION, "attachment; filename=\"audio.flac\""),
                    ],
                    flac_data,
                ).into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(header::CONTENT_TYPE, "application/json")],
                    Json(serde_json::json!({"error": format!("Failed to encode FLAC: {}", e)})).to_string(),
                ).into_response(),
            }
        },
        _ => (
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
