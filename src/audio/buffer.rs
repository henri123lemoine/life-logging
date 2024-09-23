use crate::audio::encoder::AudioEncoder;
use crate::audio::processor;
use crate::audio::visualizer::AudioVisualizer;
use crate::error::Result;
use std::time::Duration;
use tracing::info;

pub struct CircularAudioBuffer {
    pub buffer: Vec<f32>,
    pub write_position: usize,
    pub capacity: usize,
    pub sample_rate: u32,
    pub is_consistent: bool,
}

impl CircularAudioBuffer {
    pub fn new(capacity: usize, sample_rate: u32) -> Self {
        info!(
            "Creating new CircularAudioBuffer with capacity {} and sample rate {}",
            capacity, sample_rate
        );
        CircularAudioBuffer {
            buffer: vec![0.0; capacity],
            write_position: 0,
            capacity,
            sample_rate,
            is_consistent: true,
        }
    }

    pub fn write(&mut self, data: &[f32]) {
        let current_position = self.write_position;
        let data_len = data.len();

        for (i, &sample) in data.iter().enumerate() {
            let pos = (current_position + i) % self.capacity;
            self.buffer[pos] = sample;
        }

        let new_position = (current_position + data_len) % self.capacity;
        self.write_position = new_position;
        self.is_consistent = true;
    }

    pub fn read(&self, duration: Option<Duration>) -> Vec<f32> {
        let samples_to_return = if let Some(duration) = duration {
            (duration.as_secs_f32() * self.sample_rate as f32) as usize
        } else {
            self.capacity
        }
        .min(self.capacity);

        let mut audio_data = Vec::with_capacity(samples_to_return);
        let start_pos = (self.write_position + self.capacity - samples_to_return) % self.capacity;

        if start_pos + samples_to_return <= self.capacity {
            audio_data.extend_from_slice(&self.buffer[start_pos..start_pos + samples_to_return]);
        } else {
            let first_part = self.capacity - start_pos;
            audio_data.extend_from_slice(&self.buffer[start_pos..]);
            audio_data.extend_from_slice(&self.buffer[..samples_to_return - first_part]);
        }

        audio_data
    }

    pub fn encode(
        &self,
        encoder: &dyn AudioEncoder,
        duration: Option<Duration>,
    ) -> Result<Vec<u8>> {
        let audio_data = self.read(duration);
        encoder.encode(&audio_data, self.sample_rate)
    }

    pub fn visualize(&self, width: u32, height: u32) -> Vec<u8> {
        let audio_data = self.read(None);
        info!(
            "Generating waveform visualization with dimensions {}x{}",
            width, height
        );
        AudioVisualizer::create_waveform(&audio_data, width, height)
    }

    #[allow(dead_code)]
    pub fn detect_silence(&self, threshold: f32) -> Vec<(usize, usize)> {
        let data = self.read(None);
        processor::detect_silence(&data, threshold)
    }

    #[allow(dead_code)]
    pub fn compute_spectrum(&self) -> Vec<f32> {
        let data = self.read(None);
        processor::compute_spectrum(&data)
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.capacity
    }
}
