use crate::error::{AudioError, Result};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;
use tracing::{error, info};

pub trait AudioEncoder: Send + Sync {
    fn encode(&self, data: &[f32], sample_rate: u32) -> Result<Vec<u8>>;
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
                    AudioError::Encoding(format!("Failed to write WAV data: {}", e))
                })?;
        }

        info!(
            "Encoded {} samples into {} bytes of WAV data",
            data.len(),
            buffer.len()
        );
        Ok(buffer)
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
        let temp_wav = NamedTempFile::new().map_err(|e| AudioError::Encoding(e.to_string()))?;
        let wav_encoder = WavEncoder;
        let wav_data = wav_encoder.encode(data, sample_rate)?;
        temp_wav
            .as_file()
            .write_all(&wav_data)
            .map_err(|e| AudioError::Encoding(e.to_string()))?;

        // Use external FLAC encoder
        let output = Command::new("flac")
            .arg("--silent")
            .arg("--force")
            .arg("--stdout")
            .arg(temp_wav.path())
            .output()
            .map_err(|e| {
                error!("Failed to execute FLAC encoder: {}", e);
                AudioError::Encoding(format!("Failed to execute FLAC encoder: {}", e))
            })?;

        if !output.status.success() {
            let error_message = String::from_utf8_lossy(&output.stderr);
            error!("FLAC encoding failed: {}", error_message);
            return Err(
                AudioError::Encoding(format!("FLAC encoding failed: {}", error_message)).into(),
            );
        }

        info!(
            "Encoded {} samples into {} bytes of FLAC data",
            data.len(),
            output.stdout.len()
        );
        Ok(output.stdout)
    }

    fn mime_type(&self) -> &'static str {
        "audio/flac"
    }

    fn content_disposition(&self) -> &'static str {
        "attachment; filename=\"audio.flac\""
    }
}

pub struct OpusEncoder;

impl AudioEncoder for OpusEncoder {
    fn encode(&self, data: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
        // Create a temporary WAV file
        let mut temp_wav = NamedTempFile::new()
            .map_err(|e| AudioError::Encoding(format!("Failed to create temp WAV file: {}", e)))?;

        // Write WAV data to the temporary file
        let wav_encoder = WavEncoder;
        let wav_data = wav_encoder.encode(data, sample_rate)?;
        temp_wav
            .write_all(&wav_data)
            .map_err(|e| AudioError::Encoding(format!("Failed to write WAV data: {}", e)))?;

        // Use FFmpeg to convert WAV to Opus
        let output = Command::new("ffmpeg")
            .arg("-i")
            .arg(temp_wav.path())
            .arg("-c:a")
            .arg("libopus")
            .arg("-b:a")
            .arg("32k") // 32 kbps
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
            "Encoded {} samples into {} bytes of Opus data",
            data.len(),
            output.stdout.len()
        );
        Ok(output.stdout)
    }

    fn mime_type(&self) -> &'static str {
        "audio/ogg"
    }

    fn content_disposition(&self) -> &'static str {
        "attachment; filename=\"audio.opus\""
    }
}

pub static ENCODER_FACTORY: Lazy<EncoderFactory> = Lazy::new(|| {
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
        "opus".to_string(),
        Box::new(OpusEncoder) as Box<dyn AudioEncoder>,
    );
    EncoderFactory { encoders }
});

pub struct EncoderFactory {
    encoders: HashMap<String, Box<dyn AudioEncoder>>,
}

impl EncoderFactory {
    pub fn get_encoder(&self, format: &str) -> Option<&dyn AudioEncoder> {
        self.encoders.get(format).map(|boxed| boxed.as_ref())
    }
}
