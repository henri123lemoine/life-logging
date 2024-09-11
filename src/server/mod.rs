mod routes;
mod handlers;

use std::sync::Arc;
use std::net::SocketAddr;
use crate::app_state::AppState;

pub async fn run_server(app_state: &Arc<AppState>) -> Result<(), Box<dyn std::error::Error>> {
    let app = routes::create_router(app_state.clone());

    let addr = SocketAddr::new(
        app_state.config.server.host.parse()?,
        app_state.config.server.port
    );
    tracing::info!("Listening on {}", addr);
    axum_server::bind(addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
