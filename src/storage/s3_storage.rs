use crate::error::{S3Error, StorageError};
use crate::prelude::*;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::StorageClass;
use aws_sdk_s3::{config::Region, Client};
use chrono::{DateTime, Utc};
use chrono::{Datelike, Timelike};
use std::time::Duration;
use tracing::info;

use super::Storage;

pub struct S3Storage {
    client: Client,
    bucket: String,
    storage_path: String,
}

impl S3Storage {
    pub async fn new(region: String, bucket: String, storage_path: String) -> Result<Self> {
        let config = aws_config::defaults(aws_config::BehaviorVersion::v2024_03_28())
            .region(Region::new(region))
            .load()
            .await;
        let client = Client::new(&config);

        Ok(Self {
            client,
            bucket,
            storage_path,
        })
    }

    fn generate_key(&self, timestamp: &DateTime<Utc>) -> String {
        format!(
            "{}/{year:04}/{month:02}/{day:02}/audio_{hour:02}{minute:02}{second:02}.opus",
            self.storage_path,
            year = timestamp.year(),
            month = timestamp.month(),
            day = timestamp.day(),
            hour = timestamp.hour(),
            minute = timestamp.minute(),
            second = timestamp.second()
        )
    }
}

impl Storage for S3Storage {
    async fn save(&self, data: &[u8], timestamp: DateTime<Utc>) -> Result<()> {
        let key = self.generate_key(&timestamp);
        let body = ByteStream::from(data.to_vec());

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(body)
            .storage_class(StorageClass::GlacierIr)
            .send()
            .await
            .map_err(|e| StorageError::S3(S3Error::S3Upload(e.to_string())))?;

        info!("Uploaded audio data to S3: {}", key);
        Ok(())
    }

    async fn retrieve(&self, timestamp: DateTime<Utc>) -> Result<Vec<u8>> {
        let key = self.generate_key(&timestamp);

        let output = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| StorageError::S3(S3Error::S3Download(e.to_string())))?;

        let bytes = output
            .body
            .collect()
            .await
            .map_err(|e| StorageError::S3(S3Error::S3Download(e.to_string())))?;

        Ok(bytes.into_bytes().to_vec())
    }

    async fn cleanup(&self, _retention_period: Duration) -> Result<()> {
        // S3 cleanup is typically handled by lifecycle policies set on the bucket
        // This method could be used to manually delete old objects if needed
        Ok(())
    }
}
