//! Example: Basic audio recording usage
//!
//! Demonstrates how to use the audio::Recorder to record audio with silence detection.
//!
//! Run with: cargo run --example audio_example

use codescribe::audio::{Recorder, RecorderConfig};
use std::time::Duration;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .compact()
        .init();

    info!("=== CodeScribe Audio Recording Example ===\n");

    // Example 1: Default configuration with auto-silence detection
    info!("Example 1: Recording with default config (auto-silence enabled)");
    info!("Speak into your microphone... (will auto-stop after 0.8s of silence)\n");

    let mut recorder = Recorder::new()?;
    recorder.start().await?;

    // Wait for silence detection to stop recording (or timeout after 10s)
    tokio::time::sleep(Duration::from_secs(10)).await;

    if let Some(path) = recorder.stop().await? {
        info!("Recording saved to: {:?}", path);
        info!("Duration: {:.2}s", recorder.last_duration());
        info!("Diagnostics: {:?}\n", recorder.diagnostics());
    }

    // Example 2: Custom configuration with manual stop
    info!("\nExample 2: Recording with custom config (manual stop after 3s)");
    info!("Speak into your microphone...\n");

    let config = RecorderConfig {
        auto_silence: false, // Disable auto-silence
        ..Default::default()
    };

    let mut recorder = Recorder::with_config(config)?;
    recorder.start().await?;

    // Record for 3 seconds then stop manually
    tokio::time::sleep(Duration::from_secs(3)).await;

    if let Some(path) = recorder.stop().await? {
        info!("Recording saved to: {:?}", path);
        info!("Duration: {:.2}s", recorder.last_duration());
        info!("Diagnostics: {:?}\n", recorder.diagnostics());
    }

    // Example 3: Snapshot during recording
    info!("\nExample 3: Using snapshot_wav() for live streaming");
    info!("Speak into your microphone...\n");

    let mut recorder = Recorder::new()?;
    recorder.start().await?;

    // Wait 1 second then take a snapshot without stopping
    tokio::time::sleep(Duration::from_secs(1)).await;

    if let Some(snapshot_path) = recorder.snapshot_wav(0.5)? {
        info!("Snapshot saved to: {:?}", snapshot_path);
        info!("Recording is still active...");
    }

    // Wait another second then stop
    tokio::time::sleep(Duration::from_secs(1)).await;

    if let Some(path) = recorder.stop().await? {
        info!("Final recording saved to: {:?}", path);
        info!("Total duration: {:.2}s", recorder.last_duration());
    }

    info!("\n=== All examples completed ===");

    Ok(())
}
