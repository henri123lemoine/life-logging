mod local_storage;
mod s3_storage;
mod storage_manager;

pub use local_storage::LocalStorage;
pub use s3_storage::S3Storage;
pub use storage_manager::StorageManager;

use crate::prelude::*;
use chrono::{DateTime, Utc};
use std::time::Duration;

pub trait Storage: Send + Sync {
    async fn save(&self, data: &[u8], timestamp: DateTime<Utc>) -> Result<()>;
    async fn retrieve(&self, timestamp: DateTime<Utc>) -> Result<Vec<u8>>;
    async fn cleanup(&self, retention_period: Duration) -> Result<()>;
}
