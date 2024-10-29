use crate::audio::codec::traits::{Codec, CodecImpl};
use crate::error::{AudioError, CodecError};
use crate::prelude::*;
use codec_derive::Codec;

#[derive(Debug, Codec)]
#[codec(name = "WAV", mime = "audio/wav", extension = "wav", lossless)]
pub struct WavCodec {
    bits_per_sample: u16,
}

impl Default for WavCodec {
    fn default() -> Self {
        Self {
            bits_per_sample: 16,
        }
    }
}

impl CodecImpl for WavCodec {
    fn encode_samples(&self, data: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
        let channels = 1u16;
        let bytes_per_sample = self.bits_per_sample / 8;
        let byte_rate = sample_rate * u32::from(channels * bytes_per_sample);
        let block_align = channels * bytes_per_sample;
        let data_size = (data.len() * bytes_per_sample as usize) as u32;
        let file_size = 36 + data_size;

        let mut buffer = Vec::with_capacity(44 + data.len() * bytes_per_sample as usize);

        // Write WAV header
        buffer.extend_from_slice(b"RIFF");
        buffer.extend_from_slice(&file_size.to_le_bytes());
        buffer.extend_from_slice(b"WAVE");
        buffer.extend_from_slice(b"fmt ");
        buffer.extend_from_slice(&16u32.to_le_bytes());
        buffer.extend_from_slice(&1u16.to_le_bytes());
        buffer.extend_from_slice(&channels.to_le_bytes());
        buffer.extend_from_slice(&sample_rate.to_le_bytes());
        buffer.extend_from_slice(&byte_rate.to_le_bytes());
        buffer.extend_from_slice(&block_align.to_le_bytes());
        buffer.extend_from_slice(&self.bits_per_sample.to_le_bytes());
        buffer.extend_from_slice(b"data");
        buffer.extend_from_slice(&data_size.to_le_bytes());

        // Write audio data
        for &sample in data {
            match self.bits_per_sample {
                16 => {
                    let value = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
                    buffer.extend_from_slice(&value.to_le_bytes());
                }
                32 => {
                    let value = (sample.clamp(-1.0, 1.0) * 2147483647.0) as i32;
                    buffer.extend_from_slice(&value.to_le_bytes());
                }
                _ => unreachable!("bits_per_sample already validated"),
            }
        }

        Ok(buffer)
    }

    fn decode_samples(&self, data: &[u8], _sample_rate: u32) -> Result<Vec<f32>> {
        if data.len() < 44 {
            return Err(Error::Audio(AudioError::Codec(CodecError::InvalidData(
                "WAV header too short",
            ))));
        }

        if &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
            return Err(Error::Audio(AudioError::Codec(CodecError::InvalidData(
                "Invalid WAV header",
            ))));
        }

        let mut offset = 12;
        while offset + 8 <= data.len() {
            let chunk_id = &data[offset..offset + 4];
            let chunk_size = u32::from_le_bytes(
                data[offset + 4..offset + 8]
                    .try_into()
                    .map_err(|_| CodecError::InvalidData("Invalid chunk size"))?,
            );

            if chunk_id == b"data" {
                let data_offset = offset + 8;
                let bytes_per_sample = self.bits_per_sample as usize / 8;
                let mut samples = Vec::new();

                for chunk in data[data_offset..].chunks_exact(bytes_per_sample) {
                    let sample = match self.bits_per_sample {
                        16 => {
                            let value = i16::from_le_bytes(chunk.try_into()?);
                            value as f32 / 32767.0
                        }
                        32 => {
                            let value = i32::from_le_bytes(chunk.try_into()?);
                            value as f32 / 2147483647.0
                        }
                        _ => unreachable!(),
                    };
                    samples.push(sample);
                }

                return Ok(samples);
            }

            offset += 8 + chunk_size as usize;
        }

        Err(Error::Audio(AudioError::Codec(CodecError::InvalidData(
            "No data chunk found",
        ))))
    }
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
}
