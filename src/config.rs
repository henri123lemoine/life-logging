use serde::Deserialize;
use config::{Config as ConfigSource, ConfigError, File, FileFormat};
use std::sync::Arc;
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{StreamConfig, Device};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub sample_rate: u32,
    pub buffer_duration: u64,
    pub server: ServerSettings,
}

#[derive(Debug, Deserialize)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
}

impl Config {
    pub fn new() -> Result<Arc<Self>, ConfigError> {
        let env = std::env::var("RUN_MODE").unwrap_or_else(|_| "development".into());

        let s = ConfigSource::builder()
            .add_source(File::new("config/default", FileFormat::Toml))
            .add_source(File::new(&format!("config/{}", env), FileFormat::Toml).required(false))
            .build()?;

        let mut config: Self = s.try_deserialize()?;
        config.validate_and_fix();

        Ok(Arc::new(config))
    }

    fn validate_and_fix(&mut self) {
        if self.sample_rate == 0 {
            self.sample_rate = 48000;
        }
        if self.buffer_duration == 0 {
            self.buffer_duration = 60;
        }
        if self.server.host.is_empty() {
            self.server.host = "127.0.0.1".to_string();
        }
        if self.server.port == 0 {
            self.server.port = 3000;
        }
    }

    pub fn get_audio_config(&self) -> Result<(Device, StreamConfig), Box<dyn std::error::Error>> {
        let host = cpal::default_host();
        let device = host.default_input_device().ok_or("No input device available")?;
        let config = self.find_supported_config(&device)?;

        Ok((device, config))
    }

    fn find_supported_config(&self, device: &Device) -> Result<StreamConfig, Box<dyn std::error::Error>> {
        let mut supported_configs_range = device.supported_input_configs()?;
        let supported_config = supported_configs_range
            .find(|range| range.min_sample_rate().0 <= self.sample_rate && self.sample_rate <= range.max_sample_rate().0)
            .ok_or("No supported config found")?
            .with_sample_rate(cpal::SampleRate(self.sample_rate));

        Ok(supported_config.into())
    }
}

pub fn load_config() -> Result<Arc<Config>, ConfigError> {
    Config::new()
}
