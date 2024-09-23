use life_logging::app_state::AppState;
use life_logging::audio::processor;
use life_logging::error::Result;
use life_logging::server;

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    tracing::info!("Starting Life-Logging audio recording service");

    let app_state = AppState::new().await?;
    processor::setup_audio_processing(&app_state).await?;

    server::run_server(&app_state).await?;
    Ok(())
}
