use super::handlers;
use crate::app_state::AppState;
use axum::{
    extract::Request,
    middleware::Next,
    response::Response,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use std::time::Instant;
use tracing::info;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::health_check,
        handlers::get_audio,
        handlers::visualize_audio,
        handlers::list_audio_devices,
        handlers::change_audio_device,
    ),
    components(
        schemas(handlers::ChangeDeviceRequest)
    ),
    tags(
        (name = "audio", description = "Audio retrieval and management endpoints"),
        (name = "system", description = "System health and management")
    )
)]
struct ApiDoc;

async fn logging_middleware(req: Request, next: Next) -> Response {
    let path = req.uri().path().to_owned();
    let method = req.method().clone();
    let start = Instant::now();

    info!("Request: {} {}", method, path);

    let response = next.run(req).await;

    let duration = start.elapsed();
    let status = response.status();

    info!(
        "Response: {} {} - status: {}, duration: {:?}",
        method, path, status, duration
    );

    response
}

pub fn create_router(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(|| async { "Audio Recording Server" }))
        .route("/health", get(handlers::health_check))
        .route("/get_audio", get(handlers::get_audio))
        .route("/visualize_audio", get(handlers::visualize_audio))
        .route("/list_devices", get(handlers::list_audio_devices))
        .route("/change_device", post(handlers::change_audio_device))
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(axum::middleware::from_fn(logging_middleware))
        .with_state(app_state)
}
