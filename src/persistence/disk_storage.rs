use crate::audio::buffer::AudioBuffer;
use crate::audio::encoder::ENCODER_FACTORY;
use crate::error::{PersistenceError, Result};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time;
use tracing::{error, info};

pub struct DiskStorage {
    storage_path: PathBuf,
    interval: Duration,
    format: String,
    target_sample_rate: u32,
}

impl DiskStorage {
    pub fn new(
        storage_path: PathBuf,
        interval: Duration,
        format: String,
        target_sample_rate: u32,
    ) -> Result<Self> {
        fs::create_dir_all(&storage_path).map_err(PersistenceError::DirectoryCreation)?;

        // Verify that the format is supported
        if ENCODER_FACTORY.get_encoder(&format).is_none() {
            return Err(PersistenceError::UnsupportedFormat(format).into());
        }

        Ok(Self {
            storage_path,
            interval,
            format,
            target_sample_rate,
        })
    }

    pub async fn start_persistence_task(&self, audio_buffer: Arc<RwLock<AudioBuffer>>) {
        let mut interval = time::interval(self.interval);

        loop {
            interval.tick().await;
            if let Err(e) = self.persist_audio(&audio_buffer).await {
                error!("Failed to persist audio: {}", e);
            }
        }
    }

    async fn persist_audio(&self, audio_buffer: &Arc<RwLock<AudioBuffer>>) -> Result<()> {
        let buffer = audio_buffer
            .read()
            .map_err(|_| PersistenceError::BufferLockAcquisition)?;
        let data = buffer.read(Some(self.interval));
        let current_sample_rate = buffer.get_sample_rate();

        // Resample if necessary
        let resampled_data = if current_sample_rate != self.target_sample_rate {
            self.resample(&data, current_sample_rate, self.target_sample_rate)
        } else {
            data
        };

        // Get the encoder instance
        let encoder = ENCODER_FACTORY
            .get_encoder(&self.format)
            .unwrap_or_else(|| panic!("Unsupported format: {}", self.format));

        let encoded_data = encoder.encode(&resampled_data, self.target_sample_rate)?;

        let filename = self.generate_filename();
        let file_path = self.storage_path.join(filename);
        fs::write(file_path, encoded_data).map_err(PersistenceError::FileWrite)?;

        info!("Persisted audio data to disk");
        Ok(())
    }

    fn resample(&self, data: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
        // Implement resampling logic here
        // For simplicity, we'll use linear interpolation
        let ratio = from_rate as f32 / to_rate as f32;
        let new_len = (data.len() as f32 / ratio).ceil() as usize;
        let mut resampled = Vec::with_capacity(new_len);

        for i in 0..new_len {
            let pos = i as f32 * ratio;
            let index = pos.floor() as usize;
            let frac = pos - pos.floor();

            if index + 1 < data.len() {
                let sample = data[index] * (1.0 - frac) + data[index + 1] * frac;
                resampled.push(sample);
            } else {
                resampled.push(data[index]);
            }
        }

        resampled
    }

    fn generate_filename(&self) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        format!("audio_{}.{}", now, self.format)
    }
}
