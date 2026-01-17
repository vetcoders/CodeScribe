pub mod app;
pub mod assistant;
pub mod lab;
pub mod prompts;
pub mod settings;
pub mod teacher;

#[cfg(target_arch = "wasm32")]
pub mod tauri;
