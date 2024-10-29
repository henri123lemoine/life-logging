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
        let config = Config::v0_1(None);
        let vb = candle_nn::VarBuilder::zeros(DType::F32, &device);
        let model = Encodec::new(config, vb)
            .map_err(|_| CodecError::InvalidConfiguration("Failed to create Moshi model"))?;

        Ok(Self {
            model: Arc::new(Mutex::new(model)),
            device,
        })
    }
}

impl CodecImpl for MoshiCodec {
    fn encode_samples(&self, data: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
        if sample_rate != 24000 {
            // Resample data to 24kHz
            let test_signal = TestSignal {
                samples: data.to_vec(),
                sample_rate,
                description: "Input signal",
            };
            let resampled = test_signal.resample(24000);
            return self.encode_samples(&resampled, 24000);
        }

        let input_tensor = Tensor::from_slice(data, (1, 1, data.len()), &self.device)
            .map_err(|e| CodecError::Encoding(format!("Failed to create input tensor: {}", e)))?;

        let encoded = self
            .model
            .lock()
            .map_err(|e| CodecError::Encoding(format!("Failed to acquire model lock: {}", e)))?
            .encode(&input_tensor)
            .map_err(|e| CodecError::Encoding(format!("Moshi encoding failed: {}", e)))?;

        let encoded_bytes = encoded
            .to_dtype(DType::U8)
            .map_err(|e| CodecError::Encoding(format!("Failed to convert tensor: {}", e)))?
            .flatten_all()
            .map_err(|e| CodecError::Encoding(format!("Failed to flatten tensor: {}", e)))?
            .to_vec1::<u8>()
            .map_err(|e| CodecError::Encoding(format!("Failed to convert to bytes: {}", e)))?;

        Ok(encoded_bytes)
    }

    fn decode_samples(&self, data: &[u8], sample_rate: u32) -> Result<Vec<f32>> {
        if sample_rate != 24000 {
            return Err(CodecError::UnsupportedSampleRate(sample_rate).into());
        }

        let encoded_tensor = Tensor::from_slice(data, (1, data.len()), &self.device)
            .map_err(|e| CodecError::Decoding(format!("Failed to create input tensor: {}", e)))?;

        let encoded_tensor = encoded_tensor
            .unsqueeze(1)
            .map_err(|e| CodecError::Decoding(format!("Failed to reshape tensor: {}", e)))?;

        let decoded = self
            .model
            .lock()
            .map_err(|e| CodecError::Decoding(format!("Failed to acquire model lock: {}", e)))?
            .decode(&encoded_tensor)
            .map_err(|e| CodecError::Decoding(format!("Moshi decoding failed: {}", e)))?;

        let samples = decoded
            .flatten_all()
            .map_err(|e| CodecError::Decoding(format!("Failed to flatten tensor: {}", e)))?
            .to_vec1::<f32>()
            .map_err(|e| CodecError::Decoding(format!("Failed to convert to samples: {}", e)))?;

        if sample_rate != 24000 {
            // Resample back to original rate
            let test_signal = TestSignal {
                samples,
                sample_rate: 24000,
                description: "Decoded signal",
            };
            Ok(test_signal.resample(sample_rate))
        } else {
            Ok(samples)
        }
    }
}
