use std::array::TryFromSliceError;
use std::io;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    IO(#[from] std::io::Error),

    #[error("Audio error: {0}")]
    Audio(#[from] AudioError),

    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("Persistence error: {0}")]
    Storage(#[from] StorageError),

    #[error("Server error: {0}")]
    Server(#[from] ServerError),
}

#[derive(thiserror::Error, Debug)]
pub enum AudioError {
    #[error("Audio device error: {0}")]
    Device(String),

    #[error("Audio stream error: {0}")]
    Stream(#[from] cpal::BuildStreamError),

    #[error("Audio stream play error: {0}")]
    StreamPlay(#[from] cpal::PlayStreamError),

    #[error("Device enumeration error: {0}")]
    Devices(#[from] cpal::DevicesError),

    #[error("Supported stream configs error: {0}")]
    SupportedStreamConfigs(#[from] cpal::SupportedStreamConfigsError),

    #[error("Default stream config error: {0}")]
    DefaultStreamConfig(#[from] cpal::DefaultStreamConfigError),

    #[error("Device name error: {0}")]
    DeviceName(#[from] cpal::DeviceNameError),

    #[error("Failed to acquire read lock on audio buffer")]
    BufferLockAcquisition,

    #[error("Unsupported audio format: {0}")]
    UnsupportedFormat(String),

    #[error("Codec error: {0}")]
    Codec(#[from] CodecError),
}

#[derive(thiserror::Error, Debug)]
pub enum CodecError {
    #[error("Encoding error: {0}")]
    Encoding(String),

    #[error("Decoding error: {0}")]
    Decoding(String),

    #[error("Unsupported sample rate {0}Hz")]
    UnsupportedSampleRate(u32),

    #[error("Invalid data format: {0}")]
    InvalidFormat(String),

    #[error("Invalid data: {0}")]
    InvalidData(&'static str),

    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(&'static str),

    #[error("External command failed: {0}")]
    ExternalCommand(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error("Configuration file error: {0}")]
    File(String),

    #[error("Configuration parsing error: {0}")]
    Parse(#[from] config::ConfigError),

    #[error("Invalid configuration value: {0}")]
    InvalidValue(String),
}

#[derive(thiserror::Error, Debug)]
pub enum StorageError {
    #[error("Local storage error: {0}")]
    Local(#[from] LocalError),

    #[error("DB error: {0}")]
    DB(#[from] DBError),

    #[error("S3 error: {0}")]
    S3(#[from] S3Error),
}

#[derive(thiserror::Error, Debug)]
pub enum LocalError {
    #[error("Failed to create storage directory: {0}")]
    DirectoryCreation(io::Error),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Failed to read audio file: {0}")]
    FileRead(String),

    #[error("Failed to write audio data: {0}")]
    FileWrite(io::Error),

    #[error("File cleanup error: {0}")]
    FileCleanup(String),
}

#[derive(thiserror::Error, Debug)]
pub enum DBError {}

#[derive(thiserror::Error, Debug)]
pub enum S3Error {
    #[error("S3 config error: {0}")]
    S3Config(String),

    #[error("S3 upload error: {0}")]
    S3Upload(String),

    #[error("S3 download error: {0}")]
    S3Download(String),

    #[error("S3 operation timed out after {0:?}")]
    Timeout(std::time::Duration),

    #[error("S3 connection error: {0}")]
    ConnectionError(String),
}

#[derive(thiserror::Error, Debug)]
pub enum ServerError {
    #[error("Server initialization error: {0}")]
    Init(String),

    #[error("Route handling error: {0}")]
    RouteHandler(String),

    #[error("Invalid request: {0}")]
    BadRequest(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Internal server error: {0}")]
    Internal(String),
}

impl From<CodecError> for Error {
    fn from(err: CodecError) -> Self {
        Error::Audio(AudioError::Codec(err))
    }
}

impl From<cpal::DevicesError> for Error {
    fn from(err: cpal::DevicesError) -> Self {
        Error::Audio(AudioError::Devices(err))
    }
}

impl From<cpal::SupportedStreamConfigsError> for Error {
    fn from(err: cpal::SupportedStreamConfigsError) -> Self {
        Error::Audio(AudioError::SupportedStreamConfigs(err))
    }
}

impl From<cpal::DefaultStreamConfigError> for Error {
    fn from(err: cpal::DefaultStreamConfigError) -> Self {
        Error::Audio(AudioError::DefaultStreamConfig(err))
    }
}

impl From<cpal::DeviceNameError> for Error {
    fn from(err: cpal::DeviceNameError) -> Self {
        Error::Audio(AudioError::DeviceName(err))
    }
}

impl From<cpal::BuildStreamError> for Error {
    fn from(err: cpal::BuildStreamError) -> Self {
        Error::Audio(AudioError::Stream(err))
    }
}

impl From<TryFromSliceError> for Error {
    fn from(err: TryFromSliceError) -> Self {
        Error::Audio(AudioError::Codec(CodecError::InvalidData(
            "Invalid byte slice conversion",
        )))
    }
}
