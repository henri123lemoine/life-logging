use tracing::info;
use std::process::Command;
use std::io::Write;
use tempfile::NamedTempFile;
use opus::{Channels, Application, Bitrate};
use crate::error::{LifeLoggingError, Result};

use std::collections::HashMap;

pub trait AudioEncoder: Send + Sync {
    fn encode(&self, data: &[f32], sample_rate: u32) -> Result<Vec<u8>>;
    fn mime_type(&self) -> &'static str;
    fn file_extension(&self) -> &'static str;
    fn content_disposition(&self) -> &'static str;
}

pub struct PcmEncoder;

impl AudioEncoder for PcmEncoder {
    fn encode(&self, data: &[f32], _sample_rate: u32) -> Result<Vec<u8>> {
        let byte_data: Vec<u8> = data.iter()
            .flat_map(|&sample| sample.to_le_bytes().to_vec())
            .collect();
        Ok(byte_data)
    }

    fn mime_type(&self) -> &'static str {
        "audio/pcm"
    }

    fn file_extension(&self) -> &'static str {
        "pcm"
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
            buffer.write_all(&value.to_le_bytes())
                .map_err(|e| LifeLoggingError::EncodingError(format!("Failed to write WAV data: {}", e)))?;
        }

        info!("Encoded {} samples into {} bytes of WAV data", data.len(), buffer.len());
        Ok(buffer)
    }

    fn mime_type(&self) -> &'static str {
        "audio/wav"
    }

    fn file_extension(&self) -> &'static str {
        "wav"
    }

    fn content_disposition(&self) -> &'static str {
        "attachment; filename=\"audio.wav\""
    }
}

pub struct FlacEncoder;

impl AudioEncoder for FlacEncoder {
    fn encode(&self, data: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
        // Create a temporary WAV file
        let temp_wav = NamedTempFile::new().map_err(|e| LifeLoggingError::EncodingError(e.to_string()))?;
        let wav_encoder = WavEncoder;
        let wav_data = wav_encoder.encode(data, sample_rate)?;
        temp_wav.as_file().write_all(&wav_data).map_err(|e| LifeLoggingError::EncodingError(e.to_string()))?;
        
        // Use external FLAC encoder
        let output = Command::new("flac")
            .arg("--silent")
            .arg("--force")
            .arg("--stdout")
            .arg(temp_wav.path())
            .output()
            .map_err(|e| LifeLoggingError::EncodingError(format!("Failed to execute FLAC encoder: {}", e)))?;

        if !output.status.success() {
            return Err(LifeLoggingError::EncodingError(format!("FLAC encoding failed: {}", String::from_utf8_lossy(&output.stderr))));
        }

        info!("Encoded {} samples into {} bytes of FLAC data", data.len(), output.stdout.len());
        Ok(output.stdout)
    }

    fn mime_type(&self) -> &'static str {
        "audio/wav"
    }

    fn file_extension(&self) -> &'static str {
        "wav"
    }

    fn content_disposition(&self) -> &'static str {
        "attachment; filename=\"audio.flac\""
    }
}

pub struct OpusEncoder;

impl AudioEncoder for OpusEncoder {
    fn encode(&self, data: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
        // Configure the Opus encoder
        let mut encoder = opus::Encoder::new(
            sample_rate,
            Channels::Mono,
            Application::Audio
        ).map_err(|e| LifeLoggingError::EncodingError(format!("Failed to create Opus encoder: {}", e)))?;

        // Set the bitrate to 32 kbps
        encoder.set_bitrate(Bitrate::Bits(32000))
            .map_err(|e| LifeLoggingError::EncodingError(format!("Failed to set Opus bitrate: {}", e)))?;

        // Opus works with 20ms frames, so we need to calculate the frame size
        let frame_size = (sample_rate as usize / 1000) * 20;
        
        // Prepare the output buffer
        // The maximum size of an Opus packet for this configuration
        let max_packet_size = 1275; // This is the maximum for 48kHz stereo
        let mut output = Vec::new();

        // Encode the audio data in 20ms frames
        for chunk in data.chunks(frame_size) {
            let mut packet = vec![0u8; max_packet_size];
            let packet_len = encoder.encode_float(chunk, &mut packet)
                .map_err(|e| LifeLoggingError::EncodingError(format!("Failed to encode Opus frame: {}", e)))?;
            
            output.extend_from_slice(&(packet_len as u32).to_le_bytes());
            output.extend_from_slice(&packet[..packet_len]);
        }

        info!("Encoded {} samples into {} bytes of Opus data", data.len(), output.len());
        Ok(output)
    }

    fn mime_type(&self) -> &'static str {
        "audio/wav"
    }

    fn file_extension(&self) -> &'static str {
        "wav"
    }

    fn content_disposition(&self) -> &'static str {
        "attachment; filename=\"audio.opus\""
    }
}

pub struct EncoderFactory {
    encoders: HashMap<String, Box<dyn AudioEncoder>>,
}

impl EncoderFactory {
    pub fn new() -> Self {
        let mut encoders = HashMap::new();
        encoders.insert("pcm".to_string(), Box::new(PcmEncoder) as Box<dyn AudioEncoder>);
        encoders.insert("wav".to_string(), Box::new(WavEncoder) as Box<dyn AudioEncoder>);
        encoders.insert("flac".to_string(), Box::new(FlacEncoder) as Box<dyn AudioEncoder>);
        encoders.insert("opus".to_string(), Box::new(OpusEncoder) as Box<dyn AudioEncoder>);
        Self { encoders }
    }

    pub fn get_encoder(&self, format: &str) -> Option<&Box<dyn AudioEncoder>> {
        self.encoders.get(format)
    }
}
