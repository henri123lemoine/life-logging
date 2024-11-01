use crate::error::{AudioError, CodecError};
use crate::prelude::*;
use std::fmt::Debug;
use std::time::{Duration, Instant};

pub trait CodecImpl: Send + Sync + Debug {
    fn encode_samples(&self, data: &[f32], sample_rate: u32) -> Result<Vec<u8>>;
    fn decode_samples(&self, data: &[u8], sample_rate: u32) -> Result<Vec<f32>>;
}

pub trait Codec: CodecImpl {
    fn name(&self) -> &'static str;
    fn mime_type(&self) -> &'static str;
    fn extension(&self) -> &'static str;

    fn is_lossy(&self) -> bool;
    fn is_lossless(&self) -> bool;

    fn encode(&self, data: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
        self.encode_samples(data, sample_rate)
    }

    fn decode(&self, data: &[u8], sample_rate: u32) -> Result<Vec<f32>> {
        self.decode_samples(data, sample_rate)
    }

    fn compression_ratio(&self, data: &[f32], sample_rate: u32) -> Result<f32> {
        let encoded = self.encode(data, sample_rate)?;
        Ok(data.len() as f32 * std::mem::size_of::<f32>() as f32 / encoded.len() as f32)
    }

    fn measure_performance(&self, data: &[f32], sample_rate: u32) -> Result<CodecPerformance> {
        let start = Instant::now();
        let encoded = self.encode(data, sample_rate)?;
        let encode_duration = start.elapsed();

        let start = Instant::now();
        self.decode(&encoded, sample_rate)?;
        let decode_duration = start.elapsed();

        let audio_duration = Duration::from_secs_f32(data.len() as f32 / sample_rate as f32);

        Ok(CodecPerformance {
            encode_speed: audio_duration.as_secs_f32() / encode_duration.as_secs_f32(),
            decode_speed: audio_duration.as_secs_f32() / decode_duration.as_secs_f32(),
            compression_ratio: self.compression_ratio(data, sample_rate)?,
        })
    }

    fn content_disposition(&self) -> String {
        format!("attachment; filename=\"audio.{}\"", self.extension())
    }
}

pub trait LosslessCodec: Codec {}

pub trait LossyCodec: Codec {
    fn target_bitrate(&self) -> Option<u32> {
        None
    }

    fn quality_metrics(&self, original: &[f32], sample_rate: u32) -> Result<QualityMetrics> {
        let encoded = self.encode(original, sample_rate)?;
        let decoded = self.decode(&encoded, sample_rate)?;
        QualityMetrics::calculate(original, &decoded, sample_rate)
    }
}

#[derive(Debug, Clone)]
pub struct CodecPerformance {
    /// Encoding speed as a multiple of real-time (>1.0 means faster than real-time)
    pub encode_speed: f32,
    /// Decoding speed as a multiple of real-time (>1.0 means faster than real-time)
    pub decode_speed: f32,
    /// Compression ratio (original size / compressed size)
    pub compression_ratio: f32,
}

#[derive(Debug, Clone)]
pub struct QualityMetrics {
    /// Signal-to-Noise Ratio in dB
    pub snr: f32,
    /// Mean Squared Error
    pub mse: f32,
    /// Peak Signal-to-Noise Ratio in dB
    pub psnr: f32,
}

impl QualityMetrics {
    pub fn calculate(original: &[f32], decoded: &[f32], _sample_rate: u32) -> Result<Self> {
        if original.len() != decoded.len() {
            return Err(Error::Audio(AudioError::Codec(CodecError::InvalidData(
                "Length mismatch between original and decoded audio",
            ))));
        }

        let mse = original
            .iter()
            .zip(decoded.iter())
            .map(|(&o, &d)| (o - d).powi(2))
            .sum::<f32>()
            / original.len() as f32;

        // Avoid division by zero or log of zero
        if mse == 0.0 {
            return Ok(Self {
                snr: f32::INFINITY,
                mse: 0.0,
                psnr: f32::INFINITY,
            });
        }

        let signal_power = original.iter().map(|x| x.powi(2)).sum::<f32>() / original.len() as f32;
        let snr = if signal_power > 0.0 {
            10.0 * (signal_power / mse).log10()
        } else {
            f32::NEG_INFINITY
        };

        let max_value = original
            .iter()
            .map(|&x| x.abs())
            .fold(f32::NEG_INFINITY, f32::max)
            .max(1e-10); // Avoid log of zero
        let psnr = 20.0 * max_value.log10() - 10.0 * mse.log10();

        Ok(Self { snr, mse, psnr })
    }
}
