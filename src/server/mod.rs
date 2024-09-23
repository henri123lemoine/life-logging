mod handlers;
mod routes;

use crate::app_state::AppState;
use crate::config::CONFIG_MANAGER;
use crate::error::{LifeLoggingError, Result};
use std::net::SocketAddr;
use std::sync::Arc;

pub async fn run_server(app_state: &Arc<AppState>) -> Result<()> {
    let app: axum::Router = routes::create_router(app_state.clone());

    let config = CONFIG_MANAGER.get_config().await;
    let config = config.read().await;
    let addr = SocketAddr::new(
        config
            .server
            .host
            .parse()
            .map_err(|e| LifeLoggingError::Server(format!("Invalid host: {}", e)))?,
        config.server.port,
    );
    tracing::info!("Listening on {}", addr);
    axum_server::bind(addr)
        .serve(app.into_make_service())
        .await
        .map_err(|e| LifeLoggingError::Server(format!("Server error: {}", e)))?;

    Ok(())
}
