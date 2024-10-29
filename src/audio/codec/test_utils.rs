use crate::error::Error;
use crate::prelude::*;
use cpal::Sample;
use rand::prelude::*;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct AudioTestCase {
    pub name: String,
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub duration: Duration,
    pub category: AudioCategory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioCategory {
    Noise,
    Speech,
    Music,
    Synthetic,
    Custom(&'static str),
}

impl std::fmt::Display for AudioCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Noise => write!(f, "Noise"),
            Self::Speech => write!(f, "Speech"),
            Self::Music => write!(f, "Music"),
            Self::Synthetic => write!(f, "Synthetic"),
            Self::Custom(name) => write!(f, "Custom({})", name),
        }
    }
}

#[derive(Debug)]
pub struct TestSignal {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub description: &'static str,
}

impl TestSignal {
    pub fn resample(&self, new_rate: u32) -> Vec<f32> {
        if self.sample_rate == new_rate {
            return self.samples.clone();
        }

        let ratio = new_rate as f32 / self.sample_rate as f32;
        let new_len = (self.samples.len() as f32 * ratio) as usize;
        let mut resampled = Vec::with_capacity(new_len);

        for i in 0..new_len {
            let src_idx = i as f32 / ratio;
            let src_idx_floor = src_idx.floor() as usize;
            let src_idx_ceil = (src_idx_floor + 1).min(self.samples.len() - 1);
            let frac = src_idx - src_idx.floor();

            let sample =
                self.samples[src_idx_floor] * (1.0 - frac) + self.samples[src_idx_ceil] * frac;
            resampled.push(sample);
        }

        resampled
    }
}

pub struct AudioTestSuite {
    pub(crate) cases: Vec<AudioTestCase>,
}

impl AudioTestSuite {
    pub fn new() -> Self {
        Self { cases: Vec::new() }
    }

    pub fn iter(&self) -> impl Iterator<Item = &AudioTestCase> {
        self.cases.iter()
    }

    pub fn add_case(&mut self, case: AudioTestCase) {
        self.cases.push(case);
    }

    pub fn load_default_cases() -> Result<Self> {
        let mut suite = Self::new();

        // Add synthetic test cases
        suite.add_case(Self::generate_sine_sweep(48000, 2.0, "Sine Sweep")?);
        suite.add_case(Self::generate_multitone(48000, 2.0, "Multitone")?);

        // Add noise test cases with lower weights for quality metrics
        suite.add_case(Self::generate_white_noise(48000, 2.0, "White Noise")?);
        suite.add_case(Self::generate_pink_noise(48000, 2.0, "Pink Noise")?);

        // Load real audio samples
        let test_data_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data");

        if let Ok(voice_data) = std::fs::read(test_data_dir.join("test_voice.wav")) {
            let mut reader = hound::WavReader::new(std::io::Cursor::new(voice_data))
                .map_err(|e| Error::IO(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

            let spec = reader.spec();
            let samples: Vec<f32> = if spec.sample_format == hound::SampleFormat::Float {
                reader
                    .samples::<f32>()
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| Error::IO(std::io::Error::new(std::io::ErrorKind::Other, e)))?
            } else {
                reader
                    .samples::<i16>()
                    .map(|s| s.map(|s| s as f32 / 32768.0))
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| Error::IO(std::io::Error::new(std::io::ErrorKind::Other, e)))?
            };

            let sample_len = samples.len();
            let sample_rate = spec.sample_rate;

            suite.add_case(AudioTestCase {
                name: "Test Voice".to_string(),
                samples,
                sample_rate,
                duration: std::time::Duration::from_secs_f32(
                    sample_len as f32 / sample_rate as f32,
                ),
                category: AudioCategory::Speech,
            });
        } else {
            tracing::warn!("Could not find test_voice.wav in data directory");
        }

        Ok(suite)
    }

    fn generate_white_noise(sample_rate: u32, duration: f32, name: &str) -> Result<AudioTestCase> {
        let num_samples = (sample_rate as f32 * duration) as usize;
        let mut rng = rand::thread_rng();
        let samples: Vec<f32> = (0..num_samples)
            .map(|_| rng.gen_range(-1.0..=1.0))
            .collect();

        Ok(AudioTestCase {
            name: name.to_string(),
            samples,
            sample_rate,
            duration: Duration::from_secs_f32(duration),
            category: AudioCategory::Noise,
        })
    }

    fn generate_pink_noise(sample_rate: u32, duration: f32, name: &str) -> Result<AudioTestCase> {
        let num_samples = (sample_rate as f32 * duration) as usize;
        let mut rng = rand::thread_rng();

        // Simple pink noise approximation using octave bands
        let mut samples = vec![0.0; num_samples];
        let num_octaves = 8;
        let mut octave_amps = vec![0.0; num_octaves];

        for i in 0..num_samples {
            for octave in 0..num_octaves {
                if i % (1 << octave) == 0 {
                    octave_amps[octave] = rng.gen_range(-1.0..=1.0);
                }
                samples[i] += octave_amps[octave] / (octave + 1) as f32;
            }
            samples[i] *= 0.5; // Normalize
        }

        Ok(AudioTestCase {
            name: name.to_string(),
            samples,
            sample_rate,
            duration: Duration::from_secs_f32(duration),
            category: AudioCategory::Noise,
        })
    }

    fn generate_sine_sweep(sample_rate: u32, duration: f32, name: &str) -> Result<AudioTestCase> {
        let num_samples = (sample_rate as f32 * duration) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / sample_rate as f32;
            let freq = 20.0 * (1000.0f32).powf(t / duration);
            let phase = 2.0 * std::f32::consts::PI * freq * t;
            samples.push(phase.sin());
        }

        Ok(AudioTestCase {
            name: name.to_string(),
            samples,
            sample_rate,
            duration: Duration::from_secs_f32(duration),
            category: AudioCategory::Synthetic,
        })
    }

    fn generate_multitone(sample_rate: u32, duration: f32, name: &str) -> Result<AudioTestCase> {
        let num_samples = (sample_rate as f32 * duration) as usize;
        let frequencies = [440.0, 880.0, 1760.0, 3520.0];
        let mut samples = vec![0.0; num_samples];

        for i in 0..num_samples {
            let t = i as f32 / sample_rate as f32;
            for &freq in &frequencies {
                samples[i] += (2.0 * std::f32::consts::PI * freq * t).sin();
            }
        }

        // Normalize
        let max_amp = samples.iter().map(|&x| x.abs()).fold(0.0, f32::max);
        for sample in &mut samples {
            *sample /= max_amp;
        }

        Ok(AudioTestCase {
            name: name.to_string(),
            samples,
            sample_rate,
            duration: Duration::from_secs_f32(duration),
            category: AudioCategory::Synthetic,
        })
    }

    fn load_audio_file(
        path: &PathBuf,
        category: AudioCategory,
        name: &str,
    ) -> Result<AudioTestCase> {
        let mut reader = hound::WavReader::open(path)
            .map_err(|e| Error::IO(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        let spec = reader.spec();
        let samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => reader
                .samples::<f32>()
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| Error::IO(std::io::Error::new(std::io::ErrorKind::Other, e)))?,
            hound::SampleFormat::Int => reader
                .samples::<i32>()
                .map(|s| s.map(|s| s.to_sample()))
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| Error::IO(std::io::Error::new(std::io::ErrorKind::Other, e)))?,
        };

        let duration = Duration::from_secs_f32(samples.len() as f32 / spec.sample_rate as f32);

        Ok(AudioTestCase {
            name: name.to_string(),
            samples,
            sample_rate: spec.sample_rate,
            duration,
            category,
        })
    }
}

// Quality metrics for audio comparison
#[derive(Debug)]
pub struct AudioQualityMetrics {
    pub snr: f32,
    pub mse: f32,
    pub max_abs_error: f32,
    pub correlation: f32,
}

impl AudioQualityMetrics {
    pub fn calculate(original: &[f32], decoded: &[f32]) -> Self {
        let len = original.len().min(decoded.len());
        let orig = &original[..len];
        let dec = &decoded[..len];

        // Calculate MSE and max error all at once
        let (mse_sum, max_error) =
            orig.iter()
                .zip(dec.iter())
                .fold((0.0f32, 0.0f32), |(mse_acc, max_err), (&o, &d)| {
                    let err = (o - d).powi(2);
                    let abs_err = (o - d).abs();
                    (mse_acc + err, max_err.max(abs_err))
                });

        let mse = mse_sum / len as f32;

        // Calculate signal power and SNR
        let signal_power = orig.iter().map(|&x| x.powi(2)).sum::<f32>() / len as f32;

        let snr = if mse > 0.0 {
            10.0 * (signal_power / mse).log10()
        } else {
            f32::INFINITY
        };

        // Calculate correlation
        let mean_orig = orig.iter().sum::<f32>() / len as f32;
        let mean_dec = dec.iter().sum::<f32>() / len as f32;

        let covariance = orig
            .iter()
            .zip(dec.iter())
            .map(|(&o, &d)| (o - mean_orig) * (d - mean_dec))
            .sum::<f32>()
            / len as f32;

        let std_orig =
            (orig.iter().map(|&x| (x - mean_orig).powi(2)).sum::<f32>() / len as f32).sqrt();

        let std_dec =
            (dec.iter().map(|&x| (x - mean_dec).powi(2)).sum::<f32>() / len as f32).sqrt();

        let correlation = if std_orig > 0.0 && std_dec > 0.0 {
            covariance / (std_orig * std_dec)
        } else {
            0.0
        };

        Self {
            snr,
            mse,
            max_abs_error: max_error,
            correlation,
        }
    }
}
