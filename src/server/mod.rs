mod routes;
mod handlers;

use std::sync::Arc;
use std::net::SocketAddr;
use crate::app_state::AppState;
use crate::error::{LifeLoggingError, Result};

pub async fn run_server(app_state: &Arc<AppState>) -> Result<()> {
    let app = routes::create_router(app_state.clone());

    let addr = SocketAddr::new(
        app_state.config.server.host.parse().map_err(|e| LifeLoggingError::ServerError(format!("Invalid host: {}", e)))?,
        app_state.config.server.port
    );
    tracing::info!("Listening on {}", addr);
    axum_server::bind(addr)
        .serve(app.into_make_service())
        .await
        .map_err(|e| LifeLoggingError::ServerError(format!("Server error: {}", e)))?;

    Ok(())
}
