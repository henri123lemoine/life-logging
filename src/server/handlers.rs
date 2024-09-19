use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use axum::{
    extract::{State, Query},
    response::{IntoResponse, Response},
    http::{header, StatusCode},
    Json,
};
use cpal::traits::{HostTrait, DeviceTrait};
use serde::Deserialize;
use serde_json::json;
use tracing::{info, error};
use crate::app_state::AppState;
use crate::audio::encoder::{AudioEncoder, ENCODER_FACTORY};
use crate::error::LifeLoggingError;

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
    Query(params): Query<HashMap<String, String>>
) -> Response {
    let format = params.get("format")
        .map(|s| s.to_lowercase())
        .unwrap_or_else(|| "wav".to_string());

    let duration = params.get("duration")
        .and_then(|d| d.parse::<f32>().ok())
        .map(|secs| Duration::from_secs_f32(secs));

    match ENCODER_FACTORY.get_encoder(&format) {
        Some(encoder) => encode_and_respond(state, encoder.as_ref(), duration).await,
        None => unsupported_format_response(),
    }
}

async fn encode_and_respond(state: Arc<AppState>, encoder: &dyn AudioEncoder, duration: Option<Duration>) -> Response {
    let audio_buffer = state.audio_buffer.read().unwrap();
    match audio_buffer.encode(encoder, duration) {
        Ok(encoded_data) => successful_encoding_response(encoder, encoded_data),
        Err(e) => encoding_error_response(e),
    }
}

fn successful_encoding_response(encoder: &dyn AudioEncoder, encoded_data: Vec<u8>) -> Response {
    info!("Successfully encoded {} bytes of audio", encoded_data.len());
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, encoder.mime_type()),
            (header::CONTENT_DISPOSITION, encoder.content_disposition()),
        ],
        encoded_data,
    ).into_response()
}

fn unsupported_format_response() -> Response {
    (
        StatusCode::BAD_REQUEST,
        [(header::CONTENT_TYPE, "application/json")],
        Json(json!({"error": "Unsupported audio format"})),
    ).into_response()
}

fn encoding_error_response(e: LifeLoggingError) -> Response {
    error!("Failed to encode audio: {}", e);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        [(header::CONTENT_TYPE, "application/json")],
        Json(json!({"error": format!("Failed to encode audio: {}", e)})),
    ).into_response()
}

pub async fn visualize_audio(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let width = 800;
    let height = 400;
    let audio_buffer = state.audio_buffer.read().unwrap();
    let image_data = audio_buffer.visualize(width, height);

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
        },
        Err(e) => {
            error!("Failed to get input devices: {}", e);
            Json(json!({
                "error": "Failed to get input devices",
                "devices": Vec::<serde_json::Value>::new()
            }))
        }
    }
}

#[derive(Deserialize)]
pub struct ChangeDeviceRequest {
    device_id: String,
}

pub async fn change_audio_device(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ChangeDeviceRequest>,
) -> Json<serde_json::Value> {
    match state.change_audio_device(payload.device_id).await {
        Ok(()) => Json(json!({
            "status": "success",
            "message": "Audio device changed successfully"
        })),
        Err(e) => Json(json!({
            "status": "error",
            "message": format!("Failed to change audio device: {}", e)
        })),
    }
}
