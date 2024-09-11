use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use crate::app_state::AppState;
use crate::audio::buffer::CircularAudioBuffer;
use cpal::traits::{DeviceTrait, StreamTrait};

pub fn setup_audio_processing(app_state: &Arc<AppState>) {
    let audio_buffer = app_state.audio_buffer.clone();
    let mut audio_receiver = app_state.audio_sender.subscribe();

    tokio::spawn(async move {
        audio_processing_task(audio_buffer, &mut audio_receiver).await;
    });

    let (device, stream_config) = app_state.config.get_audio_config().expect("Failed to get audio config");
    start_audio_stream(device, stream_config, app_state.audio_sender.clone())
        .expect("Failed to start audio stream");
}

async fn audio_processing_task(
    audio_buffer: Arc<CircularAudioBuffer>,
    audio_receiver: &mut broadcast::Receiver<Vec<f32>>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        tokio::select! {
            _ = interval.tick() => {
                // Process audio data or do whatever every 5 seconds (general template)
                // This could involve writing to a file or other persistent storage
            }
            result = audio_receiver.recv() => {
                if let Ok(data) = result {
                    audio_buffer.write(&data);
                }
            }
        }
    }
}

fn start_audio_stream(device: cpal::Device, config: cpal::StreamConfig, audio_sender: broadcast::Sender<Vec<f32>>) -> Result<(), Box<dyn std::error::Error>> {
    let stream = device.build_input_stream(
        &config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            if !data.iter().any(|&sample| sample != 0.0) {
                tracing::debug!("Detected no audio input");
            }
            if let Err(e) = audio_sender.send(data.to_vec()) {
                tracing::warn!("Failed to send audio data: {}", e);
            }
        },
        |err| tracing::error!("An error occurred on stream: {}", err),
        Some(Duration::from_secs(2))
    )?;

    stream.play()?;
    tracing::info!("Audio stream started with sample rate: {}", config.sample_rate.0);

    // Keep the stream alive
    std::mem::forget(stream);

    Ok(())
}
