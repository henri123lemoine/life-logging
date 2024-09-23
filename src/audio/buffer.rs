use crate::audio::encoder::AudioEncoder;
use crate::audio::visualizer::AudioVisualizer;
use crate::error::Result;
use std::ptr;
use std::time::Duration;
use tracing::info;

/// A circular buffer for storing data with a fixed capacity.
///
/// This buffer continuously overwrites old data, effectively maintaining
/// a rolling window of the most recent audio samples.
///
/// # Fields
/// * `buffer`: The underlying storage for audio samples.
/// * `write_position`: The current position where new samples will be written.
/// * `capacity`: The total number of samples the buffer can hold.
pub struct CircularBuffer<T> {
    pub buffer: Vec<T>,
    pub write_position: usize,
    pub capacity: usize,
}

impl<T: Copy + Default> CircularBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        CircularBuffer {
            buffer: vec![T::default(); capacity],
            write_position: 0,
            capacity,
        }
    }

    // write            time:   [656.94 ns 658.54 ns 660.66 ns]
    pub fn write(&mut self, data: &[T]) {
        let data_len = data.len();

        for (i, &sample) in data.iter().enumerate() {
            let pos = (self.write_position + i) % self.capacity;
            self.buffer[pos] = sample;
        }

        self.write_position = (self.write_position + data_len) % self.capacity;
    }

    // write fast         time:   [55.758 ns 56.001 ns 56.263 ns] !!
    pub fn write_fast(&mut self, data: &[T]) {
        let data_len = data.len();
        let remaining_space = self.capacity - self.write_position;

        if data_len <= remaining_space {
            unsafe {
                ptr::copy_nonoverlapping(
                    data.as_ptr(),
                    self.buffer.as_mut_ptr().add(self.write_position),
                    data_len,
                );
            }
        } else {
            unsafe {
                ptr::copy_nonoverlapping(
                    data.as_ptr(),
                    self.buffer.as_mut_ptr().add(self.write_position),
                    remaining_space,
                );
                ptr::copy_nonoverlapping(
                    data.as_ptr().add(remaining_space),
                    self.buffer.as_mut_ptr(),
                    data_len - remaining_space,
                );
            }
        }

        self.write_position = (self.write_position + data_len) % self.capacity;
    }

    pub fn read(&self, count: usize) -> Vec<T> {
        let start_pos = (self.write_position + self.capacity - count) % self.capacity;

        if start_pos + count <= self.capacity {
            self.buffer[start_pos..start_pos + count].to_vec()
        } else {
            let mut data = Vec::with_capacity(count);
            data.extend_from_slice(&self.buffer[start_pos..]);
            data.extend_from_slice(&self.buffer[..count - (self.capacity - start_pos)]);
            data
        }
    }
}

pub struct AudioBuffer {
    pub buffer: CircularBuffer<f32>,
    pub sample_rate: u32,
}

impl AudioBuffer {
    pub fn new(capacity: usize, sample_rate: u32) -> Self {
        info!(
            "Creating new AudioBuffer with capacity {} and sample rate {}",
            capacity, sample_rate
        );
        AudioBuffer {
            buffer: CircularBuffer::new(capacity),
            sample_rate,
        }
    }

    pub fn write(&mut self, data: &[f32]) {
        self.buffer.write(data);
    }

    pub fn write_fast(&mut self, data: &[f32]) {
        self.buffer.write_fast(data);
    }

    pub fn read(&self, duration: Option<Duration>) -> Vec<f32> {
        let samples_to_return = if let Some(duration) = duration {
            (duration.as_secs_f32() * self.sample_rate as f32) as usize
        } else {
            self.buffer.capacity
        }
        .min(self.buffer.capacity);

        self.buffer.read(samples_to_return)
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
