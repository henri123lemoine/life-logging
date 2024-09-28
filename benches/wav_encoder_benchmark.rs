/// Benchmark for an optimized WAV encoder implementations.
/// Results (44100hz, 20 minutes audio):
///     Original:  [63.263 ms 63.455 ms 63.646 ms]
///     Optimized: [62.074 ms 62.321 ms 62.612 ms]
///     ~1-2% faster than the original implementation (statistically significant)
/// Analysis:
///     The difference is too small to justify the added complexity of the implementation.
///     The standard wav encoder implementation will remain unchanged.
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use life_logging::audio::encoder::{AudioEncoder, WavEncoder};
use life_logging::prelude::*;

pub struct OptimizedWavEncoder;

impl AudioEncoder for OptimizedWavEncoder {
    fn encode(&self, data: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
        let channels = 1u16;
        let bits_per_sample = 16u16;
        let byte_rate = sample_rate * u32::from(channels) * u32::from(bits_per_sample) / 8;
        let block_align = channels * bits_per_sample / 8;
        let data_size = (data.len() * 2) as u32;
        let file_size = 36 + data_size;

        let mut buffer = Vec::with_capacity(file_size as usize + 8);

        // Write header in one go
        buffer.extend_from_slice(&[
            b'R',
            b'I',
            b'F',
            b'F',
            (file_size & 0xFF) as u8,
            ((file_size >> 8) & 0xFF) as u8,
            ((file_size >> 16) & 0xFF) as u8,
            ((file_size >> 24) & 0xFF) as u8,
            b'W',
            b'A',
            b'V',
            b'E',
            b'f',
            b'm',
            b't',
            b' ',
            16,
            0,
            0,
            0, // Chunk size
            1,
            0, // Audio format (PCM)
            (channels & 0xFF) as u8,
            ((channels >> 8) & 0xFF) as u8,
            (sample_rate & 0xFF) as u8,
            ((sample_rate >> 8) & 0xFF) as u8,
            ((sample_rate >> 16) & 0xFF) as u8,
            ((sample_rate >> 24) & 0xFF) as u8,
            (byte_rate & 0xFF) as u8,
            ((byte_rate >> 8) & 0xFF) as u8,
            ((byte_rate >> 16) & 0xFF) as u8,
            ((byte_rate >> 24) & 0xFF) as u8,
            (block_align & 0xFF) as u8,
            ((block_align >> 8) & 0xFF) as u8,
            (bits_per_sample & 0xFF) as u8,
            ((bits_per_sample >> 8) & 0xFF) as u8,
            b'd',
            b'a',
            b't',
            b'a',
            (data_size & 0xFF) as u8,
            ((data_size >> 8) & 0xFF) as u8,
            ((data_size >> 16) & 0xFF) as u8,
            ((data_size >> 24) & 0xFF) as u8,
        ]);

        // Convert and write audio data
        let mut temp = [0u8; 2];
        for &sample in data {
            let value = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
            temp[0] = value as u8;
            temp[1] = (value >> 8) as u8;
            buffer.extend_from_slice(&temp);
        }

        Ok(buffer)
    }

    fn mime_type(&self) -> &'static str {
        "audio/wav"
    }

    fn content_disposition(&self) -> &'static str {
        "attachment; filename=\"audio.wav\""
    }
}

fn generate_test_data(samples: usize) -> Vec<f32> {
    (0..samples)
        .map(|i| (i as f32 / samples as f32).sin())
        .collect()
}

fn benchmark_wav_encoders(c: &mut Criterion) {
    let original_encoder = WavEncoder;
    let optimized_encoder = OptimizedWavEncoder;
    let sample_rates = [44100];
    let durations = [1, 60, 60 * 20]; // 1 second; 1 minute; 20 minutes

    for &sample_rate in &sample_rates {
        for &duration in &durations {
            let samples = sample_rate as usize * duration;
            let data = generate_test_data(samples);

            let mut group =
                c.benchmark_group(format!("wav_encode_{}hz_{}s", sample_rate, duration));

            group.bench_function("original", |b| {
                b.iter(|| original_encoder.encode(black_box(&data), black_box(sample_rate)))
            });

            group.bench_function("optimized", |b| {
                b.iter(|| optimized_encoder.encode(black_box(&data), black_box(sample_rate)))
            });

            group.finish();
        }
    }
}

criterion_group!(benches, benchmark_wav_encoders);
criterion_main!(benches);
