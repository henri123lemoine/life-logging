use crate::error::StorageError;
use crate::prelude::*;
use chrono::{DateTime, Utc};
use chrono::{Datelike, Timelike};
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{error, info};

use super::Storage;

pub struct LocalStorage {
    storage_path: PathBuf,
    format: String,
    local_files: Mutex<VecDeque<(DateTime<Utc>, PathBuf)>>,
}

impl LocalStorage {
    pub fn new(storage_path: PathBuf, format: String) -> Result<Self> {
        fs::create_dir_all(&storage_path).map_err(StorageError::DirectoryCreation)?;
        Ok(Self {
            storage_path,
            format,
            local_files: Mutex::new(VecDeque::new()),
        })
    }

    fn generate_filename(&self, timestamp: &DateTime<Utc>) -> String {
        format!(
            "audio_{year:04}{month:02}{day:02}_{hour:02}{minute:02}{second:02}.{ext}",
            year = timestamp.year(),
            month = timestamp.month(),
            day = timestamp.day(),
            hour = timestamp.hour(),
            minute = timestamp.minute(),
            second = timestamp.second(),
            ext = self.format
        )
    }
}

impl Storage for LocalStorage {
    async fn save(&self, data: &[u8], timestamp: DateTime<Utc>) -> Result<()> {
        let filename = self.generate_filename(&timestamp);
        let file_path = self.storage_path.join(filename);
        fs::write(&file_path, data).map_err(StorageError::FileWrite)?;

        let mut local_files = self.local_files.lock().await;
        local_files.push_back((timestamp, file_path.clone()));

        info!("Saved audio file locally: {:?}", file_path);
        Ok(())
    }

    async fn retrieve(&self, timestamp: DateTime<Utc>) -> Result<Vec<u8>> {
        let local_files = self.local_files.lock().await;
        let file_path = local_files
            .iter()
            .find(|(file_timestamp, _)| *file_timestamp <= timestamp)
            .map(|(_, path)| path.clone())
            .ok_or_else(|| StorageError::FileNotFound(timestamp.to_string()))?;

        fs::read(&file_path).map_err(|e| StorageError::FileRead(e.to_string()).into())
    }

    async fn cleanup(&self, retention_period: Duration) -> Result<()> {
        let mut local_files = self.local_files.lock().await;
        let cutoff = Utc::now() - chrono::Duration::from_std(retention_period).unwrap();

        local_files.retain(|(timestamp, path)| {
            if timestamp < &cutoff {
                if let Err(e) = fs::remove_file(path) {
                    error!("Failed to remove old local file: {:?}. Error: {}", path, e);
                    false
                } else {
                    info!("Removed old local file: {:?}", path);
                    false
                }
            } else {
                true
            }
        });

        Ok(())
    }
}
