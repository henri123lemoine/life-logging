use cpal::Stream;
use cpal::traits::{DeviceTrait, StreamTrait};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};
use crate::app_state::AppState;
use crate::audio::buffer::CircularAudioBuffer;
use crate::error::Result;

pub fn setup_audio_processing(app_state: &Arc<AppState>) -> Result<()> {
    let audio_buffer = app_state.audio_buffer.clone();
    let audio_sender = app_state.audio_sender.clone();

    tokio::spawn({
        let audio_buffer = audio_buffer.clone();
        let mut audio_receiver = audio_sender.subscribe();
        async move {
            audio_processing_task(audio_buffer, &mut audio_receiver).await;
        }
    });

    let app_state_clone = app_state.clone();
    std::thread::spawn(move || {
        audio_stream_management_task(app_state_clone);
    });

    Ok(())
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
            }
            result = audio_receiver.recv() => {
                if let Ok(data) = result {
                    audio_buffer.write(&data);
                }
            }
        }
    }
}

fn audio_stream_management_task(app_state: Arc<AppState>) {
    loop {
        let (tx, mut rx) = mpsc::channel::<()>(1);
        let stream = match start_audio_stream(&app_state, tx) {
            Ok(stream) => stream,
            Err(e) => {
                tracing::error!("Failed to start audio stream: {}", e);
                std::thread::sleep(Duration::from_secs(5));
                continue;
            }
        };

        tracing::info!("Audio stream started successfully");

        // Play the stream
        if let Err(e) = stream.play() {
            tracing::error!("Failed to play audio stream: {}", e);
            std::thread::sleep(Duration::from_secs(5));
            continue;
        }

        // Wait for the stream to end or for an error
        rx.blocking_recv();
        
        tracing::warn!("Audio stream ended, attempting to restart");
        std::thread::sleep(Duration::from_secs(1));
    }
}

fn start_audio_stream(app_state: &Arc<AppState>, tx: mpsc::Sender<()>) -> Result<Stream> {
    let (device, config) = app_state.config.get_audio_config()?;
    let audio_sender = app_state.audio_sender.clone();

    let tx1 = tx.clone();
    let tx2 = tx.clone();

    let stream = device.build_input_stream(
        &config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            if let Err(e) = audio_sender.send(data.to_vec()) {
                tracing::warn!("Failed to send audio data: {}", e);
                let _ = tx1.try_send(());  // Notify that the stream has ended
            }
        },
        move |err| {
            tracing::error!("An error occurred on stream: {}", err);
            let _ = tx2.try_send(());  // Notify that the stream has ended
        },
        Some(Duration::from_secs(2))
    )?;

    tracing::info!("Audio stream created with sample rate: {}", config.sample_rate.0);

    Ok(stream)
}
