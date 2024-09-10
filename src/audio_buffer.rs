use std::sync::{Arc, Mutex};
use rb::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::io::Cursor;
use hound::{WavSpec, WavWriter};

pub struct CircularAudioBuffer {
    buffer: Arc<Mutex<SpscRb<f32>>>,
    write_position: Arc<AtomicUsize>,
    capacity: usize,
    sample_rate: u32,
}

impl CircularAudioBuffer {
    pub fn new(capacity: usize, sample_rate: u32) -> Self {
        CircularAudioBuffer {
            buffer: Arc::new(Mutex::new(SpscRb::new(capacity))),
            write_position: Arc::new(AtomicUsize::new(0)),
            capacity,
            sample_rate,
        }
    }

    pub fn write(&self, data: &[f32]) {
        let mut buffer = self.buffer.lock().unwrap();
        let producer = buffer.producer();
        let current_position = self.write_position.load(Ordering::Relaxed);
        let data_len = data.len();

        let first_write = std::cmp::min(self.capacity - current_position, data_len);
        let _ = producer.write(&data[..first_write]);
        
        if first_write < data_len {
            let _ = producer.write(&data[first_write..]);
        }
        
        let new_position = (current_position + data_len) % self.capacity;
        self.write_position.store(new_position, Ordering::Relaxed);
    }

    pub fn read(&self) -> Vec<f32> {
        let buffer = self.buffer.lock().unwrap();
        let consumer = buffer.consumer();
        let write_pos = self.write_position.load(Ordering::Relaxed);

        let mut audio_data = vec![0.0; self.capacity];
        let _ = consumer.read(&mut audio_data);

        let start_pos = if write_pos == 0 { self.capacity - 1 } else { write_pos - 1 };
        audio_data.rotate_right(start_pos);

        audio_data
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn encode<T: AudioEncoder>(&self, encoder: T) -> Vec<u8> {
        let audio_data = self.read();
        encoder.encode(&audio_data, self.sample_rate)
    }
}

pub trait AudioEncoder {
    fn encode(&self, data: &[f32], sample_rate: u32) -> Vec<u8>;
}

pub struct WavEncoder;

impl AudioEncoder for WavEncoder {
    fn encode(&self, data: &[f32], sample_rate: u32) -> Vec<u8> {
        let spec = WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut wav_buffer = Cursor::new(Vec::new());
        {
            let mut writer = WavWriter::new(&mut wav_buffer, spec).unwrap();
            for &sample in data.iter() {
                let value = (sample * 32767.0) as i16;
                writer.write_sample(value).unwrap();
            }
            writer.finalize().unwrap();
        }
        wav_buffer.into_inner()
    }
}

pub struct Mp3Encoder;

impl AudioEncoder for Mp3Encoder {
    fn encode(&self, data: &[f32], sample_rate: u32) -> Vec<u8> {
        // MP3 encoding logic here
    }
}
// let mp3_encoder = Mp3Encoder;
// let mp3_data = state.audio_buffer.encode(mp3_encoder);
