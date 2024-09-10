use serde::Deserialize;
use config::{Config, ConfigError, File, FileFormat};

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub sample_rate: u32,
    pub buffer_duration: u64,
    pub server: ServerSettings,
}

#[derive(Debug, Deserialize)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let env = std::env::var("RUN_MODE").unwrap_or_else(|_| "development".into());

        let s = Config::builder()
            // Start off by merging in the "default" configuration file
            .add_source(File::new("config/default", FileFormat::Toml))
            // Add in the current environment file
            // Default to 'development' env
            // Note that this file is _optional_
            .add_source(File::new(&format!("config/{}", env), FileFormat::Toml).required(false))
            .build()?;

        // You can deserialize (and thus freeze) the entire configuration
        s.try_deserialize()
    }
}
