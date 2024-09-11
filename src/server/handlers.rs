use axum::{
    extract::State,
    response::IntoResponse,
    http::{header, StatusCode},
    Json,
};
use serde_json::json;
use std::sync::Arc;
use crate::app_state::AppState;
use crate::audio::encoder::{WavEncoder, FlacEncoder};

pub async fn health_check(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let uptime = state.start_time.elapsed().unwrap_or_default();
    let response = json!({
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
    let format = params.get("format").map(|s| s.to_lowercase()).unwrap_or_else(|| "pcm".to_string());
    
    let (data, content_type, content_disposition) = match format.as_str() {
        "pcm" => {
            let pcm_data = state.audio_buffer.read();
            // Convert f32 samples to bytes
            let byte_data: Vec<u8> = pcm_data.iter()
                .flat_map(|&sample| sample.to_le_bytes().to_vec())
                .collect();
            (byte_data, "audio/pcm", "attachment; filename=\"audio.pcm\"")
        },
        "wav" => {
            let wav_encoder = WavEncoder;
            let wav_data = state.audio_buffer.encode(wav_encoder);
            (wav_data, "audio/wav", "attachment; filename=\"audio.wav\"")
        },
        "flac" => {
            let flac_encoder = FlacEncoder;
            let flac_data = state.audio_buffer.encode(flac_encoder);
            (flac_data, "audio/flac", "attachment; filename=\"audio.flac\"")
        },
        _ => return (
            StatusCode::BAD_REQUEST,
            [(header::CONTENT_TYPE, "application/json")],
            Json(json!({"error": "Unsupported audio format"})).to_string(),
        ).into_response(),
    };

    tracing::info!("Encoded {} bytes of {} audio data", data.len(), format);

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type),
            (header::CONTENT_DISPOSITION, content_disposition),
        ],
        data,
    ).into_response()
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
