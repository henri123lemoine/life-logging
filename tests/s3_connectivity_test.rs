use chrono::Utc;
use dotenv::dotenv;
use life_logging::prelude::*;
use life_logging::storage::{S3Storage, Storage};
use std::env;

#[tokio::test]
async fn test_s3_connectivity() -> Result<()> {
    // Load the .env file
    dotenv().ok();

    let bucket = env::var("AWS_S3_BUCKET").expect("AWS_S3_BUCKET must be set");
    let region = env::var("AWS_REGION").expect("AWS_REGION must be set");

    println!("Using bucket: {}", bucket); // Debug print

    let s3_storage = S3Storage::new(region, bucket, "test".to_string())
        .await
        .expect("Failed to create S3Storage");

    let test_data = b"Hello, S3!";
    let timestamp = Utc::now();

    s3_storage.save(test_data, timestamp).await?;
    println!("Successfully saved test data to S3");

    let retrieved_data = s3_storage.retrieve(timestamp).await?;
    assert_eq!(
        test_data.to_vec(),
        retrieved_data,
        "Retrieved data doesn't match original"
    );
    println!("Successfully retrieved and verified test data from S3");

    Ok(())
}
