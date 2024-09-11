use crate::audio::encoder::AudioEncoder;
use crate::audio::visualizer::AudioVisualizer;

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::info;

pub struct CircularAudioBuffer {
    buffer: Arc<Mutex<Vec<f32>>>,
    write_position: Arc<AtomicUsize>,
    capacity: usize,
    sample_rate: u32,
}

impl CircularAudioBuffer {
    pub fn new(capacity: usize, sample_rate: u32) -> Self {
        info!("Creating new CircularAudioBuffer with capacity {} and sample rate {}", capacity, sample_rate);
        CircularAudioBuffer {
            buffer: Arc::new(Mutex::new(vec![0.0; capacity])),
            write_position: Arc::new(AtomicUsize::new(0)),
            capacity,
            sample_rate,
        }
    }

    pub fn write(&self, data: &[f32]) {
        let mut buffer = self.buffer.lock().unwrap();
        let current_position = self.write_position.load(Ordering::Relaxed);
        let data_len = data.len();

        for (i, &sample) in data.iter().enumerate() {
            let pos = (current_position + i) % self.capacity;
            buffer[pos] = sample;
        }
        
        let new_position = (current_position + data_len) % self.capacity;
        self.write_position.store(new_position, Ordering::Relaxed);
    }

    pub fn read(&self) -> Vec<f32> {
        let buffer = self.buffer.lock().unwrap();
        let write_pos = self.write_position.load(Ordering::Relaxed);

        let mut audio_data = Vec::with_capacity(self.capacity);
        audio_data.extend_from_slice(&buffer[write_pos..]);
        audio_data.extend_from_slice(&buffer[..write_pos]);
        audio_data
    }

    #[allow(dead_code)]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    #[allow(dead_code)]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn visualize(&self, width: u32, height: u32) -> Vec<u8> {
        let audio_data = self.read();
        info!("Generating waveform visualization with dimensions {}x{}", width, height);
        AudioVisualizer::create_waveform(&audio_data, width, height)
    }

    pub fn encode<T: AudioEncoder>(&self, encoder: T) -> Vec<u8> {
        let audio_data = self.read();
        info!("Encoding {} samples of audio data", audio_data.len());
        encoder.encode(&audio_data, self.sample_rate)
    }
}
