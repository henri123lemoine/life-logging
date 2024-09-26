use crate::prelude::*;
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
    buffer: Vec<T>,
    write_position: usize,
    capacity: usize,
}

impl<T: Copy + Default> CircularBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        CircularBuffer {
            buffer: vec![T::default(); capacity],
            write_position: 0,
            capacity,
        }
    }

    pub fn write(&mut self, data: &[T]) {
        let data_len = data.len();

        for (i, &sample) in data.iter().enumerate() {
            let pos = (self.write_position + i) % self.capacity;
            self.buffer[pos] = sample;
        }

        self.write_position = (self.write_position + data_len) % self.capacity;
    }

    // See benches for performance comparison
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

    #[allow(dead_code)]
    fn update_capacity(&mut self, new_capacity: usize) {
        let mut new_buffer = vec![T::default(); new_capacity];
        let count = self.capacity.min(new_capacity);

        new_buffer
            .iter_mut()
            .enumerate()
            .take(count)
            .for_each(|(i, new_item)| {
                let old_pos = (self.write_position + self.capacity - count + i) % self.capacity;
                *new_item = self.buffer[old_pos];
            });

        self.buffer = new_buffer;
        self.capacity = new_capacity;
        self.write_position = count % new_capacity;
    }
}

pub struct AudioBuffer {
    buffer: CircularBuffer<f32>,
    sample_rate: u32,
}

impl AudioBuffer {
    pub fn new(capacity: usize, sample_rate: u32) -> AudioBuffer {
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

    pub fn update_sample_rate(&mut self, new_sample_rate: u32) -> Result<()> {
        if self.sample_rate == new_sample_rate {
            return Ok(());
        }

        let new_capacity = (self.buffer.capacity as f32 * new_sample_rate as f32
            / self.sample_rate as f32)
            .ceil() as usize;
        let old_data = self.buffer.read(self.buffer.capacity);

        // Resample the existing data
        let new_data: Vec<f32> = (0..new_capacity)
            .map(|i| {
                let old_index = i as f32 * self.sample_rate as f32 / new_sample_rate as f32;
                let old_index_floor = old_index.floor() as usize;
                let old_index_ceil = old_index.ceil() as usize;
                let frac = old_index - old_index.floor();

                if old_index_ceil >= old_data.len() {
                    old_data[old_index_floor]
                } else {
                    old_data[old_index_floor] * (1.0 - frac) + old_data[old_index_ceil] * frac
                }
            })
            .collect();

        self.buffer = CircularBuffer::new(new_capacity);
        self.buffer.write(&new_data);
        self.sample_rate = new_sample_rate;

        info!(
            "Updated sample rate to {} Hz, new capacity: {} samples",
            new_sample_rate, new_capacity
        );

        Ok(())
    }

    pub fn get_sample_rate(&self) -> u32 {
        self.sample_rate
    }
}
