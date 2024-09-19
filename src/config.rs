use std::sync::Arc;
use config::{Config as ConfigSource, Environment, File};
use serde::Deserialize;
use tokio::sync::RwLock;
use once_cell::sync::Lazy;
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::StreamConfig;
use tracing::{info, warn};
use crate::error::{LifeLoggingError, Result};

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub buffer_duration: u64,
    pub server: ServerSettings,
    pub selected_device: Option<String>,
    pub audio_channel_buffer_size: Option<usize>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
}

pub static CONFIG_MANAGER: Lazy<ConfigManager> = Lazy::new(|| {
    ConfigManager::new().expect("Failed to initialize ConfigManager")
});

pub struct ConfigManager {
    config: Arc<RwLock<Config>>,
    config_source: ConfigSource,
}

impl ConfigManager {
    fn new() -> Result<Self> {
        let env = std::env::var("RUN_MODE").unwrap_or_else(|_| "development".into());
        let config_source = ConfigSource::builder()
            .add_source(File::with_name("config/default").required(false))
            .add_source(File::with_name(&format!("config/{}", env)).required(false))
            .add_source(Environment::with_prefix("LIFELOGGING").separator("__"))
            .build()?;

        let config = config_source.clone().try_deserialize()?;

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            config_source,
        })
    }

    pub async fn reload(&self) -> Result<()> {
        let new_config = self.config_source.clone().try_deserialize()?;
        let mut config = self.config.write().await;
        *config = new_config;
        info!("Configuration reloaded successfully");
        Ok(())
    }

    pub async fn get_config(&self) -> Arc<RwLock<Config>> {
        self.config.clone()
    }

    pub async fn get_audio_config(&self) -> Result<(cpal::Device, StreamConfig)> {
        let config = self.config.read().await;
        let host = cpal::default_host();
        self.find_working_device_and_config(&host, &config).await
    }

    async fn find_working_device_and_config(&self, host: &cpal::Host, config: &Config) -> Result<(cpal::Device, StreamConfig)> {
        let devices = host.input_devices()?;

        for device in devices {
            let name = device.name()?;
            info!("Checking device: {}", name);

            if let Some(ref selected) = config.selected_device {
                if &name != selected {
                    continue;
                }
            }

            match self.find_supported_config(&device).await {
                Ok(stream_config) => {
                    info!("Found working config for device {}: {:?}", name, stream_config);
                    return Ok((device, stream_config));
                }
                Err(e) => {
                    warn!("Config not supported for device {}: {}", name, e);
                    continue;
                }
            }
        }

        Err(LifeLoggingError::AudioDeviceError("No working audio input device and configuration found".into()))
    }

    async fn find_supported_config(&self, device: &cpal::Device) -> Result<StreamConfig> {
        let supported_configs = device.supported_input_configs()?;

        for config_range in supported_configs {
            let config = config_range.with_max_sample_rate();
            info!("Trying config: {:?}", config);

            // Check if the config is supported
            if device.default_input_config().map(|c| c.sample_rate().0).unwrap_or(0) == config.sample_rate().0 {
                return Ok(config.into());
            }
        }

        // Fallback to default config if no matching config found
        device.default_input_config()
            .map(|c| c.into())
            .map_err(|e| LifeLoggingError::AudioDeviceError(format!("No supported config found: {}", e)))
    }
}
