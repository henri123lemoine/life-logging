use thiserror::Error;

#[derive(Error, Debug)]
pub enum LifeLoggingError {
    #[error("Audio error: {0}")]
    Audio(#[from] AudioError),

    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("Server error: {0}")]
    Server(#[from] ServerError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Error, Debug)]
pub enum AudioError {
    #[error("Audio device error: {0}")]
    Device(String),

    #[error("Audio stream error: {0}")]
    Stream(#[from] cpal::BuildStreamError),

    #[error("Audio stream play error: {0}")]
    StreamPlay(#[from] cpal::PlayStreamError),

    #[error("Encoding error: {0}")]
    Encoding(String),

    #[error("Device enumeration error: {0}")]
    Devices(#[from] cpal::DevicesError),

    #[error("Supported stream configs error: {0}")]
    SupportedStreamConfigs(#[from] cpal::SupportedStreamConfigsError),

    #[error("Default stream config error: {0}")]
    DefaultStreamConfig(#[from] cpal::DefaultStreamConfigError),

    #[error("Device name error: {0}")]
    DeviceName(#[from] cpal::DeviceNameError),
}

#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Configuration file error: {0}")]
    File(String),

    #[error("Configuration parsing error: {0}")]
    Parse(#[from] config::ConfigError),

    #[error("Invalid configuration value: {0}")]
    InvalidValue(String),
}

#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum ServerError {
    #[error("Server initialization error: {0}")]
    Init(String),

    #[error("Route handling error: {0}")]
    RouteHandler(String),
}

pub type Result<T> = std::result::Result<T, LifeLoggingError>;

impl From<cpal::DevicesError> for LifeLoggingError {
    fn from(err: cpal::DevicesError) -> Self {
        LifeLoggingError::Audio(AudioError::Devices(err))
    }
}

impl From<cpal::SupportedStreamConfigsError> for LifeLoggingError {
    fn from(err: cpal::SupportedStreamConfigsError) -> Self {
        LifeLoggingError::Audio(AudioError::SupportedStreamConfigs(err))
    }
}

impl From<cpal::DefaultStreamConfigError> for LifeLoggingError {
    fn from(err: cpal::DefaultStreamConfigError) -> Self {
        LifeLoggingError::Audio(AudioError::DefaultStreamConfig(err))
    }
}

impl From<cpal::DeviceNameError> for LifeLoggingError {
    fn from(err: cpal::DeviceNameError) -> Self {
        LifeLoggingError::Audio(AudioError::DeviceName(err))
    }
}

impl From<cpal::BuildStreamError> for LifeLoggingError {
    fn from(err: cpal::BuildStreamError) -> Self {
        LifeLoggingError::Audio(AudioError::Stream(err))
    }
}
