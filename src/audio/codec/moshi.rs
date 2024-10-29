use crate::audio::codec::traits::{Codec, CodecImpl};
use crate::error::CodecError;
use crate::prelude::*;
use candle_core::{DType, Device, Tensor};
use codec_derive::Codec;
use moshi::encodec::{Config, Encodec};
use std::sync::{Arc, Mutex};

const MOSHI_SAMPLE_RATE: u32 = 24000;
const FRAME_SIZE: usize = 512;

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
        // Use default number of codebooks for stability
        let config = Config::v0_1(None);
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

        let ratio = to_rate as f32 / from_rate as f32;
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

    fn pad_to_frame_size(&self, data: &[f32]) -> Vec<f32> {
        let remainder = data.len() % FRAME_SIZE;
        if remainder == 0 {
            return data.to_vec();
        }

        let padding_size = FRAME_SIZE - remainder;
        let mut padded = Vec::with_capacity(data.len() + padding_size);
        padded.extend_from_slice(data);
        padded.extend(std::iter::repeat(0.0).take(padding_size));
        padded
    }
}

impl CodecImpl for MoshiCodec {
    fn encode_samples(&self, data: &[f32], input_rate: u32) -> Result<Vec<u8>> {
        // 1. Resample to Moshi's required rate
        let resampled = self.resample(data, input_rate, MOSHI_SAMPLE_RATE);

        // 2. Pad to required frame size
        let padded = self.pad_to_frame_size(&resampled);

        // 3. Create properly shaped input tensor (batch_size=1, channels=1, samples)
        let input_tensor = Tensor::from_slice(&padded, (1, 1, padded.len()), &self.device)
            .map_err(|e| CodecError::Encoding(format!("Failed to create input tensor: {}", e)))?;

        // 4. Encode using model
        let encoded = self
            .model
            .lock()
            .map_err(|e| CodecError::Encoding(format!("Failed to acquire model lock: {}", e)))?
            .encode(&input_tensor)
            .map_err(|e| CodecError::Encoding(format!("Moshi encoding failed: {}", e)))?;

        // 5. Convert to bytes with shape information
        let shape: Vec<usize> = encoded.shape().dims().to_vec();
        let encoded_flat = encoded
            .flatten_all()
            .map_err(|e| CodecError::Encoding(format!("Failed to flatten tensor: {}", e)))?
            .to_vec1::<u8>()
            .map_err(|e| CodecError::Encoding(format!("Failed to convert to bytes: {}", e)))?;

        // 6. Serialize shape and data
        let mut output = Vec::new();
        output.extend_from_slice(&(shape.len() as u32).to_le_bytes());
        for dim in shape {
            output.extend_from_slice(&(dim as u32).to_le_bytes());
        }
        output.extend_from_slice(&(encoded_flat.len() as u32).to_le_bytes());
        output.extend(encoded_flat);

        Ok(output)
    }

    fn decode_samples(&self, data: &[u8], output_rate: u32) -> Result<Vec<f32>> {
        // 1. Deserialize shape and data
        if data.len() < 4 {
            return Err(CodecError::Decoding("Invalid data length".into()).into());
        }

        let mut offset = 0;

        // Read number of dimensions
        let ndim = u32::from_le_bytes(data[offset..offset + 4].try_into()?) as usize;
        offset += 4;

        if offset + ndim * 4 + 4 > data.len() {
            return Err(CodecError::Decoding("Invalid shape data".into()).into());
        }

        // Read shape
        let mut shape = Vec::with_capacity(ndim);
        for _ in 0..ndim {
            let dim = u32::from_le_bytes(data[offset..offset + 4].try_into()?) as usize;
            shape.push(dim);
            offset += 4;
        }

        // Read data length
        let data_len = u32::from_le_bytes(data[offset..offset + 4].try_into()?) as usize;
        offset += 4;

        if offset + data_len > data.len() {
            return Err(CodecError::Decoding("Invalid data length".into()).into());
        }

        // 2. Create input tensor with correct shape
        let encoded_tensor =
            Tensor::from_slice(&data[offset..offset + data_len], shape, &self.device).map_err(
                |e| CodecError::Decoding(format!("Failed to create input tensor: {}", e)),
            )?;

        // 3. Decode using model
        let decoded = self
            .model
            .lock()
            .map_err(|e| CodecError::Decoding(format!("Failed to acquire model lock: {}", e)))?
            .decode(&encoded_tensor)
            .map_err(|e| CodecError::Decoding(format!("Moshi decoding failed: {}", e)))?;

        // 4. Convert decoded tensor to samples
        let mut samples = decoded
            .flatten_all()
            .map_err(|e| CodecError::Decoding(format!("Failed to flatten decoded tensor: {}", e)))?
            .to_vec1::<f32>()
            .map_err(|e| CodecError::Decoding(format!("Failed to convert to samples: {}", e)))?;

        // 5. Remove padding if present
        while samples.last().map_or(false, |&x| x.abs() < 1e-6) {
            samples.pop();
        }

        // 6. Resample to target rate if needed
        if output_rate != MOSHI_SAMPLE_RATE {
            Ok(self.resample(&samples, MOSHI_SAMPLE_RATE, output_rate))
        } else {
            Ok(samples)
        }
    }
}
