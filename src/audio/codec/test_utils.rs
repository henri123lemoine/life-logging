use crate::prelude::*;

pub struct TestSignal {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub description: &'static str,
}

impl TestSignal {
    pub fn sine(sample_rate: u32, frequency: f32, duration: f32) -> Self {
        let num_samples = (sample_rate as f32 * duration) as usize;
        let samples = (0..num_samples)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (2.0 * std::f32::consts::PI * frequency * t).sin()
            })
            .collect();

        Self {
            samples,
            sample_rate,
            description: "Sine wave",
        }
    }

    pub fn impulse_train(sample_rate: u32, interval: f32) -> Self {
        let num_samples = (sample_rate as f32 * interval * 10.0) as usize;
        let interval_samples = (sample_rate as f32 * interval) as usize;
        let mut samples = vec![0.0; num_samples];

        for i in (0..num_samples).step_by(interval_samples) {
            samples[i] = 1.0;
        }

        Self {
            samples,
            sample_rate,
            description: "Impulse train",
        }
    }

    pub fn unit_test() -> Self {
        Self {
            samples: vec![0.0, 0.5, -0.5, 1.0, -1.0],
            sample_rate: 44100,
            description: "Unit test signal",
        }
    }
}
