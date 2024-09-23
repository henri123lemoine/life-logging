use crate::app_state::AppState;
use crate::audio::encoder::{AudioEncoder, ENCODER_FACTORY};
use crate::audio::visualizer::AudioVisualizer;
use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use cpal::traits::{DeviceTrait, HostTrait};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info};

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Server is healthy", body = serde_json::Value)
    ),
    tag = "system"
)]
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

pub async fn test(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    info!("Testing endpoint. Used for development purposes only.");

    let buffer_len = {
        let audio_buffer = state.audio_buffer.read().unwrap();
        let buffer = audio_buffer.read(None);
        buffer.len()
    }; // ^^ 5ms

    let response = {
        let encoder = ENCODER_FACTORY.get_encoder("wav").unwrap();
        encode_and_respond(state, encoder, None).await
    }; // ^^ 140ms
    info!("Test response: {:?}", response);

    // Note: Encoding is the bottleneck here

    Json(json!({
        "status": "ok",
        "message": "Successfully cloned audio buffer",
        "length": buffer_len,
    }))
}

#[utoipa::path(
    get,
    path = "/get_audio",
    params(
        ("format" = Option<String>, Query, description = "Audio format (wav, flac, etc.)"),
        ("duration" = Option<f32>, Query, description = "Duration of audio to retrieve in seconds")
    ),
    responses(
        (status = 200, description = "Successfully retrieved audio", content_type = "audio/wav"),
        (status = 400, description = "Bad request", body = String),
        (status = 500, description = "Internal server error", body = String)
    ),
    tag = "audio"
)]
pub async fn get_audio(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    let format = params
        .get("format")
        .map(|s| s.to_lowercase())
        .unwrap_or_else(|| "wav".to_string());

    let duration = params
        .get("duration")
        .and_then(|d| d.parse::<f32>().ok())
        .map(Duration::from_secs_f32);

    match ENCODER_FACTORY.get_encoder(&format) {
        Some(encoder) => encode_and_respond(state, encoder, duration).await,
        None => (
            StatusCode::BAD_REQUEST,
            [(header::CONTENT_TYPE, "application/json")],
            Json(json!({"error": "Unsupported audio format"})),
        )
            .into_response(),
    }
}

async fn encode_and_respond(
    state: Arc<AppState>,
    encoder: &dyn AudioEncoder,
    duration: Option<Duration>,
) -> Response {
    let audio_buffer = state.audio_buffer.read().unwrap();
    let data = audio_buffer.read(duration);
    let sample_rate = audio_buffer.get_sample_rate();
    match encoder.encode(&data, sample_rate) {
        Ok(encoded_data) => {
            info!("Successfully encoded {} bytes of audio", encoded_data.len());
            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, encoder.mime_type()),
                    (header::CONTENT_DISPOSITION, encoder.content_disposition()),
                ],
                encoded_data,
            )
                .into_response()
        }
        Err(e) => {
            error!("Failed to encode audio: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "application/json")],
                Json(json!({"error": format!("Failed to encode audio: {}", e)})),
            )
                .into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/visualize_audio",
    responses(
        (status = 200, description = "Successfully generated audio visualization", content_type = "image/png"),
        (status = 500, description = "Internal server error", body = String)
    ),
    tag = "audio"
)]
pub async fn visualize_audio(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let width = 800;
    let height = 400;
    let duration: Option<Duration> = Some(Duration::from_secs(30));
    let audio_buffer = state.audio_buffer.read().unwrap();
    let image_data = AudioVisualizer::create_waveform(&audio_buffer.read(duration), width, height);

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CONTENT_DISPOSITION, "inline"),
        ],
        image_data,
    )
}

#[utoipa::path(
    get,
    path = "/list_devices",
    responses(
        (status = 200, description = "Successfully retrieved audio devices", body = serde_json::Value),
        (status = 500, description = "Internal server error", body = String)
    ),
    tag = "audio"
)]
pub async fn list_audio_devices() -> Json<serde_json::Value> {
    let host = cpal::default_host();

    match host.input_devices() {
        Ok(input_devices) => {
            let devices: Vec<serde_json::Value> = input_devices
                .filter_map(|device| {
                    device.name().ok().map(|name| {
                        json!({
                            "name": name,
                            "id": name, // Using name as ID for simplicity
                        })
                    })
                })
                .collect();

            Json(json!({
                "devices": devices
            }))
        }
        Err(e) => {
            error!("Failed to get input devices: {}", e);
            Json(json!({
                "error": "Failed to get input devices",
                "devices": Vec::<serde_json::Value>::new()
            }))
        }
    }
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct ChangeDeviceRequest {
    _device_id: String,
}

#[utoipa::path(
    post,
    path = "/change_device",
    request_body = ChangeDeviceRequest,
    responses(
        (status = 200, description = "Device changed successfully", body = serde_json::Value),
        (status = 400, description = "Bad request", body = String),
        (status = 500, description = "Internal server error", body = String)
    ),
    tag = "audio"
)]
pub async fn change_audio_device(
    State(_state): State<Arc<AppState>>,
    Json(_payload): Json<ChangeDeviceRequest>,
) -> Json<serde_json::Value> {
    // TODO: Implement changing audio device
    Json(json!({
        "status": "error",
        "message": "This endpoint is not yet implemented",
        "code": "NOT_IMPLEMENTED"
    }))
}
