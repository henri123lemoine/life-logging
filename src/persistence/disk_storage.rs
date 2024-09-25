use crate::audio::buffer::AudioBuffer;
use crate::audio::encoder::ENCODER_FACTORY;
use crate::error::{PersistenceError, Result};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::StorageClass;
use aws_sdk_s3::{config::Region, Client};
use chrono::{DateTime, Utc};
use chrono::{Datelike, Timelike};
use dotenv::dotenv;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::{env, fs};
use tokio::sync::Mutex;
use tokio::sync::RwLock as TokioRwLock;
use tokio::time;
use tracing::{error, info};

pub struct DiskStorage {
    local_storage_path: PathBuf,
    s3_storage_path: String,
    interval: Duration,
    format: String,
    target_sample_rate: u32,
    s3_client: Option<Client>,
    s3_bucket: Option<String>,
    local_files: Mutex<VecDeque<(DateTime<Utc>, PathBuf)>>,
}

impl DiskStorage {
    pub async fn new(
        local_storage_path: PathBuf,
        s3_storage_path: String,
        interval: Duration,
        format: String,
        target_sample_rate: u32,
    ) -> Result<Self> {
        dotenv().ok();

        fs::create_dir_all(&local_storage_path).map_err(PersistenceError::DirectoryCreation)?;

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
            local_storage_path,
            s3_storage_path,
            interval,
            format,
            target_sample_rate,
            s3_client,
            s3_bucket,
            local_files: Mutex::new(VecDeque::new()),
        })
    }

    pub async fn start_persistence_task(
        self: Arc<Self>,
        audio_buffer: Arc<TokioRwLock<AudioBuffer>>,
    ) {
        let mut interval = time::interval(self.interval);
        loop {
            interval.tick().await;
            if let Err(e) = self.persist_audio(audio_buffer.clone()).await {
                error!("Failed to persist audio: {}", e);
            }
        }
    }

    async fn persist_audio(&self, audio_buffer: Arc<TokioRwLock<AudioBuffer>>) -> Result<()> {
        let data = {
            let buffer = audio_buffer.read().await;
            buffer.read(Some(self.interval))
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

        let encoder = ENCODER_FACTORY
            .get_encoder(&self.format)
            .unwrap_or_else(|| panic!("Unsupported format: {}", self.format));
        let encoded_data = encoder.encode(&resampled_data, self.target_sample_rate)?;

        let now = Utc::now();
        let local_filename = self.generate_local_filename(&now);
        let s3_filename = self.generate_s3_filename(&now);

        let local_file_path = self.local_storage_path.join(&local_filename);
        fs::write(&local_file_path, &encoded_data).map_err(PersistenceError::FileWrite)?;
        info!(
            "Persisted audio data to local disk: {}",
            local_file_path.display()
        );

        let mut local_files = self.local_files.lock().await;
        local_files.push_back((now, local_file_path.clone()));
        self.cleanup_old_local_files(&mut local_files).await;

        if let (Some(client), Some(bucket)) = (&self.s3_client, &self.s3_bucket) {
            let s3_key = format!("{}/{}", self.s3_storage_path, s3_filename);
            self.upload_to_s3(client, bucket, &local_file_path, &s3_key)
                .await?;
            info!("Uploaded audio data to S3: {}", s3_key);
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

    fn generate_local_filename(&self, datetime: &DateTime<Utc>) -> String {
        format!(
            "audio_{hour:02}_{minute:02}.{ext}",
            hour = datetime.hour(),
            minute = datetime.minute(),
            ext = self.format
        )
    }

    fn generate_s3_filename(&self, datetime: &DateTime<Utc>) -> String {
        format!(
            "{year}/{month:02}/{day:02}/audio_{hour:02}_{minute:02}.{ext}",
            year = datetime.year(),
            month = datetime.month(),
            day = datetime.day(),
            hour = datetime.hour(),
            minute = datetime.minute(),
            ext = self.format
        )
    }

    async fn cleanup_old_local_files(&self, local_files: &mut VecDeque<(DateTime<Utc>, PathBuf)>) {
        let five_hours_ago = Utc::now() - Duration::from_secs(60 * 60 * 5);
        while let Some((timestamp, path)) = local_files.front() {
            if *timestamp < five_hours_ago {
                if let Err(e) = fs::remove_file(path) {
                    error!("Failed to remove old local file: {:?}. Error: {}", path, e);
                } else {
                    info!("Removed old local file: {:?}", path);
                }
                local_files.pop_front();
            } else {
                break;
            }
        }
    }
}
