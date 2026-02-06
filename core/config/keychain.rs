//! macOS Keychain integration for API keys.
//!
//! Stores secrets in the system Keychain instead of plaintext .env files.

use anyhow::{Context, Result};
use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};
use tracing::{debug, info};

const SERVICE: &str = "com.vetcoders.codescribe";

/// Known API key accounts stored in Keychain.
pub const KEYCHAIN_ACCOUNTS: &[&str] = &[
    "LLM_API_KEY",
    "STT_API_KEY",
    "LLM_FORMATTING_API_KEY",
    "LLM_ASSISTIVE_API_KEY",
];

/// Returns true when running inside a test harness or when Keychain is explicitly disabled.
///
/// NOTE: `cfg!(test)` only works for **unit tests** within this crate.
/// Integration tests (`tests/`, `core/tests/`) compile the library normally,
/// so `cfg!(test)` returns false. Those tests MUST set `CODESCRIBE_DISABLE_KEYCHAIN=1`
/// or `CODESCRIBE_DATA_DIR` to skip Keychain. The Makefile `TEST_SETUP` sets the former.
fn is_test_env() -> bool {
    if cfg!(test) {
        return true;
    }
    if std::env::var_os("CODESCRIBE_DISABLE_KEYCHAIN").is_some() {
        return true;
    }
    if std::env::var_os("CI").is_some() {
        return true;
    }
    std::env::var("CODESCRIBE_DATA_DIR").is_ok()
}

/// Saves a secret to the macOS Keychain under the CodeScribe service.
/// In test environments, sets the env var directly instead of touching Keychain.
pub fn save_key(account: &str, secret: &str) -> Result<()> {
    if is_test_env() {
        debug!("Test env: skipping Keychain save for {account}");
        unsafe { std::env::set_var(account, secret) };
        return Ok(());
    }
    set_generic_password(SERVICE, account, secret.as_bytes())
        .with_context(|| format!("Failed to save Keychain entry for {account}"))?;
    info!("Saved {account} to Keychain");
    Ok(())
}

/// Loads a secret from the macOS Keychain. Returns `None` if not found.
pub fn load_key(account: &str) -> Option<String> {
    if is_test_env() {
        debug!("Test env: skipping Keychain load for {account}");
        return None;
    }
    match get_generic_password(SERVICE, account) {
        Ok(bytes) => match String::from_utf8(bytes.to_vec()) {
            Ok(s) => {
                debug!("Loaded {account} from Keychain");
                Some(s)
            }
            Err(e) => {
                debug!("Keychain value for {account} is not valid UTF-8: {e}");
                None
            }
        },
        Err(e) => {
            debug!("No Keychain entry for {account}: {e}");
            None
        }
    }
}

/// Deletes a secret from the macOS Keychain. Ignores "not found" errors.
pub fn delete_key(account: &str) -> Result<()> {
    if is_test_env() {
        debug!("Test env: skipping Keychain delete for {account}");
        return Ok(());
    }
    match delete_generic_password(SERVICE, account) {
        Ok(()) => {
            info!("Deleted {account} from Keychain");
            Ok(())
        }
        Err(e) => {
            let desc = format!("{e}");
            if desc.contains("not found") || desc.contains("-25300") {
                debug!("Keychain entry {account} not found, nothing to delete");
                Ok(())
            } else {
                Err(e).with_context(|| format!("Failed to delete Keychain entry for {account}"))
            }
        }
    }
}

/// Populates environment variables from Keychain for any keys not already set.
///
/// This ensures `.env` values always take priority over Keychain entries.
pub fn populate_env_from_keychain() {
    if is_test_env() {
        debug!("Test env: skipping Keychain population");
        return;
    }
    for &account in KEYCHAIN_ACCOUNTS {
        if std::env::var(account).is_err()
            && let Some(value) = load_key(account)
        {
            // SAFETY: called during single-threaded init before spawning workers.
            unsafe {
                std::env::set_var(account, &value);
            }
            debug!("Set {account} from Keychain");
        }
    }
}
