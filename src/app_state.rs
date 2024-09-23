use crate::audio::buffer::AudioBuffer;
use crate::config::CONFIG_MANAGER;
use crate::error::Result;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;
use tokio::sync::broadcast;

pub struct AppState {
    pub audio_buffer: Arc<RwLock<AudioBuffer>>,
    pub audio_sender: broadcast::Sender<Vec<f32>>,
    pub start_time: SystemTime,
}

impl AppState {
    pub async fn new() -> Result<Arc<Self>> {
        let config = CONFIG_MANAGER.get_config().await;
        let (_, stream_config) = CONFIG_MANAGER.get_audio_config().await?;
        let buffer_size =
            config.read().await.buffer_duration as usize * stream_config.sample_rate.0 as usize;
        let app_state = Arc::new(AppState {
            audio_buffer: Arc::new(RwLock::new(AudioBuffer::new(
                buffer_size,
                stream_config.sample_rate.0,
            ))),
            audio_sender: broadcast::channel(1024).0,
            start_time: SystemTime::now(),
        });

        Ok(app_state)
    }
}
