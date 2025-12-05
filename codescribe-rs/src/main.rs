//! CodeScribe - Speech-to-text tray app for macOS
//!
//! Rust frontend that communicates with Python backend (FastAPI + MLX Whisper)

mod audio;
mod client;
mod clipboard;
mod config;
mod controller;
mod hotkeys;
mod tray;

use anyhow::Result;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .compact()
        .init();

    info!("CodeScribe starting...");

    // Check if Python backend is running
    match client::check_health().await {
        Ok(true) => info!("Python backend is healthy"),
        Ok(false) => {
            info!("Python backend not responding - please start it with: ./CodeScribe start backend");
        }
        Err(e) => {
            info!("Could not reach backend: {}", e);
        }
    }

    // Run the tray application (blocking)
    tray::run()?;

    info!("CodeScribe shutting down...");
    Ok(())
}
