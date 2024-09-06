use rb::*;
use std::thread;
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rocket::{self, get, State, routes};
use log::{error, info};
use env_logger;

const BUFFER_SIZE: usize = 48000 * 300; // 300 seconds of audio at 48kHz

struct AppState {
    is_recording: Arc<AtomicBool>,
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    info!("Starting application");

    let rb = SpscRb::new(BUFFER_SIZE);
    let (prod, cons) = (rb.producer(), rb.consumer());
    let is_recording = Arc::new(AtomicBool::new(false));
    let is_recording_audio = is_recording.clone();

    // Audio capture thread
    thread::spawn(move || {
        info!("Starting audio capture thread");
        let host = cpal::default_host();
        let device = host.default_input_device().expect("no input device available");
        let config = device.default_input_config().unwrap();

        let stream = device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if is_recording_audio.load(Ordering::Relaxed) {
                    let _ = prod.write_blocking(data);
                    info!("Wrote {} samples to buffer", data.len());
                }
            },
            |err| error!("An error occurred on stream: {}", err),
            Some(Duration::from_secs(2))
        ).unwrap();

        stream.play().unwrap();
        info!("Audio stream started");

        loop {
            thread::sleep(Duration::from_secs(1));
        }
    });

    // Web server thread
    let rocket = rocket::build()
        .manage(AppState { is_recording: is_recording.clone() })
        .mount("/", routes![start_recording, stop_recording]);

    thread::spawn(move || {
        info!("Starting web server");
        rocket.launch();
    });

    info!("Main thread entering wait loop");
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

#[get("/start")]
fn start_recording(state: &State<AppState>) -> &'static str {
    state.is_recording.store(true, Ordering::Relaxed);
    info!("Recording started");
    "Recording started"
}

#[get("/stop")]
fn stop_recording(state: &State<AppState>) -> &'static str {
    state.is_recording.store(false, Ordering::Relaxed);
    info!("Recording stopped");
    "Recording stopped"
}
