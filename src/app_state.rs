use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::broadcast;
use crate::audio::buffer::CircularAudioBuffer;
use crate::config::Config;
use crate::error::Result;
use crate::audio::encoder::EncoderFactory;

pub struct AppState {
    pub audio_buffer: Arc<CircularAudioBuffer>,
    pub audio_sender: broadcast::Sender<Vec<f32>>,
    pub start_time: SystemTime,
    pub config: Arc<Config>,
    pub encoder_factory: EncoderFactory,
}

impl AppState {
    pub fn new(config: &Arc<Config>) -> Result<Arc<Self>> {
        let buffer_size = config.sample_rate as usize * config.buffer_duration as usize;
        let audio_buffer = Arc::new(CircularAudioBuffer::new(buffer_size, config.sample_rate));
        let (audio_sender, _) = broadcast::channel(1024);

        let app_state = Arc::new(AppState {
            audio_buffer: audio_buffer.clone(),
            audio_sender,
            start_time: SystemTime::now(),
            config: config.clone(),
            encoder_factory: EncoderFactory::new(),
        });

        // Initialize buffer with silence
        let silence = vec![0.0; buffer_size];
        app_state.audio_buffer.write(&silence);

        Ok(app_state)
    }
}
