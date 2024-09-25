use life_logging::app_state::AppState;
use life_logging::audio::processor;
use life_logging::error::Result;
use life_logging::server;
use std::time::Duration;

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    tracing::info!("Starting Life-Logging audio recording service");

    let app_state = AppState::new().await?;
    processor::setup_audio_processing(&app_state).await?;

    // Start the persistence task
    tokio::spawn({
        let app_state = app_state.clone();
        async move {
            app_state
                .disk_storage
                .start_persistence_task(app_state.audio_buffer.clone())
                .await;
        }
    });

    // Start the cleanup task
    tokio::spawn({
        let disk_storage = app_state.disk_storage.clone();
        async move {
            disk_storage
                .start_cleanup_task(
                    Duration::from_secs(60 * 60),
                    Duration::from_secs(60 * 60 * 24),
                )
                .await;
        }
    });

    server::run_server(&app_state).await?;
    Ok(())
}
