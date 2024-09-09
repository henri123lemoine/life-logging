use axum::{
    routing::{get, post},
    Router, extract::State, extract::Query,
};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use rb::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::time::Duration;
use serde::Deserialize;

const SAMPLE_RATE: usize = 24000; // 24 kHz
const MAX_BUFFER_DURATION: usize = 5 * 60; // 5 minutes
const BUFFER_SIZE: usize = SAMPLE_RATE * MAX_BUFFER_DURATION;
const ADDR: ([u8; 4], u16) = ([127, 0, 0, 1], 3000);

struct AppState {
    ring_buffer: Arc<Mutex<SpscRb<f32>>>,
    audio_sender: mpsc::Sender<Vec<f32>>,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    let ring_buffer = Arc::new(Mutex::new(SpscRb::new(BUFFER_SIZE)));
    let (audio_sender, audio_receiver) = mpsc::channel(1024);
    
    let app_state = Arc::new(AppState {
        ring_buffer: ring_buffer.clone(),
        audio_sender,
    });

    // Start audio processing task
    tokio::spawn(audio_processing_task(ring_buffer.clone(), audio_receiver));

    // Set up audio capture
    setup_audio_capture(app_state.audio_sender.clone());

    let app = Router::new()
        .route("/", get(|| async { "Audio Recording Server" }))
        .route("/get_audio", get(get_audio))
        .with_state(app_state);

    let addr = SocketAddr::from(ADDR);
    println!("listening on {}", addr);
    axum_server::bind(addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

fn setup_audio_capture(audio_sender: mpsc::Sender<Vec<f32>>) {
    let host = cpal::default_host();
    let device = host.default_input_device().expect("no input device available");
    let config = device.default_input_config().unwrap();

    println!("Audio input config: {:?}", config);

    let stream = device.build_input_stream(
        &config.into(),
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            println!("Captured {} samples", data.len());
            let _ = audio_sender.try_send(data.to_vec());
        },
        |err| eprintln!("An error occurred on stream: {}", err),
        Some(Duration::from_secs(2))
    ).unwrap();

    stream.play().unwrap();
    println!("Audio stream started");

    std::mem::forget(stream);
}

async fn audio_processing_task(ring_buffer: Arc<Mutex<SpscRb<f32>>>, mut audio_receiver: mpsc::Receiver<Vec<f32>>) {
    while let Some(data) = audio_receiver.recv().await {
        println!("Received {} samples for processing", data.len());
        let mut buffer = ring_buffer.lock().unwrap();
        let producer = buffer.producer();
        let written = producer.write(&data);
        match written {
            Ok(written) => println!("Wrote {} samples to ring buffer", written),
            Err(e) => eprintln!("Error writing to ring buffer: {:?}", e),
        }
    }
}

#[derive(Deserialize)]
struct AudioQuery {
    seconds: Option<usize>,
}

async fn get_audio(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AudioQuery>,
) -> Vec<u8> {
    let seconds = params.seconds.unwrap_or(MAX_BUFFER_DURATION);
    let samples_to_read = seconds * SAMPLE_RATE;

    let buffer = state.ring_buffer.lock().unwrap();
    let consumer = buffer.consumer();
    let mut audio_data = Vec::with_capacity(samples_to_read);
    let read = consumer.read(&mut audio_data);
    match read {
        Ok(read) => println!("Read {} samples from ring buffer", read),
        Err(e) => eprintln!("Error reading from ring buffer: {:?}", e),
    }

    // If we read fewer samples than requested, pad with silence
    if audio_data.len() < samples_to_read {
        audio_data.resize(samples_to_read, 0.0);
    }

    // Convert f32 samples to bytes (16-bit PCM)
    let bytes: Vec<u8> = audio_data.iter()
        .take(samples_to_read)
        .flat_map(|&sample| {
            let value = (sample * 32767.0) as i16;
            value.to_le_bytes().to_vec()
        })
        .collect();
    
    println!("Returning {} bytes of audio data", bytes.len());
    bytes
}