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

use ringbuf::{HeapRb, traits::*};

const BUFFER_SIZE: usize = 10;

fn main() {
    // Create a ring buffer with capacity for BUFFER_SIZE i16 samples
    let rb = HeapRb::<i16>::new(BUFFER_SIZE);
    let (mut producer, mut consumer) = rb.split();

    // Push samples
    for i in 0..5 {
        if let Err(e) = producer.try_push(i as i16) {
            println!("Failed to push sample {}: {:?}", i, e);
        }
    }

    // Pop and print samples
    while let Some(item) = consumer.try_pop() {
        println!("Got sample: {}", item);
    }
}
