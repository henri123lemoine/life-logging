use crate::audio::buffer::AudioBuffer;
use crate::config::CONFIG_MANAGER;
use crate::prelude::*;
use crate::storage::{LocalStorage, S3Storage, StorageManager};
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::broadcast;
use tokio::sync::RwLock;
use tokio::time::Duration;

pub struct AppState {
    pub audio_buffer: Arc<RwLock<AudioBuffer>>,
    pub audio_sender: broadcast::Sender<Vec<f32>>,
    pub start_time: SystemTime,
    pub storage_manager: Arc<StorageManager>,
}

impl AppState {
    pub async fn new() -> Result<Self> {
        let config = CONFIG_MANAGER.get_config().await;
        let (_, stream_config) = CONFIG_MANAGER.get_audio_config().await?;
        let buffer_size =
            config.read().await.buffer_duration as usize * stream_config.sample_rate.0 as usize;

        let local_storage =
            LocalStorage::new(PathBuf::from("./data/audio_storage"), "opus".to_string())?;

        let s3_storage = match (env::var("AWS_S3_BUCKET").ok(), env::var("AWS_REGION").ok()) {
            (Some(bucket), Some(region)) => {
                Some(S3Storage::new(region, bucket, "audio/mac".to_string()).await?)
            }
            _ => None,
        };

        let storage_manager = Arc::new(StorageManager::new(
            local_storage,
            s3_storage,
            Duration::from_secs(config.read().await.buffer_duration),
            48000,
            "opus".to_string(),
        ));

        Ok(AppState {
            audio_buffer: Arc::new(RwLock::new(AudioBuffer::new(
                buffer_size,
                stream_config.sample_rate.0,
            ))),
            audio_sender: broadcast::channel(1024).0,
            start_time: SystemTime::now(),
            storage_manager,
        })
    }
}
