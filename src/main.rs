use axum::{
    routing::get,
    Router, extract::State, extract::Query,
    response::IntoResponse,
    http::{header, StatusCode},
};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use rb::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::time::Duration;
use serde::Deserialize;
use hound::{WavWriter, WavSpec};
use std::io::Cursor;

const SAMPLE_RATE: u32 = 48000; // 48 kHz
const MAX_BUFFER_DURATION: usize = 5; // 5 seconds
// const MAX_BUFFER_DURATION: usize = 5 * 60; // 5 minutes
const BUFFER_SIZE: usize = SAMPLE_RATE as usize * MAX_BUFFER_DURATION;
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

async fn audio_processing_task(ring_buffer: Arc<Mutex<SpscRb<f32>>>, mut audio_receiver: mpsc::Receiver<Vec<f32>>) {
    while let Some(data) = audio_receiver.recv().await {
        let buffer = ring_buffer.lock().unwrap();
        let producer = buffer.producer();
        let written = producer.write(&data);
        match written {
            Ok(_) => {},  // Do nothing for the Ok case
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
) -> impl IntoResponse {
    let seconds = params.seconds.unwrap_or(MAX_BUFFER_DURATION);
    let samples_to_read = seconds * SAMPLE_RATE as usize;

    let buffer = state.ring_buffer.lock().unwrap();
    let consumer = buffer.consumer();
    let mut audio_data = Vec::with_capacity(samples_to_read);
    let read = consumer.read(&mut audio_data);
    match read {
        Ok(read) => println!("Read {} samples from ring buffer", read),
        Err(e) => eprintln!("Error reading from ring buffer: {:?}", e),
    }

    println!("Ring buffer capacity: {}", buffer.capacity());
    println!("Ring buffer count: {}", buffer.count());

    if audio_data.iter().any(|&sample| sample != 0.0) {
        println!("Detected non-zero audio in buffer");
    } else {
        println!("Warning: All zero samples in buffer");
    }

    // If we read fewer samples than requested, pad with silence
    if audio_data.len() < samples_to_read {
        audio_data.resize(samples_to_read, 0.0);
    }

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
        for &sample in audio_data.iter().take(samples_to_read) {
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
