use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use plotters::prelude::*;
use plotters::backend::RGBPixel;
use tracing::info;
use std::process::Command;
use std::io::Write;
use tempfile::NamedTempFile;

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
        let channels = 1u16;
        let bits_per_sample = 16u16;
        let byte_rate = sample_rate * u32::from(channels) * u32::from(bits_per_sample) / 8;
        let block_align = channels * bits_per_sample / 8;
        let data_size = (data.len() * 2) as u32; // 2 bytes per sample for 16-bit
        let file_size = 36 + data_size;

        let mut buffer = Vec::with_capacity(file_size as usize + 8);

        // RIFF header
        buffer.extend_from_slice(b"RIFF");
        buffer.extend_from_slice(&(file_size as u32).to_le_bytes());
        buffer.extend_from_slice(b"WAVE");

        // Format chunk
        buffer.extend_from_slice(b"fmt ");
        buffer.extend_from_slice(&16u32.to_le_bytes()); // Chunk size
        buffer.extend_from_slice(&1u16.to_le_bytes()); // Audio format (PCM)
        buffer.extend_from_slice(&channels.to_le_bytes());
        buffer.extend_from_slice(&sample_rate.to_le_bytes());
        buffer.extend_from_slice(&byte_rate.to_le_bytes());
        buffer.extend_from_slice(&block_align.to_le_bytes());
        buffer.extend_from_slice(&bits_per_sample.to_le_bytes());

        // Data chunk
        buffer.extend_from_slice(b"data");
        buffer.extend_from_slice(&data_size.to_le_bytes());

        // Audio data
        for &sample in data {
            let value = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
            buffer.write_all(&value.to_le_bytes()).unwrap();
        }

        info!("Encoded {} samples into {} bytes of WAV data", data.len(), buffer.len());
        buffer
    }
}

pub struct FlacEncoder;

impl AudioEncoder for FlacEncoder {
    fn encode(&self, data: &[f32], sample_rate: u32) -> Vec<u8> {
        // Create a temporary WAV file
        let mut temp_wav = NamedTempFile::new().unwrap();
        let wav_encoder = WavEncoder;
        let wav_data = wav_encoder.encode(data, sample_rate);
        temp_wav.write_all(&wav_data).unwrap();
        
        // Use external FLAC encoder
        let output = Command::new("flac")
            .arg("--silent")
            .arg("--force")
            .arg("--stdout")
            .arg("--channels=1")
            .arg(format!("--sample-rate={}", sample_rate))
            .arg(temp_wav.path())
            .output()
            .expect("Failed to execute FLAC encoder");

        if !output.status.success() {
            panic!("FLAC encoding failed: {}", String::from_utf8_lossy(&output.stderr));
        }

        info!("Encoded {} samples into {} bytes of FLAC data", data.len(), output.stdout.len());
        output.stdout
    }
}

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
