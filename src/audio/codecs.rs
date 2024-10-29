use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::{fmt::Debug, time::Duration, time::Instant};

use candle_core::{DType, Device, Tensor};
use once_cell::sync::Lazy;
use std::any::Any;
use std::collections::HashMap;
use tempfile::NamedTempFile;
use tracing::{error, info};

use crate::error::{AudioError, CodecError};
use crate::prelude::*;

macro_rules! impl_codec_basics {
    ($type:ty, $name:expr, $mime:expr, $ext:expr) => {
        impl Codec for $type {
            fn name(&self) -> &'static str {
                $name
            }
            fn mime_type(&self) -> &'static str {
                $mime
            }
            fn extension(&self) -> &'static str {
                $ext
            }
        }
    };
}

pub trait Codec: Send + Sync + Debug + Any {
    fn name(&self) -> &'static str;
    fn mime_type(&self) -> &'static str;
    fn extension(&self) -> &'static str;

    fn is_lossy(&self) -> bool {
        false
    }

    fn is_lossless(&self) -> bool {
        false
    }

    fn encode(&self, data: &[f32], sample_rate: u32) -> Result<Vec<u8>>;
    fn decode(&self, data: &[u8], sample_rate: u32) -> Result<Vec<f32>>;

    fn compression_ratio(&self, data: &[f32], sample_rate: u32) -> Result<f32> {
        let encoded = self.encode(data, sample_rate)?;
        Ok(data.len() as f32 * std::mem::size_of::<f32>() as f32 / encoded.len() as f32)
    }

    fn encoding_speed(&self, data: &[f32], sample_rate: u32) -> Result<EncodingMetrics> {
        let start = Instant::now();
        let encoded = self.encode(data, sample_rate)?;
        let encode_duration = start.elapsed();

        let start = Instant::now();
        self.decode(&encoded, sample_rate)?;
        let decode_duration = start.elapsed();

        let duration_secs = data.len() as f32 / sample_rate as f32;

        Ok(EncodingMetrics {
            encode_speed: duration_secs / encode_duration.as_secs_f32(),
            decode_speed: duration_secs / decode_duration.as_secs_f32(),
            compression_ratio: data.len() as f32 * 4.0 / encoded.len() as f32,
        })
    }

    fn encoding_metrics(&self, data: &[f32], sample_rate: u32) -> Result<EncodingMetrics> {
        let start = Instant::now();
        let encoded = self.encode(data, sample_rate)?;
        let encode_duration = start.elapsed();

        let start = Instant::now();
        self.decode(&encoded, sample_rate)?;
        let decode_duration = start.elapsed();

        let duration_secs = data.len() as f32 / sample_rate as f32;

        Ok(EncodingMetrics {
            encode_speed: duration_secs / encode_duration.as_secs_f32(),
            decode_speed: duration_secs / decode_duration.as_secs_f32(),
            compression_ratio: (data.len() * std::mem::size_of::<f32>()) as f32
                / encoded.len() as f32,
        })
    }
}

pub trait LosslessCodec: Codec {}

pub trait LossyCodec: Codec {
    fn quality_metrics(&self, original: &[f32], sample_rate: u32) -> Result<QualityMetrics> {
        let encoded = self.encode(original, sample_rate)?;
        let decoded = self.decode(&encoded, sample_rate)?;
        Ok(QualityMetrics::calculate(original, &decoded, sample_rate))
    }

    fn target_bitrate(&self) -> Option<u32> {
        None
    }
}

#[derive(Debug, Clone)]
pub struct WavCodec {
    bits_per_sample: u16, // Usually 16 or 32
}

impl WavCodec {
    pub fn new(bits_per_sample: u16) -> Result<Self> {
        match bits_per_sample {
            16 | 32 => Ok(Self { bits_per_sample }),
            _ => Err(Error::Audio(AudioError::Codec(
                CodecError::InvalidConfiguration("WAV only supports 16 or 32 bits per sample"),
            ))),
        }
    }

    fn is_lossless(&self) -> bool {
        true
    }

    fn write_header(&self, num_samples: usize, sample_rate: u32) -> Vec<u8> {
        let channels = 1u16;
        let bytes_per_sample = self.bits_per_sample / 8;
        let byte_rate = sample_rate * u32::from(channels * bytes_per_sample);
        let block_align = channels * bytes_per_sample;
        let data_size = (num_samples * bytes_per_sample as usize) as u32;
        let file_size = 36 + data_size;

        let mut header = Vec::with_capacity(44);
        header.extend_from_slice(b"RIFF");
        header.extend_from_slice(&file_size.to_le_bytes());
        header.extend_from_slice(b"WAVE");

        // Format chunk
        header.extend_from_slice(b"fmt ");
        header.extend_from_slice(&16u32.to_le_bytes()); // Chunk size
        header.extend_from_slice(&1u16.to_le_bytes()); // Audio format (PCM)
        header.extend_from_slice(&channels.to_le_bytes());
        header.extend_from_slice(&sample_rate.to_le_bytes());
        header.extend_from_slice(&byte_rate.to_le_bytes());
        header.extend_from_slice(&block_align.to_le_bytes());
        header.extend_from_slice(&self.bits_per_sample.to_le_bytes());

        // Data chunk header
        header.extend_from_slice(b"data");
        header.extend_from_slice(&data_size.to_le_bytes());

        header
    }

    fn encode_sample(&self, sample: f32) -> Vec<u8> {
        match self.bits_per_sample {
            16 => {
                let value = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
                value.to_le_bytes().to_vec()
            }
            32 => {
                let value = (sample.clamp(-1.0, 1.0) * 2147483647.0) as i32;
                value.to_le_bytes().to_vec()
            }
            _ => unreachable!("bits_per_sample already validated"),
        }
    }

    fn decode_sample(&self, bytes: &[u8]) -> Result<f32> {
        match self.bits_per_sample {
            16 => {
                let value = i16::from_le_bytes(
                    bytes
                        .try_into()
                        .map_err(|_| CodecError::InvalidData("Invalid sample data"))?,
                );
                Ok(value as f32 / 32767.0)
            }
            32 => {
                let value = i32::from_le_bytes(
                    bytes
                        .try_into()
                        .map_err(|_| CodecError::InvalidData("Invalid sample data"))?,
                );
                Ok(value as f32 / 2147483647.0)
            }
            _ => unreachable!("bits_per_sample already validated"),
        }
    }
}

impl Codec for WavCodec {
    fn name(&self) -> &'static str {
        "WAV"
    }
    fn mime_type(&self) -> &'static str {
        "audio/wav"
    }
    fn extension(&self) -> &'static str {
        "wav"
    }

    fn encode(&self, data: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
        let mut buffer = self.write_header(data.len(), sample_rate);

        // Pre-allocate space for audio data
        let bytes_per_sample = self.bits_per_sample as usize / 8;
        buffer.reserve(data.len() * bytes_per_sample);

        // Write audio data
        for &sample in data {
            buffer.extend(self.encode_sample(sample));
        }

        Ok(buffer)
    }

    fn decode(&self, data: &[u8], sample_rate: u32) -> Result<Vec<f32>> {
        if data.len() < 44 {
            return Err(Error::Audio(AudioError::Codec(CodecError::InvalidData(
                "WAV header too short",
            ))));
        }

        // Verify RIFF header
        if &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
            return Err(Error::Audio(AudioError::Codec(CodecError::InvalidData(
                "Invalid WAV header",
            ))));
        }

        // Find data chunk
        let mut offset = 12;
        let mut data_offset = None;

        while offset + 8 <= data.len() {
            let chunk_id = &data[offset..offset + 4];
            let chunk_size = u32::from_le_bytes(
                data[offset + 4..offset + 8]
                    .try_into()
                    .map_err(|_| CodecError::InvalidData("Invalid chunk size"))?,
            );

            if chunk_id == b"data" {
                data_offset = Some(offset + 8);
                break;
            }

            offset += 8 + chunk_size as usize;
        }

        let data_offset = data_offset.ok_or(CodecError::InvalidData("No data chunk found"))?;
        let bytes_per_sample = self.bits_per_sample as usize / 8;

        let mut samples = Vec::new();
        for chunk in data[data_offset..].chunks_exact(bytes_per_sample) {
            samples.push(self.decode_sample(chunk)?);
        }

        Ok(samples)
    }
}

impl LosslessCodec for WavCodec {}

impl Default for WavCodec {
    fn default() -> Self {
        Self {
            bits_per_sample: 16,
        }
    }
}

impl_codec_basics!(WavCodec, "WAV", "audio/wav", "wav");

/// Codec Registry

#[derive(Debug, Default)]
pub struct CodecRegistry {
    codecs: HashMap<String, Box<dyn Codec>>,
}

impl CodecRegistry {
    pub fn new() -> Self {
        Self {
            codecs: HashMap::new(),
        }
    }

    pub fn register<C: Codec + 'static>(&mut self, codec: C) {
        let name = codec.name().to_string();
        self.codecs.insert(name, Box::new(codec));
    }

    pub fn get(&self, name: &str) -> Option<&dyn Codec> {
        self.codecs.get(name).map(|b| b.as_ref())
    }

    pub fn get_lossy(&self, name: &str) -> Option<&dyn Codec> {
        self.get(name).filter(|codec| codec.is_lossy())
    }

    pub fn get_lossless(&self, name: &str) -> Option<&dyn Codec> {
        self.get(name).filter(|codec| codec.is_lossless())
    }
}

#[derive(Debug, Clone)]
pub struct EncodingMetrics {
    pub encode_speed: f32,
    pub decode_speed: f32,
    pub compression_ratio: f32,
}

#[derive(Debug, Clone)]
pub struct QualityMetrics {
    pub snr: f32,
    pub mse: f32,
    pub psnr: f32,
    pub frequency_response: FrequencyResponse,
}

#[derive(Debug, Clone)]
pub struct FrequencyResponse {
    pub bands: Vec<f32>,
    pub relative_power: Vec<f32>,
}

impl QualityMetrics {
    pub fn calculate(original: &[f32], decoded: &[f32], sample_rate: u32) -> Self {
        let mse = calculate_mse(original, decoded);
        let snr = calculate_snr(original, decoded);
        let psnr = calculate_psnr(original, decoded);
        let frequency_response = analyze_frequency_response(original, decoded, sample_rate);

        Self {
            snr,
            mse,
            psnr,
            frequency_response,
        }
    }
}

fn calculate_mse(original: &[f32], decoded: &[f32]) -> f32 {
    assert_eq!(original.len(), decoded.len());
    original
        .iter()
        .zip(decoded.iter())
        .map(|(&o, &d)| (o - d).powi(2))
        .sum::<f32>()
        / original.len() as f32
}

fn calculate_snr(original: &[f32], decoded: &[f32]) -> f32 {
    let signal_power: f32 = original.iter().map(|x| x.powi(2)).sum::<f32>() / original.len() as f32;
    let noise_power = calculate_mse(original, decoded);
    10.0 * (signal_power / noise_power).log10()
}

fn calculate_psnr(original: &[f32], decoded: &[f32]) -> f32 {
    let max_value = original
        .iter()
        .map(|&x| x.abs())
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap();
    let mse = calculate_mse(original, decoded);
    20.0 * max_value.log10() - 10.0 * mse.log10()
}

fn analyze_frequency_response(
    original: &[f32],
    decoded: &[f32],
    sample_rate: u32,
) -> FrequencyResponse {
    // TODO: Currently using simplified version. Use proper FFT library
    let bands = vec![125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];
    let relative_power = bands.iter().map(|_| 1.0).collect();
    FrequencyResponse {
        bands,
        relative_power,
    }
}

// TESTING

#[derive(Debug, Clone)]
pub struct TestAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub duration: Duration,
    pub description: &'static str,
}

pub struct TestSuite {
    test_cases: Vec<TestAudio>,
}

impl TestSuite {
    pub fn new() -> Self {
        Self {
            test_cases: vec![
                Self::generate_sine_sweep(),
                Self::generate_white_noise(),
                Self::generate_impulses(),
            ],
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &TestAudio> {
        self.test_cases.iter()
    }

    fn generate_sine_sweep() -> TestAudio {
        let sample_rate = 48000;
        let duration = Duration::from_secs(5);
        let num_samples = (sample_rate as f32 * duration.as_secs_f32()) as usize;

        let mut samples = Vec::with_capacity(num_samples);
        for i in 0..num_samples {
            let t = i as f32 / sample_rate as f32;
            let freq = 20.0 * (1000.0f32).powf(t / duration.as_secs_f32());
            let phase = 2.0 * std::f32::consts::PI * freq * t;
            samples.push(phase.sin());
        }

        TestAudio {
            samples,
            sample_rate,
            duration,
            description: "Logarithmic sine sweep 20Hz-20kHz",
        }
    }

    fn generate_white_noise() -> TestAudio {
        let sample_rate = 48000;
        let duration = Duration::from_secs(5);
        let num_samples = (sample_rate as f32 * duration.as_secs_f32()) as usize;

        use rand::Rng;
        let mut rng = rand::thread_rng();
        let samples: Vec<f32> = (0..num_samples)
            .map(|_| rng.gen_range(-1.0..=1.0))
            .collect();

        TestAudio {
            samples,
            sample_rate,
            duration,
            description: "White noise",
        }
    }

    fn generate_impulses() -> TestAudio {
        let sample_rate = 48000;
        let duration = Duration::from_secs(1);
        let num_samples = (sample_rate as f32 * duration.as_secs_f32()) as usize;

        let mut samples = vec![0.0; num_samples];
        for i in 0..10 {
            let pos = i * sample_rate as usize / 10;
            if pos < samples.len() {
                samples[pos] = 1.0;
            }
        }

        TestAudio {
            samples,
            sample_rate,
            duration,
            description: "Impulse train",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub struct CodecTester {
        test_suite: TestSuite,
    }

    impl CodecTester {
        pub fn new() -> Self {
            Self {
                test_suite: TestSuite::new(),
            }
        }

        pub fn test_codec<C: Codec>(&self, codec: &C) -> Result<()> {
            info!("Testing codec: {}", codec.name());

            for test_case in &self.test_suite.test_cases {
                let metrics = self.test_single_case(codec, test_case)?;

                // Common assertions
                assert!(
                    metrics.encode_speed > 1.0,
                    "Codec {} is slower than real-time for {}",
                    codec.name(),
                    test_case.description
                );
            }
            Ok(())
        }

        pub fn test_lossless<C: LosslessCodec>(&self, codec: &C) -> Result<()> {
            self.test_codec(codec)?;

            for test_case in &self.test_suite.test_cases {
                let encoded = codec.encode(&test_case.samples, test_case.sample_rate)?;
                let decoded = codec.decode(&encoded, test_case.sample_rate)?;

                assert_eq!(
                    test_case.samples, decoded,
                    "Perfect reconstruction failed for {}",
                    test_case.description
                );
            }
            Ok(())
        }

        pub fn test_lossy<C: LossyCodec>(&self, codec: &C, minimum_snr: f32) -> Result<()> {
            self.test_codec(codec)?;

            for test_case in &self.test_suite.test_cases {
                let metrics = codec.quality_metrics(&test_case.samples, test_case.sample_rate)?;

                assert!(
                    metrics.snr >= minimum_snr,
                    "SNR {:.2}dB below minimum {:.2}dB for {}",
                    metrics.snr,
                    minimum_snr,
                    test_case.description
                );
            }
            Ok(())
        }

        fn test_single_case<C: Codec>(
            &self,
            codec: &C,
            test_case: &TestAudio,
        ) -> Result<EncodingMetrics> {
            let encoded = codec.encode(&test_case.samples, test_case.sample_rate)?;
            let decoded = codec.decode(&encoded, test_case.sample_rate)?;

            assert_eq!(
                test_case.samples.len(),
                decoded.len(),
                "Decoded length mismatch for {}",
                test_case.description
            );

            codec.encoding_metrics(&test_case.samples, test_case.sample_rate)
        }
    }

    mod codec_tests {
        use super::*;

        #[test]
        fn test_wav_codec() -> Result<()> {
            let tester = CodecTester::new();
            let codec = WavCodec::default();
            tester.test_lossless(&codec)
        }

        // #[test]
        // fn test_opus_codec() -> Result<()> {
        //     let tester = CodecTester::new();
        //     let codec = OpusCodec::new(64);
        //     tester.test_lossy(&codec, 20.0)
        // }
    }

    #[cfg(test)]
    mod benchmarks {
        use super::*;
        use criterion::{black_box, Criterion};

        pub fn benchmark_codec<C: Codec>(c: &mut Criterion, codec: &C) {
            let suite = TestSuite::new();

            let mut group = c.benchmark_group(codec.name());
            for test_case in suite.iter() {
                group.bench_function(test_case.description, |b| {
                    b.iter(|| {
                        codec.encode(
                            black_box(&test_case.samples),
                            black_box(test_case.sample_rate),
                        )
                    })
                });
            }
            group.finish();
        }
    }
}
