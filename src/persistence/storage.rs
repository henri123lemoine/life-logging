use crate::prelude::*;
use chrono::{DateTime, Utc};
use std::time::Duration;

pub trait Storage: Send + Sync {
    async fn save(&self, data: &[u8], timestamp: DateTime<Utc>) -> Result<()>;
    async fn retrieve(&self, timestamp: DateTime<Utc>) -> Result<Vec<u8>>;
    async fn cleanup(&self, retention_period: Duration) -> Result<()>;
}
