fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    if !target.contains("wasm32") {
        if std::env::var("TAURI_APP_VERSION").is_err() {
            if let Ok(version) = std::env::var("CARGO_PKG_VERSION") {
                std::env::set_var("TAURI_APP_VERSION", version);
            }
        }
        tauri_build::build()
    }
}
