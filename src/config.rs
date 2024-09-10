use serde::Deserialize;
use config::{Config, ConfigError, File, FileFormat};
use std::sync::Arc;
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{StreamConfig, Device};

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
    pub fn new() -> Result<Arc<Self>, ConfigError> {
        let env = std::env::var("RUN_MODE").unwrap_or_else(|_| "development".into());

        let s = Config::builder()
            .add_source(File::new("config/default", FileFormat::Toml))
            .add_source(File::new(&format!("config/{}", env), FileFormat::Toml).required(false))
            .build()?;

        let mut settings: Self = s.try_deserialize()?;
        settings.validate_and_fix();

        Ok(Arc::new(settings))
    }

    fn validate_and_fix(&mut self) {
        // Validate and fix sample rate
        if self.sample_rate == 0 {
            self.sample_rate = 48000; // Default to 48kHz if not set
        }

        // Validate and fix buffer duration
        if self.buffer_duration == 0 {
            self.buffer_duration = 60; // Default to 60 seconds if not set
        }

        // Validate and fix server settings
        if self.server.host.is_empty() {
            self.server.host = "127.0.0.1".to_string();
        }
        if self.server.port == 0 {
            self.server.port = 3000;
        }
    }
}

pub fn load_settings() -> Result<Arc<Settings>, ConfigError> {
    Settings::new()
}

pub fn get_audio_config(settings: &Settings) -> Result<(Device, StreamConfig), Box<dyn std::error::Error>> {
    let host = cpal::default_host();
    let device = host.default_input_device().ok_or("No input device available")?;
    let config = find_supported_config(&device, settings.sample_rate)?;

    Ok((device, config))
}

fn find_supported_config(device: &Device, desired_sample_rate: u32) -> Result<StreamConfig, Box<dyn std::error::Error>> {
    let mut supported_configs_range = device.supported_input_configs()?;
    let supported_config = supported_configs_range
        .find(|range| range.min_sample_rate().0 <= desired_sample_rate && desired_sample_rate <= range.max_sample_rate().0)
        .ok_or("No supported config found")?
        .with_sample_rate(cpal::SampleRate(desired_sample_rate));

    Ok(supported_config.into())
}
