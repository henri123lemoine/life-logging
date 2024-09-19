use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tracing::info;
use crate::audio::encoder::AudioEncoder;
use crate::audio::processor;
use crate::audio::visualizer::AudioVisualizer;
use crate::error::Result;

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

    pub fn get_last_n_seconds(&self, duration: Duration) -> Vec<f32> {
        let samples_per_second = self.sample_rate as usize;
        let samples_to_return = (duration.as_secs() as usize * samples_per_second)
            .min(self.capacity);

        let write_pos = self.write_position.load(std::sync::atomic::Ordering::Relaxed);
        let start_pos = if samples_to_return >= self.capacity {
            (write_pos + 1) % self.capacity
        } else {
            (write_pos + self.capacity - samples_to_return) % self.capacity
        };

        let mut result = Vec::with_capacity(samples_to_return);

        // Minimize the time we hold the lock
        {
            let buffer = self.buffer.lock().unwrap();
            if start_pos < write_pos {
                result.extend_from_slice(&buffer[start_pos..write_pos]);
            } else {
                result.extend_from_slice(&buffer[start_pos..]);
                result.extend_from_slice(&buffer[..write_pos]);
            }
        }

        result
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn visualize(&self, width: u32, height: u32) -> Vec<u8> {
        let audio_data = self.read();
        info!("Generating waveform visualization with dimensions {}x{}", width, height);
        AudioVisualizer::create_waveform(&audio_data, width, height)
    }

    #[allow(dead_code)]
    pub fn encode(&self, encoder: &dyn AudioEncoder) -> Result<Vec<u8>> {
        let audio_data = self.read();
        info!("Encoding {} samples of audio data", audio_data.len());
        encoder.encode(&audio_data, self.sample_rate)
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.capacity
    }

    #[allow(dead_code)]
    pub fn normalize_volume(&mut self, target_peak: f32) -> Result<()> {
        let mut data = self.read();
        processor::normalize_volume(&mut data, target_peak)?;
        self.write(&data);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn detect_silence(&self, threshold: f32) -> Vec<(usize, usize)> {
        let data = self.read();
        processor::detect_silence(&data, threshold)
    }

    #[allow(dead_code)]
    pub fn compute_spectrum(&self) -> Vec<f32> {
        let data = self.read();
        processor::compute_spectrum(&data)
    }
}
