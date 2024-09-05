/*
life-logging: A Rust-based continuous audio recording service

This program implements a local server that provides a continuous audio recording
service. It aims to minimize memory usage and interaction with other programs
while maintaining a rolling buffer of the last 300 seconds of audio in memory.

Key components:
1. Ring Buffer: Uses the `ringbuf` crate to efficiently manage audio data.
   - HeapRb: A heap-allocated ring buffer for storing audio samples.
   - SPSC (Single Producer, Single Consumer): Allows concurrent writing and reading.

2. Audio Capture: (To be implemented) Will use the `cpal` crate for audio input.

3. Server: (To be implemented) Will use the `rocket` crate for the web interface.

Current functionality:
- Creates a ring buffer and demonstrates basic push/pop operations.

Next steps:
- Implement a basic audio capture function using `cpal`.
- Create a separate thread for continuous audio capture.

Future improvements:
- Implement the web server for starting/stopping the recording.
- Add configuration options (e.g., buffer duration, sample rate).
- Implement audio compression to reduce memory usage.
*/

use ringbuf::{traits::*, HeapRb};

fn main() {
    // Create a ring buffer with capacity for 10 i16 samples
    let mut rb = HeapRb::<i16>::new(10);
    let (mut producer, mut consumer) = rb.split_ref();

    // Add some samples
    for i in 0..5 {
        producer.try_push(i as i16).unwrap();
    }

    // Read and print the samples
    while let Some(item) = consumer.try_pop() {
        println!("Got sample: {}", item);
    }
}
