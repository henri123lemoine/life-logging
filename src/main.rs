mod audio;
mod config;
mod server;
mod app_state;

use config::load_config;
use app_state::AppState;
use server::run_server;
use audio::processor::setup_audio_processing;

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    tracing::info!("Starting Life-Logging audio recording service");

    let config = load_config()?;
    let app_state = AppState::new(&config)?;
    setup_audio_processing(&app_state);
    
    run_server(&app_state).await?;
    Ok(())
}
