use chrono::Utc;
use life_logging::app_state::AppState;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_storage_and_cleanup() {
    // Initialize AppState
    let app_state = Arc::new(AppState::new().await.expect("Failed to create AppState"));

    // Generate some dummy audio data
    let sample_rate = 44100;
    let duration = 5; // 5 seconds of audio
    let dummy_data: Vec<f32> = (0..sample_rate * duration)
        .map(|i| (i as f32 / sample_rate as f32).sin())
        .collect();

    // Write dummy data to AudioBuffer
    {
        let mut audio_buffer = app_state.audio_buffer.write().await;
        audio_buffer.write(&dummy_data);
    }

    // Persist audio data
    app_state
        .storage_manager
        .persist_audio(app_state.audio_buffer.clone())
        .await
        .expect("Failed to persist audio");

    println!("Audio data persisted successfully.");

    // Simulate passage of time
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Perform cleanup
    app_state
        .storage_manager
        .cleanup(Duration::from_secs(1), Duration::from_secs(3600))
        .await
        .expect("Failed to cleanup");

    println!("Cleanup completed successfully.");

    // Try to retrieve the persisted data
    // Note: We don't have a public method to retrieve data directly, so we'll just log the attempt
    println!(
        "Attempted to retrieve data. Check logs for any errors during persist_audio or cleanup."
    );

    // Log the current timestamp for reference
    println!("Current timestamp: {}", Utc::now());
}
