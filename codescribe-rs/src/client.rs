//! HTTP client for communicating with CodeScribe Python backend (FastAPI + MLX Whisper)
//! or external WhisperX servers.
//!
//! Features:
//! - Automatic server discovery across multiple ports
//! - Support for external WhisperX servers (8443, 8444, 8445)
//! - Health checks with caching
//! - Multipart file upload for transcription
//! - Retry logic with exponential backoff
//! - Proper error handling and logging

use anyhow::{Context, Result};
use reqwest::multipart::{Form, Part};
use reqwest::Client;
use serde::Deserialize;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tracing::{debug, error, info, warn};

/// Cached server URL after successful discovery
static SERVER_URL: OnceLock<String> = OnceLock::new();

/// Ports to probe for backend server (in order of preference)
/// 8237 is the default Python backend port
const PROBE_PORTS: &[u16] = &[8237, 8238, 7237, 6237, 5237];

// Note: Retry constants and format_text moved to ai_formatting.rs module

/// Health check response structure
#[derive(Debug, Deserialize)]
struct HealthResponse {
    ok: bool,
}

/// Transcription response structure
#[derive(Debug, Deserialize)]
struct TranscribeResponse {
    text: String,
}

/// Get or create HTTP client with sensible defaults
fn get_client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(120)) // Long timeout for transcription
            .connect_timeout(Duration::from_secs(5))
            .pool_idle_timeout(Duration::from_secs(90))
            .build()
            .expect("Failed to create HTTP client")
    })
}

/// Discover backend server by probing known ports
///
/// Tries ports in order: 8237, 8238, 7237, 6237, 5237
/// Returns the first responding server URL or None
///
/// Retries each port up to 5 times with 500ms delay to handle race conditions
/// where backend just started but isn't fully accepting connections yet.
async fn discover_server() -> Option<String> {
    let client = get_client();
    const RETRIES_PER_PORT: u32 = 5;
    const RETRY_DELAY_MS: u64 = 500;

    for port in PROBE_PORTS {
        for attempt in 1..=RETRIES_PER_PORT {
            let url = format!("http://127.0.0.1:{}/healthz", port);
            debug!(
                "Probing server at {} (attempt {}/{})",
                url, attempt, RETRIES_PER_PORT
            );

            match client.get(&url).send().await {
                Ok(response) if response.status().is_success() => {
                    // Server is responding - accept even if model not loaded yet
                    // (ok=false means server running but model initializing)
                    if let Ok(health) = response.json::<HealthResponse>().await {
                        let base_url = format!("http://127.0.0.1:{}", port);
                        if health.ok {
                            info!(
                                "Discovered backend server at {} (fully ready, attempt {})",
                                base_url, attempt
                            );
                        } else {
                            info!(
                                "Discovered backend server at {} (model loading, attempt {})",
                                base_url, attempt
                            );
                        }
                        return Some(base_url);
                    }
                }
                Ok(response) => {
                    debug!(
                        "Port {} responded with status {} (attempt {})",
                        port,
                        response.status(),
                        attempt
                    );
                }
                Err(e) => {
                    debug!("Port {} not responding: {} (attempt {})", port, e, attempt);
                }
            }

            // Retry with delay (except on last attempt)
            if attempt < RETRIES_PER_PORT {
                tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
            }
        }
    }

    warn!(
        "No backend server found on any probe port after {} retries per port",
        RETRIES_PER_PORT
    );
    None
}

/// Get base server URL (cached or discovered)
async fn get_server_url() -> Result<String> {
    // Check cache first
    if let Some(url) = SERVER_URL.get() {
        return Ok(url.clone());
    }

    // Discover and cache
    let url = discover_server()
        .await
        .context("Backend server not found - ensure Python backend is running")?;

    // Try to cache (ignore if already set by another thread)
    let _ = SERVER_URL.set(url.clone());

    Ok(url)
}

/// Check if backend is healthy
///
/// Returns:
/// - `Ok(true)` if backend responds with {"ok": true} (model loaded)
/// - `Ok(false)` if backend responds but model still loading
/// - `Err(_)` if cannot connect or parse response
pub async fn check_health() -> Result<bool> {
    let base_url = get_server_url().await?;
    let url = format!("{}/healthz", base_url);

    // Use short timeout for health check to avoid stale connections
    let response = get_client()
        .get(&url)
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .context("Failed to send health check request")?;

    if !response.status().is_success() {
        return Ok(false);
    }

    let health: HealthResponse = response
        .json()
        .await
        .context("Failed to parse health check response")?;

    if !health.ok {
        info!("Backend responding but model still loading");
    }

    Ok(health.ok)
}

/// Transcribe audio file using backend STT service
///
/// # Arguments
/// * `path` - Path to audio file (WAV, MP3, M4A, etc.)
/// * `language` - Optional language code (e.g., "pl", "en"). If None, auto-detect.
///
/// # Returns
/// Transcribed text or error
///
/// # Example
/// ```no_run
/// use std::path::Path;
///
/// let transcript = client::transcribe(Path::new("recording.wav"), Some("pl")).await?;
/// println!("Transcript: {}", transcript);
/// ```
pub async fn transcribe(path: &Path, language: Option<&str>) -> Result<String> {
    // Check if Libraxis Cloud STT is configured via WHISPER_SERVER_URL
    // Format: https://api.libraxis.cloud (requires VOICE_API_KEY)
    let external_url = std::env::var("WHISPER_SERVER_URL").ok();
    let api_key = std::env::var("VOICE_API_KEY").ok();

    let is_libraxis_cloud = external_url
        .as_ref()
        .is_some_and(|url| url.contains("libraxis.cloud") || url.contains("api.libraxis"));

    if is_libraxis_cloud {
        let base_url = external_url.unwrap();
        let key = api_key.context("VOICE_API_KEY required for Libraxis Cloud STT")?;
        return transcribe_libraxis_cloud(path, language, &base_url, &key).await;
    }

    // Local Python backend uses /transcribe with "audio" field
    let base_url = get_server_url().await?;
    let url = format!("{}/transcribe", base_url);
    let field_name = "audio";

    // Read file into memory (path comes from internal recorder, not user input)
    let mut file = File::open(path) // nosemgrep: tainted-path
        .await
        .context("Failed to open audio file")?;

    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .await
        .context("Failed to read audio file")?;

    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("recording.wav");

    // Build multipart form
    let file_part = Part::bytes(buffer)
        .file_name(filename.to_string())
        .mime_str("audio/wav")
        .context("Failed to set MIME type")?;

    let mut form = Form::new().part(field_name, file_part);

    if let Some(lang) = language {
        form = form.text("language", lang.to_string());
    }

    info!("Sending transcription request to {} for {}", url, filename);

    // Single request (Form cannot be cloned for retry)
    let response = match get_client().post(&url).multipart(form).send().await {
        Ok(r) => r,
        Err(e) => {
            error!("HTTP request failed: {:?}", e);
            anyhow::bail!("Failed to send transcription request: {}", e);
        }
    };

    let status = response.status();
    info!("Transcription response status: {}", status);

    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "(no body)".to_string());
        error!("Transcription failed - status: {}, body: {}", status, body);
        anyhow::bail!("Transcription failed with status {}: {}", status, body);
    }

    let transcribe_response: TranscribeResponse = response
        .json()
        .await
        .context("Failed to parse transcription response")?;

    info!(
        "Transcription successful, length: {} chars",
        transcribe_response.text.len()
    );

    Ok(transcribe_response.text)
}

/// Transcribe audio using Libraxis Cloud API (OpenAI-compatible endpoint)
///
/// Uses /v1/audio/transcriptions endpoint with x-api-key header
async fn transcribe_libraxis_cloud(
    path: &Path,
    language: Option<&str>,
    base_url: &str,
    api_key: &str,
) -> Result<String> {
    info!("Using Libraxis Cloud STT: {}", base_url);

    let url = format!("{}/v1/audio/transcriptions", base_url.trim_end_matches('/'));

    // Read file into memory
    let mut file = File::open(path)
        .await
        .context("Failed to open audio file")?;

    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .await
        .context("Failed to read audio file")?;

    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("recording.wav");

    // Build multipart form (OpenAI-compatible format)
    let file_part = Part::bytes(buffer)
        .file_name(filename.to_string())
        .mime_str("audio/wav")
        .context("Failed to set MIME type")?;

    let mut form = Form::new()
        .part("file", file_part)
        .text("model", "whisper-large-v3");

    if let Some(lang) = language {
        form = form.text("language", lang.to_string());
    }

    debug!(
        "Sending transcription request to Libraxis Cloud for {}",
        filename
    );

    let response = get_client()
        .post(&url)
        .header("x-api-key", api_key)
        .multipart(form)
        .send()
        .await
        .context("Failed to send transcription request to Libraxis Cloud")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "(no body)".to_string());
        anyhow::bail!(
            "Libraxis Cloud transcription failed with status {}: {}",
            status,
            body
        );
    }

    // Libraxis Cloud returns OpenAI-compatible response
    let transcribe_response: TranscribeResponse = response
        .json()
        .await
        .context("Failed to parse Libraxis Cloud transcription response")?;

    info!(
        "Libraxis Cloud transcription successful, length: {} chars",
        transcribe_response.text.len()
    );

    Ok(transcribe_response.text)
}

// Note: format_text moved to ai_formatting.rs module for OpenAI/Libraxis support

/// Model set response structure
#[derive(Debug, Deserialize)]
struct ModelSetResponse {
    ok: bool,
    #[serde(default)]
    variant: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

/// Get current Whisper model variant from backend
pub async fn get_current_model() -> Result<String> {
    let base_url = get_server_url().await?;
    let url = format!("{}/model", base_url);

    let response = get_client()
        .get(&url)
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .context("Failed to get current model")?;

    #[derive(Deserialize)]
    struct ModelInfo {
        variant: String,
    }

    let info: ModelInfo = response
        .json()
        .await
        .context("Failed to parse model info")?;
    Ok(info.variant)
}

/// Set Whisper model variant
///
/// # Arguments
/// * `variant` - Model variant (small, medium, large-v3, large-v3-turbo)
///
/// # Returns
/// Ok(()) on success, error if model not found or switch failed
pub async fn set_whisper_model(variant: &str) -> Result<()> {
    let base_url = get_server_url().await?;
    let url = format!("{}/model/set", base_url);

    debug!("Setting Whisper model to: {}", variant);

    let response = get_client()
        .post(&url)
        .json(&serde_json::json!({ "variant": variant }))
        .send()
        .await
        .context("Failed to send model set request")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "(no body)".to_string());
        anyhow::bail!("Model set request failed with status {}: {}", status, body);
    }

    let set_response: ModelSetResponse = response
        .json()
        .await
        .context("Failed to parse model set response")?;

    if !set_response.ok {
        anyhow::bail!(
            "Failed to set model: {}",
            set_response
                .error
                .unwrap_or_else(|| "unknown error".to_string())
        );
    }

    info!(
        "Whisper model switched to: {} at {:?}",
        set_response.variant.unwrap_or_default(),
        set_response.path
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check() {
        // This test requires backend to be running
        match check_health().await {
            Ok(healthy) => println!("Backend health: {}", healthy),
            Err(e) => println!("Backend not available: {}", e),
        }
    }

    #[tokio::test]
    async fn test_server_discovery() {
        match discover_server().await {
            Some(url) => println!("Discovered server: {}", url),
            None => println!("No server found"),
        }
    }
}
