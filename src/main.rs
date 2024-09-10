mod audio_buffer;
mod config;

use axum::{
    routing::get,
    Router, extract::State,
    response::IntoResponse,
    http::{header, StatusCode},
    Json,
};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SupportedStreamConfig, SampleRate};
use std::time::{Duration, Instant};
use audio_buffer::{CircularAudioBuffer, WavEncoder};
use config::Settings;
use tracing::{info, warn, error, debug};
use tokio::sync::broadcast;

struct AppState {
    audio_buffer: Arc<CircularAudioBuffer>,
    audio_sender: broadcast::Sender<Vec<f32>>,
    start_time: std::time::SystemTime,
    last_log_time: Arc<Mutex<Instant>>,
    settings: Arc<Settings>,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_tracing();
    let settings = load_settings()?;
    let app_state = setup_app_state(&settings)?;
    setup_audio_processing(&app_state);
    setup_audio_capture(&app_state)?;
    run_server(&app_state).await?;
    Ok(())
}

fn setup_tracing() {
    tracing_subscriber::fmt::init();
    info!("Starting Life-Logging audio recording service");
}

fn load_settings() -> Result<Arc<Settings>, Box<dyn std::error::Error>> {
    let settings = Arc::new(Settings::new()?);
    debug!("Loaded configuration: {:?}", settings);
    Ok(settings)
}

fn setup_app_state(settings: &Arc<Settings>) -> Result<Arc<AppState>, Box<dyn std::error::Error>> {
    let buffer_size = settings.sample_rate as usize * settings.buffer_duration as usize;
    let audio_buffer = Arc::new(CircularAudioBuffer::new(buffer_size, settings.sample_rate));
    let (audio_sender, _) = broadcast::channel(1024);

    let app_state = Arc::new(AppState {
        audio_buffer: audio_buffer.clone(),
        audio_sender,
        start_time: std::time::SystemTime::now(),
        last_log_time: Arc::new(Mutex::new(Instant::now())),
        settings: settings.clone(),
    });

    // Initialize buffer with silence
    let silence = vec![0.0; buffer_size];
    app_state.audio_buffer.write(&silence);

    Ok(app_state)
}

fn setup_audio_processing(app_state: &Arc<AppState>) {
    let audio_buffer = app_state.audio_buffer.clone();
    let mut audio_receiver = app_state.audio_sender.subscribe();
    let last_log_time = app_state.last_log_time.clone();

    tokio::spawn(async move {
        audio_processing_task(audio_buffer, &mut audio_receiver, last_log_time).await;
    });
}

fn setup_audio_capture(app_state: &Arc<AppState>) -> Result<(), Box<dyn std::error::Error>> {
    let audio_sender = app_state.audio_sender.clone();
    let sample_rate = app_state.settings.sample_rate;

    let host = cpal::default_host();
    let device = host.default_input_device().ok_or("No input device available")?;
    let config = find_supported_config(&device, sample_rate)?;

    start_audio_stream(device, config, audio_sender)?;

    Ok(())
}

fn find_supported_config(device: &cpal::Device, desired_sample_rate: u32) -> Result<cpal::StreamConfig, Box<dyn std::error::Error>> {
    let mut supported_configs_range = device.supported_input_configs()?;
    let supported_config = supported_configs_range
        .find(|range| range.min_sample_rate().0 <= desired_sample_rate && desired_sample_rate <= range.max_sample_rate().0)
        .ok_or("No supported config found")?
        .with_sample_rate(cpal::SampleRate(desired_sample_rate));

    Ok(supported_config.into())
}

async fn audio_processing_task(
    audio_buffer: Arc<CircularAudioBuffer>,
    audio_receiver: &mut broadcast::Receiver<Vec<f32>>,
    last_log_time: Arc<Mutex<Instant>>
) {
    while let Ok(data) = audio_receiver.recv().await {
        audio_buffer.write(&data);
        
        // Rate-limited logging
        let mut last_time = last_log_time.lock().unwrap();
        if last_time.elapsed() >= Duration::from_secs(10) {
            info!("Audio buffer status: wrote {} samples", data.len());
            *last_time = Instant::now();
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
        app_state.settings.server.host.parse()?,
        app_state.settings.server.port
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
