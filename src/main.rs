use axum::{
    routing::get,
    Router, extract::State, extract::Query,
    response::IntoResponse,
    http::{header, StatusCode},
    Json,
};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use rb::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::time::{Duration, Instant};
// use serde::Deserialize;
use hound::{WavWriter, WavSpec};
use std::io::Cursor;
use std::sync::atomic::{AtomicUsize, Ordering};

const SAMPLE_RATE: u32 = 48000; // 48 kHz
const MAX_BUFFER_DURATION: usize = 60; // 60 seconds
const BUFFER_SIZE: usize = SAMPLE_RATE as usize * MAX_BUFFER_DURATION;
const ADDR: ([u8; 4], u16) = ([127, 0, 0, 1], 3000);

struct AppState {
    ring_buffer: Arc<Mutex<SpscRb<f32>>>,
    audio_sender: mpsc::Sender<Vec<f32>>,
    write_position: Arc<AtomicUsize>,
    start_time: std::time::SystemTime,
    last_log_time: Arc<Mutex<Instant>>,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    let ring_buffer = Arc::new(Mutex::new(SpscRb::new(BUFFER_SIZE)));
    let (audio_sender, audio_receiver) = mpsc::channel(1024);
    
    let app_state = Arc::new(AppState {
        ring_buffer: ring_buffer.clone(),
        audio_sender,
        write_position: Arc::new(AtomicUsize::new(0)),
        start_time: std::time::SystemTime::now(),
        last_log_time: Arc::new(Mutex::new(Instant::now())),
    });

    // Initialize buffer with silence
    {
        let mut buffer = app_state.ring_buffer.lock().unwrap();
        let producer = buffer.producer();
        let silence = vec![0.0; BUFFER_SIZE];
        let _ = producer.write(&silence);
    }

    // Start audio processing task
    tokio::spawn(audio_processing_task(
        ring_buffer.clone(),
        audio_receiver,
        app_state.write_position.clone(),
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
    ring_buffer: Arc<Mutex<SpscRb<f32>>>,
    mut audio_receiver: mpsc::Receiver<Vec<f32>>,
    write_position: Arc<AtomicUsize>,
    last_log_time: Arc<Mutex<Instant>>
) {
    while let Some(data) = audio_receiver.recv().await {
        let mut buffer = ring_buffer.lock().unwrap();
        let producer = buffer.producer();
        let buffer_capacity = buffer.capacity();
        let current_position = write_position.load(Ordering::Relaxed);
        let data_len = data.len();
        
        // Write data, wrapping around if necessary
        let first_write = std::cmp::min(buffer_capacity - current_position, data_len);
        let _ = producer.write(&data[..first_write]);
        
        if first_write < data_len {
            let _ = producer.write(&data[first_write..]);
        }
        
        let new_position = (current_position + data_len) % buffer_capacity;
        write_position.store(new_position, Ordering::Relaxed);
        
        // Rate-limited logging
        let mut last_time = last_log_time.lock().unwrap();
        if last_time.elapsed() >= Duration::from_secs(10) {
            println!("Audio buffer status: wrote {} samples, buffer position: {}/{}", 
                     data_len, new_position, buffer_capacity);
            *last_time = Instant::now();
        }
    }
}

async fn get_audio(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let buffer = state.ring_buffer.lock().unwrap();
    let consumer = buffer.consumer();
    let write_pos = state.write_position.load(Ordering::Relaxed);
    let buffer_size = buffer.capacity();

    let mut audio_data = vec![0.0; buffer_size];

    // Read the most recent data first (from write_pos to end)
    let read_recent = consumer.read(&mut audio_data[..buffer_size - write_pos]).unwrap_or(0);

    // Then read the older data (from start to write_pos)
    let read_older = consumer.read(&mut audio_data[buffer_size - write_pos..]).unwrap_or(0);

    let total_read = read_recent + read_older;

    // Rearrange the buffer so that the oldest data comes first
    audio_data.rotate_left(buffer_size - write_pos);

    println!("Read {} samples from ring buffer", total_read);

    // Create a WAV file in memory
    let spec = WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut wav_buffer = Cursor::new(Vec::new());
    {
        let mut writer = WavWriter::new(&mut wav_buffer, spec).unwrap();
        for &sample in audio_data.iter() {
            let value = (sample * 32767.0) as i16;
            writer.write_sample(value).unwrap();
        }
        writer.finalize().unwrap();
    }

    let wav_data = wav_buffer.into_inner();
    println!("Returning {} bytes of WAV audio data", wav_data.len());

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "audio/wav"),
            (header::CONTENT_DISPOSITION, "attachment; filename=\"audio.wav\""),
        ],
        wav_data,
    )
}
