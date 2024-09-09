use axum::{
    routing::{get, post},
    Router, extract::State,
};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use rb::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::time::Duration;

const BUFFER_SIZE: usize = 48000 * 300; // 300 seconds of audio at 48kHz

struct AppState {
    ring_buffer: Arc<SpscRb<f32>>,
    is_recording: Arc<Mutex<bool>>,
    audio_sender: mpsc::Sender<Vec<f32>>,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    let ring_buffer = Arc::new(SpscRb::new(BUFFER_SIZE));
    let is_recording = Arc::new(Mutex::new(false));
    
    let (audio_sender, audio_receiver) = mpsc::channel(1024);
    
    let app_state = Arc::new(AppState {
        ring_buffer: ring_buffer.clone(),
        is_recording: is_recording.clone(),
        audio_sender,
    });

    // Start audio processing task
    tokio::spawn(audio_processing_task(ring_buffer.clone(), audio_receiver));

    // Set up audio capture
    setup_audio_capture(is_recording.clone(), app_state.audio_sender.clone());

    let app = Router::new()
        .route("/", get(|| async { "Hello, world!" }))
        .route("/start", post(start_recording))
        .route("/stop", post(stop_recording))
        .route("/get_audio", get(get_audio))
        .with_state(app_state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("listening on {}", addr);
    axum_server::bind(addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

fn setup_audio_capture(is_recording: Arc<Mutex<bool>>, audio_sender: mpsc::Sender<Vec<f32>>) {
    let host = cpal::default_host();
    let device = host.default_input_device().expect("no input device available");
    let config = device.default_input_config().unwrap();

    let stream = device.build_input_stream(
        &config.into(),
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            if *is_recording.lock().unwrap() {
                let _ = audio_sender.try_send(data.to_vec());
            }
        },
        |err| eprintln!("An error occurred on stream: {}", err),
        Some(Duration::from_secs(2))
    ).unwrap();

    stream.play().unwrap();

    // Keep the stream alive by not dropping it
    std::mem::forget(stream);
}

async fn audio_processing_task(ring_buffer: Arc<SpscRb<f32>>, mut audio_receiver: mpsc::Receiver<Vec<f32>>) {
    let producer = ring_buffer.producer();
    while let Some(data) = audio_receiver.recv().await {
        let _ = producer.write(&data);
    }
}

async fn start_recording(State(state): State<Arc<AppState>>) -> &'static str {
    *state.is_recording.lock().unwrap() = true;
    "Recording started"
}

async fn stop_recording(State(state): State<Arc<AppState>>) -> &'static str {
    *state.is_recording.lock().unwrap() = false;
    "Recording stopped"
}

async fn get_audio(State(state): State<Arc<AppState>>) -> Vec<u8> {
    let consumer = state.ring_buffer.consumer();
    let mut audio_data = Vec::new();
    let _ = consumer.read(&mut audio_data);
    
    // Convert f32 samples to bytes (assuming 16-bit PCM)
    audio_data.iter()
        .flat_map(|&sample| {
            let value = (sample * 32767.0) as i16;
            value.to_le_bytes().to_vec()
        })
        .collect()
}
