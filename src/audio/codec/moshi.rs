use crate::audio::codec::test_utils::TestSignal;
use crate::audio::codec::traits::{Codec, CodecImpl};
use crate::error::CodecError;
use crate::prelude::*;
use candle_core::{DType, Device, Tensor};
use codec_derive::Codec;
use moshi::encodec::{Config, Encodec};
use std::sync::{Arc, Mutex};

#[derive(Debug, Codec)]
#[codec(name = "MOSHI", mime = "audio/moshi", extension = "moshi", lossy)]
pub struct MoshiCodec {
    model: Arc<Mutex<Encodec>>,
    device: Device,
}

impl Default for MoshiCodec {
    fn default() -> Self {
        Self::new().expect("Failed to initialize Moshi model")
    }
}

impl MoshiCodec {
    pub fn new() -> Result<Self> {
        let device = Device::Cpu;
        let config = Config::v0_1(Some(4));
        let vb = candle_nn::VarBuilder::zeros(DType::F32, &device);
        let model = Encodec::new(config, vb)
            .map_err(|_| CodecError::InvalidConfiguration("Failed to create Moshi model"))?;

        Ok(Self {
            model: Arc::new(Mutex::new(model)),
            device,
        })
    }

    fn resample(&self, data: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
        if from_rate == to_rate {
            return data.to_vec();
        }

        // Use the TestSignal resample functionality
        let signal = TestSignal {
            samples: data.to_vec(),
            sample_rate: from_rate,
            description: "Input signal",
        };
        signal.resample(to_rate)
    }
}

impl CodecImpl for MoshiCodec {
    fn encode_samples(&self, data: &[f32], input_rate: u32) -> Result<Vec<u8>> {
        const MOSHI_RATE: u32 = 24000;

        // Resample to Moshi's required rate
        let resampled = self.resample(data, input_rate, MOSHI_RATE);

        // Ensure input length is a multiple of 512 (Moshi requirement)
        let pad_len = (512 - (resampled.len() % 512)) % 512;
        let mut padded = resampled;
        padded.extend(std::iter::repeat(0.0).take(pad_len));

        let input_tensor = Tensor::from_slice(&padded, (1, 1, padded.len()), &self.device)
            .map_err(|e| CodecError::Encoding(format!("Failed to create input tensor: {}", e)))?;

        let encoded = self
            .model
            .lock()
            .map_err(|_| CodecError::Encoding("Failed to acquire model lock".into()))?
            .encode(&input_tensor)
            .map_err(|e| CodecError::Encoding(format!("Moshi encoding failed: {}", e)))?;

        // Serialize tensor to bytes
        let encoded_bytes = encoded
            .to_dtype(DType::U8)
            .map_err(|e| CodecError::Encoding(format!("Failed to convert tensor: {}", e)))?
            .flatten_all()
            .map_err(|e| CodecError::Encoding(format!("Failed to flatten tensor: {}", e)))?
            .to_vec1::<u8>()
            .map_err(|e| CodecError::Encoding(format!("Failed to convert to bytes: {}", e)))?;

        Ok(encoded_bytes)
    }

    fn decode_samples(&self, data: &[u8], output_rate: u32) -> Result<Vec<f32>> {
        const MOSHI_RATE: u32 = 24000;

        // Convert bytes back to tensor
        let encoded_tensor = Tensor::from_slice(data, (1, data.len()), &self.device)
            .map_err(|e| CodecError::Decoding(format!("Failed to create input tensor: {}", e)))?
            .unsqueeze(1)
            .map_err(|e| CodecError::Decoding(format!("Failed to reshape tensor: {}", e)))?;

        // Decode
        let decoded = self
            .model
            .lock()
            .map_err(|_| CodecError::Decoding("Failed to acquire model lock".into()))?
            .decode(&encoded_tensor)
            .map_err(|e| CodecError::Decoding(format!("Moshi decoding failed: {}", e)))?;

        // Convert to samples
        let mut samples = decoded
            .flatten_all()
            .map_err(|e| CodecError::Decoding(format!("Failed to flatten tensor: {}", e)))?
            .to_vec1::<f32>()
            .map_err(|e| CodecError::Decoding(format!("Failed to convert to samples: {}", e)))?;

        // Remove any padding
        while let Some(&last) = samples.last() {
            if last == 0.0 {
                samples.pop();
            } else {
                break;
            }
        }

        // Resample to target rate if needed
        if output_rate != MOSHI_RATE {
            Ok(self.resample(&samples, MOSHI_RATE, output_rate))
        } else {
            Ok(samples)
        }
    }
}
