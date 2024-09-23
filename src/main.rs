mod app_state;
mod audio;
mod config;
mod error;
mod server;

use app_state::AppState;
use audio::processor;
use error::Result;

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    tracing::info!("Starting Life-Logging audio recording service");

    let app_state = AppState::new().await?;
    processor::setup_audio_processing(&app_state).await?;

    server::run_server(&app_state).await?;
    Ok(())
}
