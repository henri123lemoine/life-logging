use crate::error::CodecError;
use crate::prelude::*;
use candle_core::{DType, Device, Tensor};
use moshi::encodec::{Config, Encodec};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use tempfile::NamedTempFile;
use tracing::{error, info};

pub trait AudioEncoder: Send + Sync {
    fn encode(&self, data: &[f32], sample_rate: u32) -> Result<Vec<u8>>;
    fn decode(&self, data: &[u8], sample_rate: u32) -> Result<Vec<f32>>;
    fn mime_type(&self) -> &'static str;
    fn content_disposition(&self) -> &'static str;
}

pub struct PcmEncoder;

impl AudioEncoder for PcmEncoder {
    fn encode(&self, data: &[f32], _sample_rate: u32) -> Result<Vec<u8>> {
        let byte_data: Vec<u8> = data
            .iter()
            .flat_map(|&sample| sample.to_le_bytes().to_vec())
            .collect();
        Ok(byte_data)
    }

    fn decode(&self, data: &[u8], _sample_rate: u32) -> Result<Vec<f32>> {
        todo!()
    }

    fn mime_type(&self) -> &'static str {
        "audio/pcm"
    }

    fn content_disposition(&self) -> &'static str {
        "attachment; filename=\"audio.pcm\""
    }
}

pub struct WavEncoder;

impl AudioEncoder for WavEncoder {
    fn encode(&self, data: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
        let channels = 1u16;
        let bits_per_sample = 16u16;
        let byte_rate = sample_rate * u32::from(channels) * u32::from(bits_per_sample) / 8;
        let block_align = channels * bits_per_sample / 8;
        let data_size = (data.len() * 2) as u32; // 2 bytes per sample for 16-bit
        let file_size = 36 + data_size;

        let mut buffer = Vec::with_capacity(file_size as usize + 8);

        // RIFF header
        buffer.extend_from_slice(b"RIFF");
        buffer.extend_from_slice(&file_size.to_le_bytes());
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
            buffer
                .write_all(&value.to_le_bytes())
                .map_err(|e: std::io::Error| {
                    CodecError::Encoding(format!("Failed to write WAV data: {}", e))
                });
        }

        info!(
            "Encoded {} samples into {} bytes of WAV data",
            data.len(),
            buffer.len()
        );
        Ok(buffer)
    }

    fn decode(&self, data: &[u8], _sample_rate: u32) -> Result<Vec<f32>> {
        todo!()
    }

    fn mime_type(&self) -> &'static str {
        "audio/wav"
    }

    fn content_disposition(&self) -> &'static str {
        "attachment; filename=\"audio.wav\""
    }
}

pub struct FlacEncoder;

impl AudioEncoder for FlacEncoder {
    fn encode(&self, data: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
        // Create a temporary WAV file
        let temp_wav = NamedTempFile::new().map_err(|e| CodecError::Encoding(e.to_string()));
        let wav_encoder = WavEncoder;
        let wav_data = wav_encoder.encode(data, sample_rate)?;
        temp_wav
            .as_file()
            .write_all(&wav_data)
            .map_err(|e| CodecError::Encoding(e.to_string()));

        // Use external FLAC encoder
        let output = Command::new("flac")
            .arg("--silent")
            .arg("--force")
            .arg("--stdout")
            .arg(temp_wav.path())
            .output()
            .map_err(|e| {
                error!("Failed to execute FLAC encoder: {}", e);
                CodecError::Encoding(format!("Failed to execute FLAC encoder: {}", e))
            });

        if !output.status.success() {
            let error_message = String::from_utf8_lossy(&output.stderr);
            error!("FLAC encoding failed: {}", error_message);
            return Err(CodecError::Encoding(format!(
                "FLAC encoding failed: {}",
                error_message
            )));
        }

        info!(
            "Encoded {} samples into {} bytes of FLAC data",
            data.len(),
            output.stdout.len()
        );
        Ok(output.stdout)
    }

    fn decode(&self, data: &[u8], _sample_rate: u32) -> Result<Vec<f32>> {
        todo!()
    }

    fn mime_type(&self) -> &'static str {
        "audio/flac"
    }

    fn content_disposition(&self) -> &'static str {
        "attachment; filename=\"audio.flac\""
    }
}

pub struct OpusEncoder {
    bitrate: u32,
}

impl OpusEncoder {
    pub fn new(bitrate: u32) -> Self {
        OpusEncoder { bitrate }
    }
}

impl AudioEncoder for OpusEncoder {
    fn encode(&self, data: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
        // Create a temporary WAV file
        let mut temp_wav: NamedTempFile = NamedTempFile::new()
            .map_err(|e| CodecError::Encoding(format!("Failed to create temp WAV file: {}", e)))
            .into()?;

        // Write WAV data to the temporary file
        let wav_encoder = WavEncoder;
        let wav_data = wav_encoder.encode(data, sample_rate)?;
        temp_wav
            .write_all(&wav_data)
            .map_err(|e| CodecError::Encoding(format!("Failed to write WAV data: {}", e)))?;

        // Use FFmpeg to convert WAV to Opus
        let output = Command::new("ffmpeg")
            .arg("-i")
            .arg(temp_wav.path())
            .arg("-c:a")
            .arg("libopus")
            .arg("-b:a")
            .arg(format!("{}k", self.bitrate))
            .arg("-f")
            .arg("opus")
            .arg("-")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| AudioError::Encoding(format!("Failed to execute FFmpeg: {}", e)))?;

        if !output.status.success() {
            let error_message = String::from_utf8_lossy(&output.stderr);
            return Err(
                AudioError::Encoding(format!("FFmpeg encoding failed: {}", error_message)).into(),
            );
        }

        info!(
            "Encoded {} samples into {} bytes of Opus data at {}kbps",
            data.len(),
            output.stdout.len(),
            self.bitrate
        );
        Ok(output.stdout)
    }

    fn decode(&self, data: &[u8], _sample_rate: u32) -> Result<Vec<f32>> {
        todo!()
    }

    fn mime_type(&self) -> &'static str {
        "audio/ogg"
    }

    fn content_disposition(&self) -> &'static str {
        "attachment; filename=\"audio.opus\""
    }
}

pub struct MoshiEncoder {
    model: Arc<Mutex<Encodec>>,
    device: Device,
}

impl MoshiEncoder {
    pub fn new() -> Result<Self> {
        let device = Device::Cpu;
        let config = Config::v0_1(None); // Use default number of codebooks
        let vb = candle_nn::VarBuilder::zeros(candle_core::DType::F32, &device);
        let model = Encodec::new(config, vb).map_err(|e| {
            AudioError::Encoding(format!("Failed to create Moshi Encodec model: {}", e))
        })?;

        Ok(Self {
            model: Arc::new(Mutex::new(model)),
            device,
        })
    }
}

impl AudioEncoder for MoshiEncoder {
    fn encode(&self, data: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
        if sample_rate != 24000 {
            return Err(AudioError::Encoding("Moshi requires 24kHz audio".into()).into());
        }

        let input_tensor = Tensor::from_slice(data, (1, 1, data.len()), &self.device)
            .map_err(|e| AudioError::Encoding(format!("Failed to create input tensor: {}", e)))?;

        let encoded = self
            .model
            .lock()
            .map_err(|e| {
                AudioError::Encoding(format!("Failed to acquire lock on Moshi model: {}", e))
            })?
            .encode(&input_tensor)
            .map_err(|e| AudioError::Encoding(format!("Moshi encoding failed: {}", e)))?;

        let encoded_bytes = encoded
            .to_dtype(DType::U8)
            .map_err(|e| AudioError::Encoding(format!("Failed to convert tensor to U8: {}", e)))?
            .flatten_all()
            .map_err(|e| AudioError::Encoding(format!("Failed to flatten tensor: {}", e)))?
            .to_vec1::<u8>()
            .map_err(|e| {
                AudioError::Encoding(format!("Failed to convert Moshi encoding to bytes: {}", e))
            })?;

        Ok(encoded_bytes)
    }

    fn decode(&self, data: &[u8], sample_rate: u32) -> Result<Vec<f32>> {
        if sample_rate != 24000 {
            return Err(AudioError::Encoding("Moshi requires 24kHz audio".into()).into());
        }

        let encoded_tensor = Tensor::from_slice(data, (1, data.len()), &self.device)
            .map_err(|e| AudioError::Encoding(format!("Failed to create input tensor: {}", e)))?;

        let encoded_tensor = encoded_tensor
            .unsqueeze(1)
            .map_err(|e| AudioError::Encoding(format!("Failed to unsqueeze tensor: {}", e)))?;

        let decoded = self
            .model
            .lock()
            .map_err(|e| {
                AudioError::Encoding(format!("Failed to acquire lock on Moshi model: {}", e))
            })?
            .decode(&encoded_tensor)
            .map_err(|e| AudioError::Encoding(format!("Moshi decoding failed: {}", e)))?;

        let decoded_samples = decoded
            .flatten_all()
            .map_err(|e| AudioError::Encoding(format!("Failed to flatten tensor: {}", e)))?
            .to_vec1::<f32>()
            .map_err(|e| {
                AudioError::Encoding(format!("Failed to convert Moshi decoding to vec: {}", e))
            })?;

        Ok(decoded_samples)
    }

    fn mime_type(&self) -> &'static str {
        "application/x-moshi"
    }

    fn content_disposition(&self) -> &'static str {
        "attachment; filename=\"audio.moshi\""
    }
}

pub struct EncoderFactory {
    encoders: HashMap<String, Box<dyn AudioEncoder>>,
}

impl Default for EncoderFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl EncoderFactory {
    pub fn new() -> Self {
        let mut encoders = HashMap::new();
        encoders.insert(
            "pcm".to_string(),
            Box::new(PcmEncoder) as Box<dyn AudioEncoder>,
        );
        encoders.insert(
            "wav".to_string(),
            Box::new(WavEncoder) as Box<dyn AudioEncoder>,
        );
        encoders.insert(
            "flac".to_string(),
            Box::new(FlacEncoder) as Box<dyn AudioEncoder>,
        );
        encoders.insert(
            "opus".to_string(), // Default to 32kbps
            Box::new(OpusEncoder::new(32)) as Box<dyn AudioEncoder>,
        );
        encoders.insert(
            "opus32".to_string(),
            Box::new(OpusEncoder::new(32)) as Box<dyn AudioEncoder>,
        );
        encoders.insert(
            "opus64".to_string(),
            Box::new(OpusEncoder::new(64)) as Box<dyn AudioEncoder>,
        );
        encoders.insert(
            "moshi".to_string(),
            Box::new(MoshiEncoder::new().expect("Failed to initialize MoshiEncoder"))
                as Box<dyn AudioEncoder>,
        );
        EncoderFactory { encoders }
    }

    pub fn get_encoder(&self, format: &str) -> Option<&dyn AudioEncoder> {
        self.encoders.get(format).map(|boxed| boxed.as_ref())
    }
}

pub static ENCODER_FACTORY: Lazy<EncoderFactory> = Lazy::new(EncoderFactory::default);
