/*
life-logging: A Rust-based continuous audio recording service

Project Plan:
1. Implement a ring buffer for efficient audio data management
   - Use HeapRb from the ringbuf crate for heap-allocated storage
   - Utilize SPSC (Single Producer, Single Consumer) for concurrent access

2. Develop audio capture functionality
   - Use the cpal crate for cross-platform audio input
   - Implement a separate thread for continuous audio capture

3. Create a local server interface
   - Use the rocket crate to provide a web-based control interface
   - Allow starting and stopping the recording service

4. Implement audio data management
   - Maintain a rolling buffer of the last 300 seconds of audio
   - Minimize memory usage and system resource consumption

5. Add configuration options
   - Allow customization of buffer duration, sample rate, etc.
*/

use rb::*;
use std::thread;
use std::time::Duration;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rocket::{self, get, post, routes};

const BUFFER_SIZE: usize = 48000 * 300; // 300 seconds of audio at 48kHz

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let rb = SpscRb::new(BUFFER_SIZE);
    let (prod, cons) = (rb.producer(), rb.consumer());

    // Audio capture thread
    thread::spawn(move || {
        let host = cpal::default_host();
        let device = host.default_input_device().expect("no input device available");
        let config = device.default_input_config().unwrap();

        let stream = device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let _ = prod.write_blocking(data);
            },
            |err| eprintln!("an error occurred on stream: {}", err),
            Some(Duration::from_secs(2)) // Added timeout parameter
        ).unwrap();

        stream.play().unwrap();

        std::thread::park(); // Keep the thread alive
    });

    // Web server thread
    thread::spawn(move || {
        rocket::build().mount("/", routes![start_recording, stop_recording]).launch();
    });

    // Main thread can handle other tasks or just wait
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

#[get("/start")]
fn start_recording() -> &'static str {
    // Implement start recording logic
    "Recording started"
}

#[get("/stop")]
fn stop_recording() -> &'static str {
    // Implement stop recording logic
    "Recording stopped"
}
