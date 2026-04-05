//! Cloud transcription tests.
//!
//! This file contains:
//! - deterministic, offline contract tests (default path)
//! - one real-cloud opt-in E2E test (requires credentials)
//!
//! Real-cloud E2E enable:
//!   CODESCRIBE_E2E_CLOUD=1 STT_ENDPOINT=... STT_API_KEY=... cargo test --test cloud_transcribe_e2e

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use codescribe_core::pipeline::contracts::{DeltaSink, TranscriptDelta};
use codescribe_core::pipeline::sinks::CollectorSink;
use serial_test::serial;

fn write_min_valid_audio_file() -> tempfile::NamedTempFile {
    let mut audio = tempfile::NamedTempFile::new().expect("create temp audio file");
    // Must be > 1KB to pass `validate_audio`.
    audio
        .write_all(&vec![0xAB; 2048])
        .expect("write temp audio bytes");
    audio.flush().expect("flush temp audio");
    audio
}

fn write_min_valid_wav_file() -> tempfile::NamedTempFile {
    let mut audio = tempfile::NamedTempFile::new().expect("create temp wav file");
    let sample_rate = 16_000u32;
    let bits_per_sample = 16u16;
    let channels = 1u16;
    let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
    let block_align = channels * bits_per_sample / 8;
    let pcm_data = vec![0u8; 2048];
    let data_len = pcm_data.len() as u32;
    let riff_len = 36 + data_len;

    audio.write_all(b"RIFF").expect("write riff");
    audio
        .write_all(&riff_len.to_le_bytes())
        .expect("write riff len");
    audio.write_all(b"WAVE").expect("write wave");
    audio.write_all(b"fmt ").expect("write fmt tag");
    audio
        .write_all(&16u32.to_le_bytes())
        .expect("write fmt chunk len");
    audio
        .write_all(&1u16.to_le_bytes())
        .expect("write audio format");
    audio
        .write_all(&channels.to_le_bytes())
        .expect("write channels");
    audio
        .write_all(&sample_rate.to_le_bytes())
        .expect("write sample rate");
    audio
        .write_all(&byte_rate.to_le_bytes())
        .expect("write byte rate");
    audio
        .write_all(&block_align.to_le_bytes())
        .expect("write block align");
    audio
        .write_all(&bits_per_sample.to_le_bytes())
        .expect("write bits per sample");
    audio.write_all(b"data").expect("write data tag");
    audio
        .write_all(&data_len.to_le_bytes())
        .expect("write data len");
    audio.write_all(&pcm_data).expect("write pcm data");
    audio.flush().expect("flush temp wav");
    audio
}

#[tokio::test]
#[serial]
async fn contract_cloud_transcribe_success() {
    let mut server = mockito::Server::new_async().await;
    let endpoint = format!("{}/v1/audio/transcriptions", server.url());

    let success = server
        .mock("POST", "/v1/audio/transcriptions")
        .match_header("x-api-key", "test-key")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"text":"hello from cloud"}"#)
        .expect(1)
        .create_async()
        .await;

    let audio = write_min_valid_wav_file();
    let text =
        codescribe::client::transcribe_cloud(audio.path(), Some("en"), &endpoint, "test-key")
            .await
            .expect("cloud transcription should succeed");

    success.assert_async().await;
    assert_eq!(text, "hello from cloud");
}

#[tokio::test]
#[serial]
async fn contract_cloud_transcribe_auth_failure_is_not_retried() {
    let mut server = mockito::Server::new_async().await;
    let endpoint = format!("{}/v1/audio/transcriptions", server.url());

    let unauthorized = server
        .mock("POST", "/v1/audio/transcriptions")
        .with_status(401)
        .with_body("unauthorized")
        .expect(1)
        .create_async()
        .await;

    let audio = write_min_valid_wav_file();
    let err = codescribe::client::transcribe_cloud(audio.path(), Some("en"), &endpoint, "test-key")
        .await
        .expect_err("401 contract should fail");

    unauthorized.assert_async().await;
    let err_msg = format!("{:#}", err);
    assert!(
        err_msg.contains("status 401"),
        "expected auth status in error chain, got: {err_msg}"
    );
}

#[tokio::test]
#[serial]
async fn contract_cloud_transcribe_malformed_response_is_not_retried() {
    let mut server = mockito::Server::new_async().await;
    let endpoint = format!("{}/v1/audio/transcriptions", server.url());

    let malformed = server
        .mock("POST", "/v1/audio/transcriptions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"unexpected":"shape"}"#)
        .expect(1)
        .create_async()
        .await;

    let audio = write_min_valid_audio_file();
    let err = codescribe::client::transcribe_cloud(audio.path(), Some("en"), &endpoint, "test-key")
        .await
        .expect_err("malformed response should fail");

    malformed.assert_async().await;
    let err_msg = format!("{:#}", err);
    assert!(
        err_msg.contains("Failed to parse external STT transcription response"),
        "expected parse failure context, got: {err_msg}"
    );
}

#[tokio::test]
#[serial]
async fn contract_cloud_transcribe_retry_boundary_on_retryable_5xx() {
    let mut server = mockito::Server::new_async().await;
    let endpoint = format!("{}/v1/audio/transcriptions", server.url());

    let unavailable = server
        .mock("POST", "/v1/audio/transcriptions")
        .with_status(503)
        .with_body("temporarily unavailable")
        .expect(3)
        .create_async()
        .await;

    let audio = write_min_valid_audio_file();
    let err = codescribe::client::transcribe_cloud(audio.path(), Some("en"), &endpoint, "test-key")
        .await
        .expect_err("retry boundary should still fail on repeated 5xx");

    unavailable.assert_async().await;
    let err_msg = format!("{:#}", err);
    assert!(
        err_msg.contains("status 503"),
        "expected final retry error to include 503, got: {err_msg}"
    );
}

#[tokio::test]
#[serial]
async fn contract_cloud_transcribe_ndjson_preview_sink_streams_deltas() {
    let mut server = mockito::Server::new_async().await;
    let endpoint = format!("{}/v1/audio/transcriptions:stream", server.url());

    let stream = concat!(
        "data: {\"text\":\"hel\"}\n",
        "data: {\"text\":\"hello\"}\n",
        "data: {\"text\":\"hello, world\",\"is_final\":true}\n",
        "data: [DONE]\n"
    );

    let streaming = server
        .mock("POST", "/v1/audio/transcriptions:stream")
        .match_header("x-api-key", "test-key")
        .match_header("content-type", "application/x-ndjson")
        .with_status(200)
        .with_header("content-type", "application/x-ndjson")
        .with_body(stream)
        .expect(1)
        .create_async()
        .await;

    let audio = write_min_valid_wav_file();
    let collector = Arc::new(CollectorSink::new());
    let preview_sink = collector.clone() as Arc<dyn DeltaSink>;

    let text = codescribe::client::transcribe_cloud_with_preview_sink(
        audio.path(),
        Some("en"),
        &endpoint,
        "test-key",
        Some(preview_sink),
    )
    .await
    .expect("ndjson transcription with preview sink should succeed");

    streaming.assert_async().await;
    assert_eq!(text, "hello, world");

    let deltas = collector.collected();
    assert_eq!(deltas, vec!["hel", "lo", ", world"]);

    let mut preview = String::new();
    for delta in deltas {
        TranscriptDelta::from_raw(delta).apply(&mut preview);
    }
    assert_eq!(preview, "hello, world");
}

#[cfg(target_os = "macos")]
#[tokio::test]
#[serial]
async fn test_cloud_transcribe_e2e() {
    if !env_bool("CODESCRIBE_E2E_CLOUD") {
        eprintln!("Skipping cloud E2E (set CODESCRIBE_E2E_CLOUD=1 to enable)");
        return;
    }

    let endpoint = match std::env::var("STT_ENDPOINT") {
        Ok(val) if !val.trim().is_empty() => val,
        _ => {
            eprintln!("Skipping cloud E2E (STT_ENDPOINT missing)");
            return;
        }
    };
    let api_key = match std::env::var("STT_API_KEY") {
        Ok(val) if !val.trim().is_empty() => val,
        _ => {
            eprintln!("Skipping cloud E2E (STT_API_KEY missing)");
            return;
        }
    };

    let audio = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/assets/1.fretka-Ziggy.mp3");
    assert!(audio.exists(), "Missing test audio at {}", audio.display());

    let text = codescribe::client::transcribe_cloud(&audio, None, &endpoint, &api_key)
        .await
        .expect("Cloud transcription failed");
    assert!(
        !text.trim().is_empty(),
        "Cloud transcription returned empty text"
    );
}

fn env_bool(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}
