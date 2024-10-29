use crate::audio::codec::traits::{Codec, CodecImpl};
use crate::audio::codec::wav::WavCodec;
use crate::error::CodecError;
use crate::prelude::*;
use codec_derive::Codec;
use std::io::Write;
use std::process::Command;
use tempfile::NamedTempFile;
use tracing::{error, info};

#[derive(Debug, Codec)]
#[codec(name = "OPUS", mime = "audio/opus", extension = "opus", lossy)]
pub struct OpusCodec {
    bitrate: u32,
    sample_rate: u32,
}

impl Default for OpusCodec {
    fn default() -> Self {
        Self {
            bitrate: 32,
            sample_rate: 48000, // Opus works best with 48kHz
        }
    }
}

impl OpusCodec {
    pub fn new(bitrate: u32) -> Self {
        Self {
            bitrate,
            sample_rate: 48000,
        }
    }

    fn resample(&self, data: &[f32], input_rate: u32) -> Vec<f32> {
        if input_rate == self.sample_rate {
            return data.to_vec();
        }

        let ratio = self.sample_rate as f32 / input_rate as f32;
        let new_len = (data.len() as f32 * ratio) as usize;
        let mut resampled = Vec::with_capacity(new_len);

        for i in 0..new_len {
            let src_idx = i as f32 / ratio;
            let src_idx_floor = src_idx.floor() as usize;
            let src_idx_ceil = (src_idx_floor + 1).min(data.len() - 1);
            let frac = src_idx - src_idx.floor();

            let sample = data[src_idx_floor] * (1.0 - frac) + data[src_idx_ceil] * frac;
            resampled.push(sample);
        }

        resampled
    }
}

impl CodecImpl for OpusCodec {
    fn encode_samples(&self, data: &[f32], input_rate: u32) -> Result<Vec<u8>> {
        // First resample to 48kHz if needed
        let resampled_data = self.resample(data, input_rate);

        // Create temporary WAV file
        let mut temp_wav = NamedTempFile::new()
            .map_err(|e| CodecError::Encoding(format!("Failed to create temp WAV file: {}", e)))?;

        let wav_data = WavCodec::default().encode(&resampled_data, self.sample_rate)?;
        temp_wav.write_all(&wav_data)?;

        // Use FFmpeg with better quality settings
        let output = Command::new("ffmpeg")
            .arg("-i")
            .arg(temp_wav.path())
            .arg("-c:a")
            .arg("libopus")
            .arg("-b:a")
            .arg(format!("{}k", self.bitrate))
            .arg("-compression_level")
            .arg("10") // Maximum quality
            .arg("-frame_duration")
            .arg("20") // Lower latency
            .arg("-application")
            .arg("audio") // Optimize for audio quality
            .arg("-ar")
            .arg(self.sample_rate.to_string())
            .arg("-ac")
            .arg("1")
            .arg("-f")
            .arg("opus")
            .arg("-")
            .output()?;

        if !output.status.success() {
            let error_message = String::from_utf8_lossy(&output.stderr);
            error!("FFmpeg encoding failed: {}", error_message);
            return Err(
                CodecError::Encoding(format!("FFmpeg encoding failed: {}", error_message)).into(),
            );
        }

        Ok(output.stdout)
    }

    fn decode_samples(&self, data: &[u8], output_rate: u32) -> Result<Vec<f32>> {
        // Create temporary Opus file
        let mut temp_opus = NamedTempFile::new()
            .map_err(|e| CodecError::Decoding(format!("Failed to create temp file: {}", e)))?;

        temp_opus
            .write_all(data)
            .map_err(|e| CodecError::Decoding(format!("Failed to write encoded data: {}", e)))?;

        // Use FFmpeg to decode to WAV
        let output = Command::new("ffmpeg")
            .arg("-i")
            .arg(temp_opus.path())
            .arg("-ar")
            .arg(self.sample_rate.to_string()) // First decode to 48kHz
            .arg("-ac")
            .arg("1")
            .arg("-f")
            .arg("wav")
            .arg("-")
            .output()
            .map_err(|e| CodecError::Decoding(format!("Failed to execute FFmpeg: {}", e)))?;

        if !output.status.success() {
            let error_message = String::from_utf8_lossy(&output.stderr);
            error!("FFmpeg decoding failed: {}", error_message);
            return Err(
                CodecError::Decoding(format!("FFmpeg decoding failed: {}", error_message)).into(),
            );
        }

        // Decode WAV data
        let decoded = WavCodec::default().decode(&output.stdout, self.sample_rate)?;

        // Resample to target rate if needed
        let resampled = self.resample(&decoded, output_rate);

        info!("Decoded Opus data to {} samples", resampled.len());
        Ok(resampled)
    }
}
