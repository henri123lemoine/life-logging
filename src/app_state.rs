use std::sync::{Arc, RwLock};
use std::time::SystemTime;
use tokio::sync::broadcast;
use tracing::info;
use crate::audio::buffer::CircularAudioBuffer;
use crate::config::CONFIG_MANAGER;
use crate::error::{LifeLoggingError, Result};

pub struct AppState {
    pub audio_buffer: Arc<RwLock<CircularAudioBuffer>>,
    pub audio_sender: broadcast::Sender<Vec<f32>>,
    pub start_time: SystemTime,
}

impl AppState {
    pub async fn new() -> Result<Arc<Self>> {
        let config = CONFIG_MANAGER.get_config().await;
        let (_, stream_config) = CONFIG_MANAGER.get_audio_config().await?;
        let buffer_size = config.read().await.buffer_duration as usize * stream_config.sample_rate.0 as usize;

        let app_state = Arc::new(AppState {
            audio_buffer: Arc::new(RwLock::new(CircularAudioBuffer::new(buffer_size, stream_config.sample_rate.0))),
            audio_sender: broadcast::channel(1024).0,
            start_time: SystemTime::now(),
        });

        Ok(app_state)
    }

    pub fn update_sample_rate(&self, new_sample_rate: u32) -> Result<()> {
        let mut audio_buffer = self.audio_buffer.write().map_err(|_| LifeLoggingError::AudioDeviceError("Failed to acquire write lock on audio buffer".to_string()))?;
        let old_sample_rate = audio_buffer.sample_rate;
        let old_capacity = audio_buffer.capacity;

        if old_sample_rate == new_sample_rate {
            return Ok(());
        }

        let new_capacity = (old_capacity as f32 * new_sample_rate as f32 / old_sample_rate as f32).ceil() as usize;
        let mut new_buffer = vec![0.0; new_capacity];

        // Resample the existing data
        for i in 0..new_capacity {
            let old_index = i as f32 * old_sample_rate as f32 / new_sample_rate as f32;
            let old_index_floor = old_index.floor() as usize;
            let old_index_ceil = (old_index.ceil() as usize).min(old_capacity - 1);
            let frac = old_index - old_index.floor();

            let old_pos1 = (audio_buffer.write_position + old_index_floor) % old_capacity;
            let old_pos2 = (audio_buffer.write_position + old_index_ceil) % old_capacity;

            new_buffer[i] = audio_buffer.buffer[old_pos1] * (1.0 - frac) + audio_buffer.buffer[old_pos2] * frac;
        }

        audio_buffer.buffer = new_buffer;
        audio_buffer.write_position = 0;
        audio_buffer.capacity = new_capacity;
        audio_buffer.sample_rate = new_sample_rate;

        info!("Updated sample rate to {} Hz, new capacity: {} samples", new_sample_rate, new_capacity);

        Ok(())
    }
}
