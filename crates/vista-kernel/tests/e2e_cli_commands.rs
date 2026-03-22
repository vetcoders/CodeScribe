//! CLI handoff coverage after the package split.
//!
//! `vista-kernel` no longer ships the historical `codescribe` binary, so this
//! suite verifies the library-side contracts that the external CLI depends on:
//! config path bootstrap and the documented EventSink-based live pipeline.

use std::fs;
use std::path::PathBuf;

use serial_test::serial;
use tempfile::TempDir;
use vista_kernel::config::Config;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn test_vista_kernel_no_longer_embeds_cli_binary() {
    let legacy_bin = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bin/codescribe.rs");
    assert!(
        !legacy_bin.exists(),
        "vista-kernel should not pretend to own the legacy CLI binary anymore"
    );
}

#[test]
fn test_cli_live_contract_documented_as_event_sink_pipeline() {
    let doc = fs::read_to_string(repo_root().join("docs/architecture/ipc-event-stream.md"))
        .expect("read ipc event-stream doc");

    assert!(
        doc.contains("set_event_sink(Some"),
        "live CLI handoff should document EventSink wiring"
    );
    assert!(
        doc.contains("start_event_session"),
        "live CLI handoff should document session startup through start_event_session"
    );
}

#[test]
fn test_cli_docs_still_advertise_transcribe_and_config_entrypoints() {
    let readme = fs::read_to_string(repo_root().join("README.md")).expect("read README");

    assert!(
        readme.contains("codescribe --config"),
        "README should still advertise the config entrypoint used by the CLI package"
    );
    assert!(
        readme.contains("transcribe"),
        "README should still advertise transcription entrypoints"
    );
}

#[test]
#[serial]
fn test_cli_side_config_bootstrap_uses_config_dir_override() {
    let tmp = TempDir::new().expect("tempdir");
    unsafe {
        std::env::set_var("CODESCRIBE_DATA_DIR", tmp.path());
    }

    let config_dir = Config::config_dir();
    assert!(
        config_dir.exists(),
        "config dir should be created on demand"
    );
    assert!(
        config_dir.starts_with(
            tmp.path()
                .canonicalize()
                .unwrap_or_else(|_| tmp.path().to_path_buf())
        ),
        "config dir should respect CODESCRIBE_DATA_DIR override"
    );
}
