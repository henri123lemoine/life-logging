use crate::prelude::*;

mod app_state;
mod audio;
mod config;
mod error;
mod persistence;
mod prelude;
mod server;

use app_state::AppState;
use audio::processor;
use std::sync::Arc;

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    tracing::info!("Starting Life-Logging audio recording service");

    let app_state = Arc::new(AppState::new().await?);

    // Setup audio processing
    let audio_processor_app_state = app_state.clone();
    processor::setup_audio_processing(audio_processor_app_state).await?;

    // Start the persistence task
    let persistence_app_state = app_state.clone();
    tokio::spawn(async move {
        let storage_manager = persistence_app_state.storage_manager.clone();
        let audio_buffer = persistence_app_state.audio_buffer.clone();
        storage_manager.start_persistence_task(audio_buffer).await;
    });

    // Start the server
    let server_app_state = app_state.clone();
    server::run_server(server_app_state).await?;

    Ok(())
}
