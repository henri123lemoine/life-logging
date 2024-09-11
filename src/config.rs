use serde::Deserialize;
use config::{Config as ConfigSource, File, FileFormat};
use std::sync::Arc;
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{StreamConfig, Device, Host};
use tracing::{info, warn};
use crate::error::{LifeLoggingError, Result};

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
    pub fn new() -> Result<Arc<Self>> {
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

    pub fn get_audio_config(&self) -> Result<(Device, StreamConfig)> {
        let host = cpal::default_host();
        self.find_working_device_and_config(&host)
    }

    fn find_working_device_and_config(&self, host: &Host) -> Result<(Device, StreamConfig)> {
        let devices = host.input_devices()?;
        
        for device in devices {
            if let Ok(name) = device.name() {
                info!("Checking device: {}", name);
            }
            
            match self.find_supported_config(&device) {
                Ok(config) => {
                    info!("Found working config for device {:?}: {:?}", device.name(), config);
                    return Ok((device, config));
                }
                Err(e) => {
                    warn!("Config not supported for device {:?}: {}", device.name(), e);
                    continue;
                }
            }
        }
        
        Err(LifeLoggingError::AudioDeviceError("No working audio input device and configuration found".into()))
    }

    fn find_supported_config(&self, device: &Device) -> Result<StreamConfig> {
        let supported_configs_range = device.supported_input_configs()?;
        
        info!("Desired sample rate: {}", self.sample_rate);
        
        for range in supported_configs_range {
            info!("Checking range: {} - {}", range.min_sample_rate().0, range.max_sample_rate().0);
            if range.min_sample_rate().0 <= self.sample_rate && self.sample_rate <= range.max_sample_rate().0 {
                let config = range.with_sample_rate(cpal::SampleRate(self.sample_rate));
                info!("Found exact match for sample rate: {}", self.sample_rate);
                return Ok(config.config());
            }
        }
        
        warn!("No exact match found for sample rate: {}", self.sample_rate);
        // Fall back to the default config
        let default_config = device.default_input_config()?;
        info!("Using default config with sample rate: {}", default_config.sample_rate().0);
        Ok(default_config.config())
    }
}

pub fn load_config() -> Result<Arc<Config>> {
    Config::new()
}
