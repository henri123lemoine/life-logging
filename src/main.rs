mod audio_buffer;

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
use std::time::{Duration, Instant};
use audio_buffer::{CircularAudioBuffer, WavEncoder};

const SAMPLE_RATE: u32 = 48000; // 48 kHz
const MAX_BUFFER_DURATION: usize = 60; // 60 seconds
const BUFFER_SIZE: usize = SAMPLE_RATE as usize * MAX_BUFFER_DURATION;
const ADDR: ([u8; 4], u16) = ([127, 0, 0, 1], 3000);

struct AppState {
    audio_buffer: Arc<CircularAudioBuffer>,
    audio_sender: mpsc::Sender<Vec<f32>>,
    start_time: std::time::SystemTime,
    last_log_time: Arc<Mutex<Instant>>,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    let audio_buffer = Arc::new(CircularAudioBuffer::new(BUFFER_SIZE, SAMPLE_RATE));
    let (audio_sender, audio_receiver) = mpsc::channel(1024);
    
    let app_state = Arc::new(AppState {
        audio_buffer: audio_buffer.clone(),
        audio_sender,
        start_time: std::time::SystemTime::now(),
        last_log_time: Arc::new(Mutex::new(Instant::now())),
    });

    // Initialize buffer with silence
    {
        let silence = vec![0.0; BUFFER_SIZE];
        app_state.audio_buffer.write(&silence);
    }

    // Start audio processing task
    tokio::spawn(audio_processing_task(
        audio_buffer.clone(),
        audio_receiver,
        app_state.last_log_time.clone()
    ));

    // Set up audio capture
    setup_audio_capture(app_state.audio_sender.clone());

    let app = Router::new()
        .route("/", get(|| async { "Audio Recording Server" }))
        .route("/health", get(health_check))
        .route("/get_audio", get(get_audio))
        .with_state(app_state);

    let addr = SocketAddr::from(ADDR);
    println!("listening on {}", addr);
    axum_server::bind(addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn health_check(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let uptime = state.start_time.elapsed().unwrap_or_default();
    Json(json!({
        "status": "ok",
        "uptime": format!("{}s", uptime.as_secs()),
        "message": "Audio Recording Server is running"
    }))
}

fn setup_audio_capture(audio_sender: mpsc::Sender<Vec<f32>>) {
    let host = cpal::default_host();
    let device = host.default_input_device().expect("no input device available");
    let config = device.default_input_config().unwrap();

    println!("Audio input config: {:?}", config);

    let stream: cpal::Stream = device.build_input_stream(
        &config.into(),
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            if !data.iter().any(|&sample| sample != 0.0) {
                println!("Detected no audio input");
            }
            let _ = audio_sender.try_send(data.to_vec());
        },
        |err| eprintln!("An error occurred on stream: {}", err),
        Some(Duration::from_secs(2))
    ).unwrap();

    stream.play().unwrap();
    println!("Audio stream started");

    std::mem::forget(stream);
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
            println!("Audio buffer status: wrote {} samples", data.len());
            *last_time = Instant::now();
        }
    }
}

async fn get_audio(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let wav_encoder = WavEncoder;
    let wav_data = state.audio_buffer.encode(wav_encoder);

    println!("Encoded {} bytes of WAV audio data", wav_data.len());

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "audio/wav"),
            (header::CONTENT_DISPOSITION, "attachment; filename=\"audio.wav\""),
        ],
        wav_data,
    )
}
