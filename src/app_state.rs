use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use tokio::sync::broadcast;
use tracing::info;
use crate::audio::buffer::CircularAudioBuffer;
use crate::config::CONFIG_MANAGER;
use crate::error::Result;

pub struct AppState {
    pub audio_buffer: Arc<CircularAudioBuffer>,
    pub audio_sender: broadcast::Sender<Vec<f32>>,
    pub start_time: SystemTime,
}

impl AppState {
    pub async fn new() -> Result<Arc<Self>> {
        let config = CONFIG_MANAGER.get_config().await;
        let (_, stream_config) = CONFIG_MANAGER.get_audio_config().await?;
        let buffer_size = config.read().await.buffer_duration as usize * stream_config.sample_rate.0 as usize;
        let audio_buffer = Arc::new(CircularAudioBuffer::new(buffer_size, stream_config.sample_rate.0));
        let buffer_size = config.read().await.audio_channel_buffer_size.unwrap_or(1024);
        let (audio_sender, _) = broadcast::channel(buffer_size);

        let app_state = Arc::new(AppState {
            audio_buffer: audio_buffer.clone(),
            audio_sender,
            start_time: SystemTime::now(),
        });

        // Initialize buffer with silence
        let silence = vec![0.0; buffer_size];
        app_state.audio_buffer.write(&silence);

        Ok(app_state)
    }
}
