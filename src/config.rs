use crate::error::{AudioError, ConfigError, Result};
use config::{Config as ConfigSource, Environment, File};
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::StreamConfig;
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub buffer_duration: u64,
    pub server: ServerSettings,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
}

pub static CONFIG_MANAGER: Lazy<ConfigManager> =
    Lazy::new(|| ConfigManager::new().expect("Failed to initialize ConfigManager"));

pub struct ConfigManager {
    config: Arc<RwLock<Config>>,
}

impl ConfigManager {
    fn new() -> Result<Self> {
        let env = std::env::var("RUN_MODE").unwrap_or_else(|_| "development".into());
        let config_source = ConfigSource::builder()
            .add_source(File::with_name("config/default").required(false))
            .add_source(File::with_name(&format!("config/{}", env)).required(false))
            .add_source(Environment::with_prefix("LIFELOGGING").separator("__"))
            .build()
            .map_err(|e| ConfigError::File(e.to_string()))?;

        let config = config_source
            .clone()
            .try_deserialize()
            .map_err(ConfigError::Parse)?;

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
        })
    }

    pub async fn get_config(&self) -> Arc<RwLock<Config>> {
        self.config.clone()
    }

    pub async fn get_audio_config(&self) -> Result<(cpal::Device, StreamConfig)> {
        let host = cpal::default_host();
        self.find_working_device_and_config(&host).await
    }

    async fn find_working_device_and_config(
        &self,
        host: &cpal::Host,
    ) -> Result<(cpal::Device, StreamConfig)> {
        let devices = host.input_devices()?;

        for device in devices {
            let name = device.name()?;
            info!("Checking device: {}", name);

            match self.find_supported_config(&device).await {
                Ok(stream_config) => {
                    info!(
                        "Found working config for device {}: {:?}",
                        name, stream_config
                    );
                    return Ok((device, stream_config));
                }
                Err(e) => {
                    warn!("Config not supported for device {}: {}", name, e);
                    continue;
                }
            }
        }

        Err(
            AudioError::Device("No working audio input device and configuration found".into())
                .into(),
        )
    }

    async fn find_supported_config(&self, device: &cpal::Device) -> Result<StreamConfig> {
        let supported_configs = device.supported_input_configs()?;

        for config_range in supported_configs {
            let config = config_range.with_max_sample_rate();
            info!("Trying config: {:?}", config);

            // Check if the config is supported
            if device
                .default_input_config()
                .map(|c| c.sample_rate().0)
                .unwrap_or(0)
                == config.sample_rate().0
            {
                return Ok(config.into());
            }
        }

        // Fallback to default config if no matching config found
        device
            .default_input_config()
            .map(|c| c.into())
            .map_err(|e| AudioError::Device(format!("No supported config found: {}", e)).into())
    }
}
