use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::broadcast;
use crate::audio::buffer::CircularAudioBuffer;
use crate::config::ConfigManager;
use crate::error::Result;
use crate::audio::encoder::EncoderFactory;

pub struct AppState {
    pub audio_buffer: Arc<CircularAudioBuffer>,
    pub audio_sender: broadcast::Sender<Vec<f32>>,
    pub start_time: SystemTime,
    pub config_manager: Arc<ConfigManager>,
    pub encoder_factory: EncoderFactory,
}

impl AppState {
    pub async fn new(config_manager: &Arc<ConfigManager>) -> Result<Arc<Self>> {
        let config = config_manager.get_config().await;
        let (_, stream_config) = config_manager.get_audio_config().await?;
        let buffer_size = config.buffer_duration as usize * stream_config.sample_rate.0 as usize;
        let audio_buffer = Arc::new(CircularAudioBuffer::new(buffer_size, stream_config.sample_rate.0));
        let buffer_size = config.audio_channel_buffer_size.unwrap_or(1024);
        let (audio_sender, _) = broadcast::channel(buffer_size);

        let app_state = Arc::new(AppState {
            audio_buffer: audio_buffer.clone(),
            audio_sender,
            start_time: SystemTime::now(),
            config_manager: config_manager.clone(),
            encoder_factory: EncoderFactory::new(),
        });

        // Initialize buffer with silence
        let silence = vec![0.0; buffer_size];
        app_state.audio_buffer.write(&silence);

        Ok(app_state)
    }
}
