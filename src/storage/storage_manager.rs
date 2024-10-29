use super::{LocalStorage, S3Storage, Storage};
use crate::audio::buffer::AudioBuffer;
use crate::audio::codec::CODEC_FACTORY;
use crate::error::AudioError;
use crate::prelude::*;
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time;
use tracing::{error, info};

pub struct StorageManager {
    local_storage: Arc<LocalStorage>,
    s3_storage: Option<Arc<S3Storage>>,
    local_interval: Duration,
    target_sample_rate: u32,
    format: String,
}

impl StorageManager {
    pub fn new(
        local_storage: LocalStorage,
        s3_storage: Option<S3Storage>,
        local_interval: Duration,
        target_sample_rate: u32,
        format: String,
    ) -> Self {
        Self {
            local_storage: Arc::new(local_storage),
            s3_storage: s3_storage.map(Arc::new),
            local_interval,
            target_sample_rate,
            format,
        }
    }

    pub async fn persist_audio(&self, audio_buffer: Arc<RwLock<AudioBuffer>>) -> Result<()> {
        let data = {
            let buffer = audio_buffer.read().await;
            buffer.read(Some(self.local_interval))
        };

        let current_sample_rate = {
            let buffer = audio_buffer.read().await;
            buffer.get_sample_rate()
        };

        let resampled_data = if current_sample_rate != self.target_sample_rate {
            self.resample(&data, current_sample_rate, self.target_sample_rate)
        } else {
            data
        };

        let encoder = CODEC_FACTORY
            .get(&self.format)
            .ok_or_else(|| AudioError::UnsupportedFormat(self.format.clone()))?;

        let encoded_data = encoder.encode(&resampled_data, self.target_sample_rate)?;

        let timestamp = Utc::now();

        self.local_storage.save(&encoded_data, timestamp).await?;

        match &self.s3_storage {
            Some(s3) => {
                info!("Attempting to save to S3");
                s3.save(&encoded_data, timestamp).await?
            }
            None => info!("S3 storage not configured, skipping S3 upload"),
        }

        Ok(())
    }

    pub async fn start_persistence_task(self: Arc<Self>, audio_buffer: Arc<RwLock<AudioBuffer>>) {
        let mut interval = time::interval(self.local_interval);
        loop {
            interval.tick().await;
            if let Err(e) = self.persist_audio(audio_buffer.clone()).await {
                error!("Failed to persist audio: {}", e);
            }
        }
    }

    pub async fn start_cleanup_task(self: Arc<Self>, local_retention: Duration) {
        let cleanup_interval = Duration::from_secs(3600); // Run every hour
        let mut interval = time::interval(cleanup_interval);
        loop {
            interval.tick().await;
            if let Err(e) = self.local_storage.cleanup(local_retention).await {
                error!("Failed to clean up local storage: {}", e);
            }
        }
    }

    fn resample(&self, data: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
        if from_rate == to_rate {
            return data.to_vec();
        }

        if data.is_empty() {
            return Vec::new();
        }

        let ratio = from_rate as f32 / to_rate as f32;
        let new_len = (data.len() as f32 / ratio).ceil() as usize;
        let mut resampled = Vec::with_capacity(new_len);

        for i in 0..new_len {
            let pos = i as f32 * ratio;
            let index = (pos.floor() as usize).min(data.len() - 1);
            let next_index = (index + 1).min(data.len() - 1);
            let frac = pos - pos.floor();

            let sample = data[index] * (1.0 - frac) + data[next_index] * frac;
            resampled.push(sample);
        }

        resampled
    }

    pub async fn cleanup(&self, local_retention: Duration, s3_retention: Duration) -> Result<()> {
        self.local_storage.cleanup(local_retention).await?;
        if let Some(s3) = &self.s3_storage {
            s3.cleanup(s3_retention).await?;
        }
        Ok(())
    }
}
