use cpal::Stream;
use cpal::traits::{DeviceTrait, StreamTrait};
use std::sync::Arc;
use std::time::Duration;
use tokio::task;
use tokio::sync::{broadcast, mpsc};
use crate::app_state::AppState;
use crate::audio::buffer::CircularAudioBuffer;
use crate::config::CONFIG_MANAGER;
use crate::error::Result;

pub async fn setup_audio_processing(app_state: &Arc<AppState>) -> Result<()> {
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

async fn audio_processing_task(
    audio_buffer: Arc<CircularAudioBuffer>,
    audio_receiver: &mut broadcast::Receiver<Vec<f32>>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    loop {
        tokio::select! {
            _ = interval.tick() => {
                // Process audio data or do whatever every 5 seconds (general template)
                // I leave this here as a placeholder for any processing that needs to be done later
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
        let stream = match task::block_in_place(|| {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async {
                    start_audio_stream(&app_state, tx).await
                })
        }) {
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

async fn start_audio_stream(app_state: &Arc<AppState>, tx: mpsc::Sender<()>) -> Result<Stream> {
    let (device, config) = CONFIG_MANAGER.get_audio_config().await?;
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

pub fn normalize_volume(data: &mut [f32], target_peak: f32) -> Result<()> {
    if data.is_empty() {
        return Ok(());
    }

    let max_amplitude = data.iter().map(|&x| x.abs()).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap();
    
    if max_amplitude == 0.0 {
        return Ok(());
    }

    let scale_factor = target_peak / max_amplitude;

    for sample in data.iter_mut() {
        *sample *= scale_factor;
    }

    Ok(())
}
