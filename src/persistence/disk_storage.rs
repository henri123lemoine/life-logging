use crate::audio::buffer::AudioBuffer;
use crate::audio::encoder::ENCODER_FACTORY;
use crate::error::{PersistenceError, Result};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::StorageClass;
use aws_sdk_s3::{config::Region, Client};
use chrono::Datelike;
use chrono::{TimeZone, Utc};
use dotenv::dotenv;
use std::env;
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
    s3_client: Option<Client>,
    s3_bucket: Option<String>,
}

impl DiskStorage {
    pub async fn new(
        storage_path: PathBuf,
        interval: Duration,
        format: String,
        target_sample_rate: u32,
    ) -> Result<Self> {
        dotenv().ok();

        fs::create_dir_all(&storage_path).map_err(PersistenceError::DirectoryCreation)?;

        if ENCODER_FACTORY.get_encoder(&format).is_none() {
            return Err(PersistenceError::UnsupportedFormat(format).into());
        }

        let s3_bucket = env::var("AWS_S3_BUCKET").ok();
        let s3_region = env::var("AWS_REGION").ok();

        let s3_client = if let (Some(_), Some(region)) = (s3_bucket.as_ref(), s3_region.as_ref()) {
            let aws_access_key_id = env::var("AWS_ACCESS_KEY_ID")
                .map_err(|_| PersistenceError::S3Config("AWS_ACCESS_KEY_ID not set".to_string()))?;
            let aws_secret_access_key = env::var("AWS_SECRET_ACCESS_KEY").map_err(|_| {
                PersistenceError::S3Config("AWS_SECRET_ACCESS_KEY not set".to_string())
            })?;

            let config = aws_config::defaults(aws_config::BehaviorVersion::v2024_03_28())
                .region(Region::new(region.clone()))
                .credentials_provider(aws_sdk_s3::config::Credentials::new(
                    aws_access_key_id,
                    aws_secret_access_key,
                    None,
                    None,
                    "env",
                ))
                .load()
                .await;

            Some(Client::new(&config))
        } else {
            None
        };

        Ok(Self {
            storage_path,
            interval,
            format,
            target_sample_rate,
            s3_client,
            s3_bucket,
        })
    }

    pub async fn start_persistence_task(&self, audio_buffer: Arc<RwLock<AudioBuffer>>) {
        let mut interval = time::interval(self.interval);
        loop {
            interval.tick().await;
            if let Err(e) = self.persist_audio(audio_buffer.clone()).await {
                error!("Failed to persist audio: {}", e);
            }
        }
    }

    async fn persist_audio(&self, audio_buffer: Arc<RwLock<AudioBuffer>>) -> Result<()> {
        let data = {
            let buffer = audio_buffer
                .read()
                .map_err(|_| PersistenceError::BufferLockAcquisition)?;
            buffer.read(Some(self.interval))
        };

        let current_sample_rate = {
            let buffer = audio_buffer
                .read()
                .map_err(|_| PersistenceError::BufferLockAcquisition)?;
            buffer.get_sample_rate()
        };

        let resampled_data = if current_sample_rate != self.target_sample_rate {
            self.resample(&data, current_sample_rate, self.target_sample_rate)
        } else {
            data
        };

        let encoder = ENCODER_FACTORY
            .get_encoder(&self.format)
            .unwrap_or_else(|| panic!("Unsupported format: {}", self.format));
        let encoded_data = encoder.encode(&resampled_data, self.target_sample_rate)?;

        let filename = self.generate_filename();
        let file_path = self.storage_path.join(&filename);

        // Create directory structure if it doesn't exist
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).map_err(PersistenceError::DirectoryCreation)?;
        }

        fs::write(&file_path, &encoded_data).map_err(PersistenceError::FileWrite)?;
        info!("Persisted audio data to disk: {}", file_path.display());

        if let (Some(client), Some(bucket)) = (&self.s3_client, &self.s3_bucket) {
            self.upload_to_s3(client, bucket, &file_path, &filename)
                .await?;
            info!("Uploaded audio data to S3: {}", filename);
        }

        Ok(())
    }

    async fn upload_to_s3(
        &self,
        client: &Client,
        bucket: &str,
        file_path: &PathBuf,
        key: &str,
    ) -> Result<()> {
        let body = match ByteStream::from_path(file_path).await {
            Ok(stream) => stream,
            Err(e) => {
                error!("Failed to create ByteStream from file: {:?}", e);
                return Err(PersistenceError::S3Upload(format!(
                    "Failed to create ByteStream: {}",
                    e
                ))
                .into());
            }
        };

        let result = client
            .put_object()
            .bucket(bucket)
            .key(key)
            .body(body)
            .storage_class(StorageClass::GlacierIr)
            .send()
            .await;

        match result {
            Ok(_) => {
                info!("Successfully uploaded file to S3: {}", key);
                Ok(())
            }
            Err(e) => {
                error!("Failed to upload file to S3: {:?}", e);
                Err(PersistenceError::S3Upload(format!("S3 upload failed: {}", e)).into())
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

    fn generate_filename(&self) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let datetime = Utc.timestamp_opt(now as i64, 0).unwrap();

        format!(
            "audio/mac-audio/{year}/{month:02}/{day:02}/audio_{timestamp}.{ext}",
            year = datetime.year(),
            month = datetime.month(),
            day = datetime.day(),
            timestamp = now,
            ext = self.format
        )
    }
}
