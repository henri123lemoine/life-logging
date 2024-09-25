use crate::audio::buffer::AudioBuffer;
use crate::config::CONFIG_MANAGER;
use crate::error::Result;
use crate::persistence::DiskStorage;
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
    pub disk_storage: Arc<DiskStorage>,
}

impl AppState {
    pub async fn new() -> Result<Self> {
        let config = CONFIG_MANAGER.get_config().await;
        let buffer_duration = Duration::from_secs(config.read().await.buffer_duration);
        let (_, stream_config) = CONFIG_MANAGER.get_audio_config().await?;
        let buffer_size =
            config.read().await.buffer_duration as usize * stream_config.sample_rate.0 as usize;

        let disk_storage = Arc::new(
            DiskStorage::new(
                PathBuf::from("./data/audio_storage"),
                "audio/mac".to_string(), // S3 storage path
                buffer_duration,
                "opus".to_string(),
                48000,
            )
            .await?,
        );

        Ok(AppState {
            audio_buffer: Arc::new(RwLock::new(AudioBuffer::new(
                buffer_size,
                stream_config.sample_rate.0,
            ))),
            audio_sender: broadcast::channel(1024).0,
            start_time: SystemTime::now(),
            disk_storage,
        })
    }
}
