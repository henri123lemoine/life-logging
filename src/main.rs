mod audio_buffer;
mod config;
use config::{Config, load_config};

use axum::{
    routing::get,
    Router, extract::State,
    response::IntoResponse,
    http::{header, StatusCode},
    Json,
};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use cpal::traits::{DeviceTrait, StreamTrait};
use std::time::Duration;
use audio_buffer::{CircularAudioBuffer, WavEncoder};
use tracing::{info, warn, error, debug};
use tokio::sync::broadcast;

struct AppState {
    audio_buffer: Arc<CircularAudioBuffer>,
    audio_sender: broadcast::Sender<Vec<f32>>,
    start_time: std::time::SystemTime,
    config: Arc<Config>,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    info!("Starting Life-Logging audio recording service");

    let config = load_config()?;
    let app_state = setup_app_state(&config)?;
    setup_audio_processing(&app_state);
    
    let (device, stream_config) = config.get_audio_config()?;
    start_audio_stream(device, stream_config, app_state.audio_sender.clone())?;

    run_server(&app_state).await?;
    Ok(())
}

fn setup_app_state(config: &Arc<Config>) -> Result<Arc<AppState>, Box<dyn std::error::Error>> {
    let buffer_size = config.sample_rate as usize * config.buffer_duration as usize;
    let audio_buffer = Arc::new(CircularAudioBuffer::new(buffer_size, config.sample_rate));
    let (audio_sender, _) = broadcast::channel(1024);

    let app_state = Arc::new(AppState {
        audio_buffer: audio_buffer.clone(),
        audio_sender,
        start_time: std::time::SystemTime::now(),
        config: config.clone(),
    });

    // Initialize buffer with silence
    let silence = vec![0.0; buffer_size];
    app_state.audio_buffer.write(&silence);

    Ok(app_state)
}

fn setup_audio_processing(app_state: &Arc<AppState>) {
    let audio_buffer = app_state.audio_buffer.clone();
    let mut audio_receiver = app_state.audio_sender.subscribe();

    tokio::spawn(async move {
        audio_processing_task(audio_buffer, &mut audio_receiver).await;
    });
}

async fn audio_processing_task(
    audio_buffer: Arc<CircularAudioBuffer>,
    audio_receiver: &mut broadcast::Receiver<Vec<f32>>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        tokio::select! {
            _ = interval.tick() => {
                // Process audio data or do whatever every 5 seconds (general template)
                // This could involve writing to a file or other persistent storage
            }
            result = audio_receiver.recv() => {
                if let Ok(data) = result {
                    audio_buffer.write(&data);
                }
            }
        }
    }
}

fn start_audio_stream(device: cpal::Device, config: cpal::StreamConfig, audio_sender: broadcast::Sender<Vec<f32>>) -> Result<(), Box<dyn std::error::Error>> {
    let stream = device.build_input_stream(
        &config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            if !data.iter().any(|&sample| sample != 0.0) {
                debug!("Detected no audio input");
            }
            if let Err(e) = audio_sender.send(data.to_vec()) {
                warn!("Failed to send audio data: {}", e);
            }
        },
        |err| error!("An error occurred on stream: {}", err),
        Some(Duration::from_secs(2))
    )?;

    stream.play()?;
    info!("Audio stream started with sample rate: {}", config.sample_rate.0);

    // Keep the stream alive
    std::mem::forget(stream);

    Ok(())
}

async fn run_server(app_state: &Arc<AppState>) -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new()
        .route("/", get(|| async { "Audio Recording Server" }))
        .route("/health", get(health_check))
        .route("/get_audio", get(get_audio))
        .route("/visualize_audio", get(visualize_audio))
        .with_state(app_state.clone());

    let addr = SocketAddr::new(
        app_state.config.server.host.parse()?,
        app_state.config.server.port
    );
    info!("Listening on {}", addr);
    axum_server::bind(addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

async fn health_check(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let uptime = state.start_time.elapsed().unwrap_or_default();
    let response = json!({
        "status": "ok",
        "uptime": format!("{}s", uptime.as_secs()),
        "message": "Audio Recording Server is running"
    });
    debug!("Health check response: {:?}", response);
    Json(response)
}

async fn get_audio(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let wav_encoder = WavEncoder;
    let wav_data = state.audio_buffer.encode(wav_encoder);

    info!("Encoded {} bytes of WAV audio data", wav_data.len());

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "audio/wav"),
            (header::CONTENT_DISPOSITION, "attachment; filename=\"audio.wav\""),
        ],
        wav_data,
    )
}

async fn visualize_audio(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let width = 800;
    let height = 400;
    let image_data = state.audio_buffer.visualize(width, height);

    info!("Generated audio visualization image");

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CONTENT_DISPOSITION, "inline"),
        ],
        image_data,
    )
}
