use crate::audio::encoder::AudioEncoder;
use crate::audio::visualizer::AudioVisualizer;
use crate::error::Result;
use std::time::Duration;
use tracing::info;

/// A circular buffer for storing audio data with a fixed capacity.
///
/// This buffer continuously overwrites old data when it reaches its capacity,
/// effectively maintaining a rolling window of the most recent audio samples.
///
/// # Fields
/// * `buffer`: The underlying storage for audio samples.
/// * `write_position`: The current position where new samples will be written.
/// * `capacity`: The total number of samples the buffer can hold.
/// * `sample_rate`: The number of samples per second for the stored audio.
///
/// # Behavior
/// - When new data is written, it overwrites the oldest data if the buffer is full.
/// - The buffer always contains the most recent `capacity` samples of audio.
/// - For a 120-second buffer, `capacity` would be `120 * sample_rate`.
pub struct CircularAudioBuffer {
    pub buffer: Vec<f32>,
    pub write_position: usize,
    pub capacity: usize,
    pub sample_rate: u32,
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
        AudioVisualizer::create_waveform(&audio_data, width, height)
    }
}
