use crate::app_state::AppState;
use crate::audio::buffer::AudioBuffer;
use crate::config::CONFIG_MANAGER;
use crate::prelude::*;
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::Stream;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{error, info, instrument, warn};

#[instrument(skip(app_state))]
pub async fn setup_audio_processing(app_state: Arc<AppState>) -> Result<()> {
    info!("Setting up audio processing");

    let audio_sender = app_state.audio_sender.clone();
    let audio_buffer = app_state.audio_buffer.clone();

    tokio::spawn(async move {
        let mut audio_receiver = audio_sender.subscribe();
        audio_processing_task(audio_buffer, &mut audio_receiver).await;
    });

    let app_state_clone = app_state.clone();
    std::thread::spawn(move || {
        audio_stream_management_task(app_state_clone);
    });

    Ok(())
}

#[instrument(skip(audio_buffer, audio_receiver))]
async fn audio_processing_task(
    audio_buffer: Arc<RwLock<AudioBuffer>>,
    audio_receiver: &mut broadcast::Receiver<Vec<f32>>,
) {
    info!("Starting audio processing task");

    while let Ok(data) = audio_receiver.recv().await {
        let mut buffer = audio_buffer.write().await;
        buffer.write(&data);
    }
}

#[instrument(skip(app_state))]
fn audio_stream_management_task(app_state: Arc<AppState>) {
    info!("Starting audio stream management task");

    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

    loop {
        let (tx, mut rx) = mpsc::channel::<()>(1);
        match rt.block_on(start_audio_stream(&app_state, tx)) {
            Ok((stream, new_sample_rate)) => {
                rt.block_on(async {
                    let mut audio_buffer = app_state.audio_buffer.write().await;
                    if let Err(e) = audio_buffer.update_sample_rate(new_sample_rate) {
                        error!("Failed to update sample rate: {}", e)
                    }
                });

                info!("Audio stream started successfully");

                if let Err(e) = stream.play() {
                    error!("Failed to play audio stream: {}", e);
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    continue;
                }

                // Wait for the stream to end or for an error
                rt.block_on(async { rx.recv().await });
            }
            Err(e) => {
                error!("Failed to start audio stream: {}", e);
                std::thread::sleep(std::time::Duration::from_secs(5));
                continue;
            }
        }

        warn!("Audio stream ended, attempting to restart");
        std::thread::sleep(std::time::Duration::from_secs(1));
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
                warn!("Failed to send audio data: {}", e);
                let _ = tx1.try_send(());
            }
        },
        move |err| {
            error!("An error occurred on stream: {}", err);
            let _ = tx2.try_send(());
        },
        Some(Duration::from_secs(2)),
    )?;

    info!(
        "Audio stream created with sample rate: {}",
        config.sample_rate.0
    );

    Ok((stream, config.sample_rate.0))
}
