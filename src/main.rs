mod audio;
mod config;
mod server;
mod app_state;
mod error;

use std::sync::Arc;
use app_state::AppState;
use audio::processor;
use error::Result;
use config::ConfigManager;

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    tracing::info!("Starting Life-Logging audio recording service");

    let config_manager = Arc::new(ConfigManager::new()?);
    let app_state = AppState::new(&config_manager).await?;
    processor::setup_audio_processing(&app_state).await?;
    
    server::run_server(&app_state).await?;
    Ok(())
}
