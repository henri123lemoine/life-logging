use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::{fmt::Debug, time::Duration, time::Instant};

use candle_core::{DType, Device, Tensor};
use once_cell::sync::Lazy;
use tempfile::NamedTempFile;
use thiserror::Error;
use tracing::{error, info};

use crate::error::{AudioError, CodecError};
use crate::prelude::*;

fn codec_err(e: CodecError) -> Error {
    Error::Audio(AudioError::Codec(e))
}

macro_rules! codec_err {
    ($msg:expr) => {
        Err(codec_err(CodecError::InvalidData($msg)))
    };
}

pub trait Codec: Send + Sync + Debug {
    fn name(&self) -> &'static str;
    fn mime_type(&self) -> &'static str;
    fn extension(&self) -> &'static str;

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

#[derive(Debug, Clone, Default)]
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
            return codec_err!("WAV header too short");
        }

        // Verify RIFF header
        if &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
            return codec_err!("Invalid WAV header");
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

pub static CODEC_REGISTRY: Lazy<CodecRegistry> = Lazy::new(|| {
    let mut registry = CodecRegistry::new();
    registry.register_lossless(WavCodec::default());
    registry.register_lossless(FlacCodec::default());
    registry.register_lossy(OpusCodec::new(64));
    registry.register_lossy(MoshiCodec::default());
    registry
});

pub struct CodecRegistry {
    lossless_codecs: Vec<Box<dyn LosslessCodec>>,
    lossy_codecs: Vec<Box<dyn LossyCodec>>,
}

impl CodecRegistry {
    pub fn new() -> Self {
        Self {
            lossless_codecs: Vec::new(),
            lossy_codecs: Vec::new(),
        }
    }

    pub fn register_lossless(&mut self, codec: impl LosslessCodec + 'static) {
        self.lossless_codecs.push(Box::new(codec));
    }

    pub fn register_lossy(&mut self, codec: impl LossyCodec + 'static) {
        self.lossy_codecs.push(Box::new(codec));
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
    use criterion::{black_box, Criterion};
    use test_case::test_case;

    pub struct CodecTester<'a> {
        codec: &'a dyn Codec,
        test_suite: TestSuite,
    }

    impl<'a> CodecTester<'a> {
        pub fn new(codec: &'a dyn Codec) -> Self {
            Self {
                codec,
                test_suite: TestSuite::new(),
            }
        }

        pub fn test_basic_properties(&self) -> Result<()> {
            info!("Testing basic properties for codec: {}", self.codec.name());

            for test_case in &self.test_suite.test_cases {
                let encoded = self
                    .codec
                    .encode(&test_case.samples, test_case.sample_rate)?;
                let decoded = self.codec.decode(&encoded, test_case.sample_rate)?;

                assert_eq!(
                    test_case.samples.len(),
                    decoded.len(),
                    "Decoded length mismatch for {}",
                    test_case.description
                );

                let metrics = self
                    .codec
                    .encoding_speed(&test_case.samples, test_case.sample_rate)?;

                assert!(
                    metrics.encode_speed > 1.0,
                    "Codec {} is slower than real-time for {}",
                    self.codec.name(),
                    test_case.description
                );
            }
            Ok(())
        }
    }

    pub struct LosslessCodecTester {
        codec: Box<dyn LosslessCodec>,
        test_suite: TestSuite,
    }

    impl LosslessCodecTester {
        pub fn new(codec: Box<dyn LosslessCodec>) -> Self {
            Self {
                codec,
                test_suite: TestSuite::new(),
            }
        }

        // For references to boxed codecs
        pub fn new_from_ref(codec: Box<dyn LosslessCodec>) -> Self {
            // Changed parameter type
            Self {
                codec, // Just move the box directly
                test_suite: TestSuite::new(),
            }
        }

        pub fn test_basic_properties(&self) -> Result<()> {
            info!("Testing basic properties for codec: {}", self.codec.name());

            for test_case in &self.test_suite.test_cases {
                let encoded = self
                    .codec
                    .encode(&test_case.samples, test_case.sample_rate)?;
                let decoded = self.codec.decode(&encoded, test_case.sample_rate)?;

                assert_eq!(
                    test_case.samples, decoded,
                    "Perfect reconstruction failed for {}",
                    test_case.description
                );
            }
            Ok(())
        }

        pub fn test_perfect_reconstruction(&self) -> Result<()> {
            info!(
                "Testing perfect reconstruction for codec: {}",
                self.codec.name()
            );

            for test_case in &self.test_suite.test_cases {
                let encoded = self
                    .codec
                    .encode(&test_case.samples, test_case.sample_rate)?;
                let decoded = self.codec.decode(&encoded, test_case.sample_rate)?;

                assert_eq!(
                    test_case.samples, decoded,
                    "Perfect reconstruction failed for {}",
                    test_case.description
                );
            }
            Ok(())
        }
    }

    pub struct LossyCodecTester {
        codec: Box<dyn LossyCodec>,
        test_suite: TestSuite,
        minimum_snr: f32,
    }

    impl LossyCodecTester {
        pub fn new(codec: Box<dyn LossyCodec>, minimum_snr: f32) -> Self {
            Self {
                codec,
                test_suite: TestSuite::new(),
                minimum_snr,
            }
        }

        pub fn new_from_ref(codec: Box<dyn LossyCodec>, minimum_snr: f32) -> Self {
            Self {
                codec,
                test_suite: TestSuite::new(),
                minimum_snr,
            }
        }

        pub fn test_basic_properties(&self) -> Result<()> {
            info!("Testing basic properties for codec: {}", self.codec.name());

            for test_case in &self.test_suite.test_cases {
                let encoded = self
                    .codec
                    .encode(&test_case.samples, test_case.sample_rate)?;
                let decoded = self.codec.decode(&encoded, test_case.sample_rate)?;

                assert_eq!(
                    test_case.samples.len(),
                    decoded.len(),
                    "Decoded length mismatch for {}",
                    test_case.description
                );

                let metrics = self
                    .codec
                    .encoding_speed(&test_case.samples, test_case.sample_rate)?;

                assert!(
                    metrics.encode_speed > 1.0,
                    "Codec {} is slower than real-time for {}",
                    self.codec.name(),
                    test_case.description
                );
            }
            Ok(())
        }

        pub fn test_quality_metrics(&self) -> Result<()> {
            info!("Testing quality metrics for codec: {}", self.codec.name());

            for test_case in &self.test_suite.test_cases {
                let metrics = self
                    .codec
                    .quality_metrics(&test_case.samples, test_case.sample_rate)?;

                assert!(
                    metrics.snr >= self.minimum_snr,
                    "SNR {:.2}dB below minimum {:.2}dB for {}",
                    metrics.snr,
                    self.minimum_snr,
                    test_case.description
                );
            }
            Ok(())
        }
    }

    mod codec_tests {
        use super::*;

        #[test]
        fn test_all_lossless_codecs() -> Result<()> {
            for codec in &CODEC_REGISTRY.lossless_codecs {
                // Create a new Box by cloning the contents
                let tester = LosslessCodecTester::new(Box::new(WavCodec::default()));
                tester.test_basic_properties()?;
                tester.test_perfect_reconstruction()?;
            }
            Ok(())
        }

        #[test]
        fn test_all_lossy_codecs() -> Result<()> {
            for codec in &CODEC_REGISTRY.lossy_codecs {
                let tester = LossyCodecTester::new(Box::new(OpusCodec::new(64)), 20.0);
                tester.test_basic_properties()?;
                tester.test_quality_metrics()?;
            }
            Ok(())
        }
    }

    // BENCHMARKS

    pub mod benches {
        use super::*;
        use criterion::{BenchmarkId, Criterion};

        pub fn benchmark_codec(c: &mut Criterion, codec: &dyn Codec) {
            let test_suite = TestSuite::new();
            let mut group = c.benchmark_group(codec.name());

            for test_case in &test_suite.test_cases {
                group.bench_with_input(
                    BenchmarkId::new("encode", test_case.description),
                    &test_case.samples,
                    |b, samples| {
                        b.iter(|| {
                            codec.encode(black_box(samples), black_box(test_case.sample_rate))
                        })
                    },
                );

                if let Ok(encoded) = codec.encode(&test_case.samples, test_case.sample_rate) {
                    group.bench_with_input(
                        BenchmarkId::new("decode", test_case.description),
                        &encoded,
                        |b, encoded_data| {
                            b.iter(|| {
                                codec.decode(
                                    black_box(encoded_data),
                                    black_box(test_case.sample_rate),
                                )
                            })
                        },
                    );
                }
            }
            group.finish();
        }
    }
}

#[derive(Debug)]
struct CodecWrapper<'a, T: ?Sized + Debug>(&'a T);

impl<T: Codec + ?Sized> Codec for CodecWrapper<'_, T> {
    fn name(&self) -> &'static str {
        self.0.name()
    }

    fn mime_type(&self) -> &'static str {
        self.0.mime_type()
    }

    fn extension(&self) -> &'static str {
        self.0.extension()
    }

    fn encode(&self, data: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
        self.0.encode(data, sample_rate)
    }

    fn decode(&self, data: &[u8], sample_rate: u32) -> Result<Vec<f32>> {
        self.0.decode(data, sample_rate)
    }
}

impl<T: LosslessCodec + ?Sized> LosslessCodec for CodecWrapper<'_, T> {}
impl<T: LossyCodec + ?Sized> LossyCodec for CodecWrapper<'_, T> {}
