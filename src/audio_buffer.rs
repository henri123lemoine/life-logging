use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::io::Cursor;
use hound::{WavSpec, WavWriter};
use plotters::prelude::*;
use plotters::backend::RGBPixel;
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

    pub fn capacity(&self) -> usize {
        self.capacity
    }

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
        let encoded_data = wav_buffer.into_inner();
        info!("Encoded {} samples into {} bytes of WAV data", data.len(), encoded_data.len());
        encoded_data
    }
}

// pub struct Mp3Encoder;

// impl AudioEncoder for Mp3Encoder {
//     fn encode(&self, data: &[f32], sample_rate: u32) -> Vec<u8> {
//         // MP3 encoding logic here
//         vec![]
//     }
// }
// // let mp3_encoder = Mp3Encoder;
// // let mp3_data = state.audio_buffer.encode(mp3_encoder);

pub struct AudioVisualizer;

impl AudioVisualizer {
    pub fn create_waveform(data: &[f32], width: u32, height: u32) -> Vec<u8> {
        let mut buffer = vec![0u8; (width * height * 3) as usize];
        {
            let root = BitMapBackend::<RGBPixel>::with_buffer_and_format(
                &mut buffer,
                (width, height)
            ).unwrap().into_drawing_area();
            
            root.fill(&WHITE).unwrap();

            let mut chart = ChartBuilder::on(&root)
                .build_cartesian_2d(0f32..data.len() as f32, -1f32..1f32)
                .unwrap();

            chart
                .configure_mesh()
                .disable_x_mesh()
                .disable_y_mesh()
                .draw()
                .unwrap();

            chart
                .draw_series(LineSeries::new(
                    data.iter().enumerate().map(|(i, &v)| (i as f32, v)),
                    &RED,
                ))
                .unwrap();

            root.present().unwrap();
        }

        // Convert RGB buffer to PNG
        let mut png_data = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png_data, width, height);
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&buffer).unwrap();
        }

        info!("Generated waveform visualization of {} bytes", png_data.len());
        png_data
    }
}
