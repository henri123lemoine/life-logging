mod audio;
mod config;
mod server;
mod app_state;
mod error;

use app_state::AppState;
use audio::processor;
use error::Result;

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    tracing::info!("Starting Life-Logging audio recording service");

    let config = config::load_config()?;
    let app_state = AppState::new(&config)?;
    processor::setup_audio_processing(&app_state)?;
    
    server::run_server(&app_state).await?;
    Ok(())
}
