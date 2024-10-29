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
#[codec(name = "FLAC", mime = "audio/flac", extension = "flac", lossless)]
pub struct FlacCodec;

impl Default for FlacCodec {
    fn default() -> Self {
        Self
    }
}

impl CodecImpl for FlacCodec {
    fn encode_samples(&self, data: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
        // First convert to WAV
        let wav_data = WavCodec::default().encode(data, sample_rate)?;
        let mut temp_wav = NamedTempFile::new()
            .map_err(|e| CodecError::Encoding(format!("Failed to create temp WAV file: {}", e)))?;
        temp_wav
            .write_all(&wav_data)
            .map_err(|e| CodecError::Encoding(format!("Failed to write WAV data: {}", e)))?;

        // Use FLAC encoder
        let output = Command::new("flac")
            .arg("--silent")
            .arg("--force")
            .arg("--stdout")
            .arg(temp_wav.path())
            .output()
            .map_err(|e| {
                error!("Failed to execute FLAC encoder: {}", e);
                CodecError::Encoding(format!("Failed to execute FLAC encoder: {}", e))
            })?;

        if !output.status.success() {
            let error_message = String::from_utf8_lossy(&output.stderr);
            error!("FLAC encoding failed: {}", error_message);
            return Err(
                CodecError::Encoding(format!("FLAC encoding failed: {}", error_message)).into(),
            );
        }

        info!(
            "Encoded {} samples into {} bytes of FLAC data",
            data.len(),
            output.stdout.len()
        );
        Ok(output.stdout)
    }

    fn decode_samples(&self, data: &[u8], sample_rate: u32) -> Result<Vec<f32>> {
        let mut temp_flac = NamedTempFile::new()
            .map_err(|e| CodecError::Decoding(format!("Failed to create temp FLAC file: {}", e)))?;
        temp_flac
            .write_all(data)
            .map_err(|e| CodecError::Decoding(format!("Failed to write FLAC data: {}", e)))?;

        let output = Command::new("flac")
            .arg("--decode")
            .arg("--stdout")
            .arg(temp_flac.path())
            .output()
            .map_err(|e| CodecError::Decoding(format!("Failed to execute FLAC decoder: {}", e)))?;

        if !output.status.success() {
            let error_message = String::from_utf8_lossy(&output.stderr);
            return Err(
                CodecError::Decoding(format!("FLAC decoding failed: {}", error_message)).into(),
            );
        }

        // Decode the WAV data
        WavCodec::default().decode(&output.stdout, sample_rate)
    }
}
