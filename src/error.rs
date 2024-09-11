use thiserror::Error;
use cpal::DevicesError;
use cpal::SupportedStreamConfigsError;
use cpal::DefaultStreamConfigError;

#[derive(Error, Debug)]
pub enum LifeLoggingError {
    #[error("Configuration error: {0}")]
    ConfigError(#[from] config::ConfigError),

    #[error("Audio device error: {0}")]
    AudioDeviceError(String),

    #[error("Audio stream error: {0}")]
    AudioStreamError(#[from] cpal::BuildStreamError),

    #[error("Audio stream play error: {0}")]
    AudioStreamPlayError(#[from] cpal::PlayStreamError),

    #[error("Encoding error: {0}")]
    EncodingError(String),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Server error: {0}")]
    ServerError(String),

    #[error("Axum error: {0}")]
    AxumError(#[from] axum::Error),

    #[error("Device enumeration error: {0}")]
    DevicesError(#[from] DevicesError),

    #[error("Supported stream configs error: {0}")]
    SupportedStreamConfigsError(#[from] SupportedStreamConfigsError),

    #[error("Default stream config error: {0}")]
    DefaultStreamConfigError(#[from] DefaultStreamConfigError),
}

pub type Result<T> = std::result::Result<T, LifeLoggingError>;
