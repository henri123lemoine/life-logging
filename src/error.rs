use thiserror::Error;

#[derive(Error, Debug)]
pub enum LifeLoggingError {
    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),

    #[error("Audio device error: {0}")]
    AudioDevice(String),

    #[error("Audio stream error: {0}")]
    AudioStream(#[from] cpal::BuildStreamError),

    #[error("Audio stream play error: {0}")]
    AudioStreamPlay(#[from] cpal::PlayStreamError),

    #[error("Encoding error: {0}")]
    Encoding(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Server error: {0}")]
    Server(String),

    #[error("Axum error: {0}")]
    Axum(#[from] axum::Error),

    #[error("Device enumeration error: {0}")]
    Devices(#[from] cpal::DevicesError),

    #[error("Supported stream configs error: {0}")]
    SupportedStreamConfigs(#[from] cpal::SupportedStreamConfigsError),

    #[error("Default stream config error: {0}")]
    DefaultStreamConfig(#[from] cpal::DefaultStreamConfigError),

    #[error("Device name error: {0}")]
    DeviceName(#[from] cpal::DeviceNameError),
}

pub type Result<T> = std::result::Result<T, LifeLoggingError>;
