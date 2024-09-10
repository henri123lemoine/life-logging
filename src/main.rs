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

struct AppState {
    audio_buffer: Arc<CircularAudioBuffer>,
    audio_sender: mpsc::Sender<Vec<f32>>,
    start_time: std::time::SystemTime,
    last_log_time: Arc<Mutex<Instant>>,
    settings: Arc<Settings>,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up tracing
    tracing_subscriber::fmt::init();

    info!("Starting Life-Logging audio recording service");

    let settings = Arc::new(Settings::new()?);
    debug!("Loaded configuration: {:?}", settings);

    let buffer_size = settings.sample_rate as usize * settings.buffer_duration as usize;

    let audio_buffer = Arc::new(CircularAudioBuffer::new(buffer_size, settings.sample_rate));
    let (audio_sender, audio_receiver) = mpsc::channel(1024);

    let app_state = Arc::new(AppState {
        audio_buffer: audio_buffer.clone(),
        audio_sender,
        start_time: std::time::SystemTime::now(),
        last_log_time: Arc::new(Mutex::new(Instant::now())),
        settings: settings.clone(),
    });

    // Initialize buffer with silence
    {
        let silence = vec![0.0; buffer_size];
        app_state.audio_buffer.write(&silence);
    }

    // Start audio processing task
    tokio::spawn(audio_processing_task(
        audio_buffer.clone(),
        audio_receiver,
        app_state.last_log_time.clone()
    ));

    // Set up audio capture
    match setup_audio_capture(app_state.audio_sender.clone(), settings.sample_rate) {
        Ok(_) => info!("Audio capture set up successfully"),
        Err(e) => {
            error!("Failed to set up audio capture: {}", e);
            // TODO: handle this error more gracefully, e.g. by retrying with a different sample rate or exiting the program
        }
    }

    let app = Router::new()
        .route("/", get(|| async { "Audio Recording Server" }))
        .route("/health", get(health_check))
        .route("/get_audio", get(get_audio))
        .route("/visualize_audio", get(visualize_audio))
        .with_state(app_state);

        info!("Listening on {}", settings.server.host);

    let addr = SocketAddr::new(
        settings.server.host.parse()?,
        settings.server.port
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

fn setup_audio_capture(audio_sender: mpsc::Sender<Vec<f32>>, desired_sample_rate: u32) -> Result<(), Box<dyn std::error::Error>> {
    let host = cpal::default_host();
    let device = host.default_input_device().ok_or("No input device available")?;

    info!("Available input devices:");
    for (idx, device) in host.input_devices()?.enumerate() {
        info!("  {}. {}", idx + 1, device.name()?);
    }

    let mut supported_configs_range = device.supported_input_configs()?;

    info!("Supported configurations for the default device:");
    let mut best_config: Option<SupportedStreamConfig> = None;
    let mut closest_rate_diff = u32::MAX;

    while let Some(supported_config) = supported_configs_range.next() {
        info!("  {:?}", supported_config);
        
        let range = supported_config.min_sample_rate().0..=supported_config.max_sample_rate().0;
        if range.contains(&desired_sample_rate) {
            best_config = Some(supported_config.with_sample_rate(SampleRate(desired_sample_rate)));
            break;
        } else {
            let start = *range.start();
            let end = *range.end();
            let diff = if desired_sample_rate < start {
                start - desired_sample_rate
            } else {
                desired_sample_rate - end
            };
            if diff < closest_rate_diff {
                closest_rate_diff = diff;
                best_config = Some(supported_config.with_sample_rate(SampleRate(if desired_sample_rate < start { start } else { end })));
            }
        }
    }

    let supported_config = best_config.ok_or("No supported config found")?;
    info!("Using audio config: {:?}", supported_config);

    let config: cpal::StreamConfig = supported_config.into();

    let stream = device.build_input_stream(
        &config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            if !data.iter().any(|&sample| sample != 0.0) {
                debug!("Detected no audio input");
            }
            if let Err(e) = audio_sender.try_send(data.to_vec()) {
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

async fn audio_processing_task(
    audio_buffer: Arc<CircularAudioBuffer>,
    mut audio_receiver: mpsc::Receiver<Vec<f32>>,
    last_log_time: Arc<Mutex<Instant>>
) {
    while let Some(data) = audio_receiver.recv().await {
        audio_buffer.write(&data);
        
        // Rate-limited logging
        let mut last_time = last_log_time.lock().unwrap();
        if last_time.elapsed() >= Duration::from_secs(10) {
            info!("Audio buffer status: wrote {} samples", data.len());
            *last_time = Instant::now();
        }
    }
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
