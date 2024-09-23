use crate::app_state::AppState;
use crate::audio::buffer::CircularAudioBuffer;
use crate::config::CONFIG_MANAGER;
use crate::error::Result;
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::Stream;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};
use tokio::task;
use tracing::{info, instrument, warn};

#[instrument(skip(app_state))]
pub async fn setup_audio_processing(app_state: &Arc<AppState>) -> Result<()> {
    info!("Setting up audio processing");

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
    task::spawn_blocking(move || {
        audio_stream_management_task(app_state_clone);
    });

    Ok(())
}

#[instrument(skip(audio_buffer, audio_receiver))]
async fn audio_processing_task(
    audio_buffer: Arc<RwLock<CircularAudioBuffer>>,
    audio_receiver: &mut broadcast::Receiver<Vec<f32>>,
) {
    info!("Starting audio processing task");

    loop {
        tokio::select! {
            result = audio_receiver.recv() => {
                if let Ok(data) = result {
                    let mut buffer = audio_buffer.write().unwrap();
                    buffer.write(&data);
                }
            }
        }
    }
}

#[instrument(skip(app_state))]
fn audio_stream_management_task(app_state: Arc<AppState>) {
    info!("Starting audio stream management task");

    loop {
        let (tx, mut rx) = mpsc::channel::<()>(1);
        let stream = match task::block_in_place(|| {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async { start_audio_stream(&app_state, tx).await })
        }) {
            Ok((stream, new_sample_rate)) => {
                let buffer = app_state.audio_buffer.write().unwrap();
                if new_sample_rate != buffer.sample_rate {
                    info!(
                        "Sample rate changed from {} to {}",
                        buffer.sample_rate, new_sample_rate
                    );
                    drop(buffer); // Release the write lock
                    app_state.update_sample_rate(new_sample_rate).unwrap();
                }
                stream
            }
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

#[instrument(skip(app_state, tx))]
async fn start_audio_stream(
    app_state: &Arc<AppState>,
    tx: mpsc::Sender<()>,
) -> Result<(Stream, u32)> {
    info!("Starting audio stream");

    let (device, config) = CONFIG_MANAGER.get_audio_config().await?;
    let audio_sender = app_state.audio_sender.clone();

    let tx1 = tx.clone();
    let tx2 = tx.clone();

    let stream = device.build_input_stream(
        &config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            if let Err(e) = audio_sender.send(data.to_vec()) {
                tracing::warn!("Failed to send audio data: {}", e);
                let _ = tx1.try_send(());
            }
        },
        move |err| {
            tracing::error!("An error occurred on stream: {}", err);
            let _ = tx2.try_send(());
        },
        Some(Duration::from_secs(2)),
    )?;

    tracing::info!(
        "Audio stream created with sample rate: {}",
        config.sample_rate.0
    );

    Ok((stream, config.sample_rate.0))
}
