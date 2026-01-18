//! Settings view for CodeScribe tray app
//!
//! Simplified settings focused on:
//! - Hotkey configuration (hold mods, toggle trigger)
//! - Audio input device selection
//! - Language preference
//!
//! Created by M&K (c)2026 VetCoders

use leptos::prelude::*;
use serde_json::Value;

use crate::ui::tauri;

#[derive(serde::Serialize)]
struct NoArgs {}

#[derive(serde::Serialize)]
struct SaveConfigArgs {
    config: Value,
}

#[component]
pub fn SettingsView() -> impl IntoView {
    let (loaded, set_loaded) = signal(false);
    let (error, set_error) = signal(None::<String>);
    let (saved, set_saved) = signal(false);

    // Hotkey settings
    let (hold_mods, set_hold_mods) = signal(String::from("ctrl"));
    let (toggle_trigger, set_toggle_trigger) = signal(String::from("double_option"));

    // Audio settings
    let (audio_devices, set_audio_devices) = signal(Vec::<String>::new());
    let (current_audio_device, set_current_audio_device) = signal(None::<String>);
    let (audio_input_device, set_audio_input_device) = signal(String::new());

    // Language
    let (whisper_language, set_whisper_language) = signal(String::from("auto"));

    // Load config on mount
    Effect::new(move |_| {
        if loaded.get() {
            return;
        }
        set_loaded.set(true);

        leptos::task::spawn_local(async move {
            // Load config
            let cfg: Result<Value, String> = tauri::invoke("get_config", NoArgs {}).await;
            match cfg {
                Ok(v) => {
                    set_hold_mods.set(
                        v.get("hold_mods")
                            .and_then(|x| x.as_str())
                            .unwrap_or("ctrl")
                            .to_string(),
                    );
                    set_toggle_trigger.set(
                        v.get("toggle_trigger")
                            .and_then(|x| x.as_str())
                            .unwrap_or("double_option")
                            .to_string(),
                    );
                    set_whisper_language.set(
                        v.get("whisper_language")
                            .and_then(|x| x.as_str())
                            .unwrap_or("auto")
                            .to_string(),
                    );
                    set_audio_input_device.set(
                        v.get("audio_input_device")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string(),
                    );
                }
                Err(e) => set_error.set(Some(e)),
            }

            // Load audio devices
            let devs: Result<Vec<String>, String> =
                tauri::invoke("list_audio_devices", NoArgs {}).await;
            if let Ok(v) = devs {
                set_audio_devices.set(v);
            }
            let current: Result<Option<String>, String> =
                tauri::invoke("get_current_audio_device", NoArgs {}).await;
            if let Ok(v) = current {
                set_current_audio_device.set(v);
            }
        });
    });

    let save_config = move |_| {
        set_error.set(None);
        set_saved.set(false);

        let payload = serde_json::json!({
            "hold_mods": hold_mods.get(),
            "toggle_trigger": toggle_trigger.get(),
            "whisper_language": whisper_language.get(),
            "audio_input_device": audio_input_device.get(),
        });

        leptos::task::spawn_local(async move {
            let res: Result<(), String> =
                tauri::invoke("save_config", SaveConfigArgs { config: payload }).await;
            match res {
                Ok(()) => set_saved.set(true),
                Err(e) => set_error.set(Some(e)),
            }
        });
    };

    view! {
        <div class="settings-page">
            <header class="settings-header">
                <h1>"Settings"</h1>
                <p class="subtitle">"Configure hotkeys, audio, and language"</p>
            </header>

            <Show when=move || error.get().is_some()>
                <div class="toast toast--error">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>

            <Show when=move || saved.get()>
                <div class="toast toast--success">
                    "Settings saved"
                </div>
            </Show>

            <div class="settings-grid">
                // Hotkeys Panel
                <section class="panel">
                    <div class="panel__header">
                        <span class="panel__icon">"⌨️"</span>
                        <h2>"Hotkeys"</h2>
                    </div>

                    <div class="field">
                        <label class="field__label">"Hold to record"</label>
                        <select
                            class="field__input"
                            prop:value=move || hold_mods.get()
                            on:change=move |ev| set_hold_mods.set(event_target_value(&ev))
                        >
                            <option value="ctrl">"Ctrl"</option>
                            <option value="ctrl_alt">"Ctrl + Option"</option>
                            <option value="ctrl_shift">"Ctrl + Shift"</option>
                            <option value="ctrl_cmd">"Ctrl + Command"</option>
                        </select>
                        <span class="field__hint">"Hold these keys to start recording"</span>
                    </div>

                    <div class="field">
                        <label class="field__label">"Toggle recording"</label>
                        <select
                            class="field__input"
                            prop:value=move || toggle_trigger.get()
                            on:change=move |ev| set_toggle_trigger.set(event_target_value(&ev))
                        >
                            <option value="double_option">"Double-tap Option"</option>
                            <option value="double_ralt">"Double-tap Right Option"</option>
                            <option value="none">"Disabled"</option>
                        </select>
                        <span class="field__hint">"Quick double-tap to toggle"</span>
                    </div>
                </section>

                // Audio Panel
                <section class="panel">
                    <div class="panel__header">
                        <span class="panel__icon">"🎤"</span>
                        <h2>"Audio"</h2>
                    </div>

                    <div class="field">
                        <label class="field__label">"Input device"</label>
                        <select
                            class="field__input"
                            prop:value=move || audio_input_device.get()
                            on:change=move |ev| set_audio_input_device.set(event_target_value(&ev))
                        >
                            <option value="">
                                {move || {
                                    let current = current_audio_device.get()
                                        .unwrap_or_else(|| "Unknown".to_string());
                                    format!("System default ({})", current)
                                }}
                            </option>
                            <For
                                each=move || audio_devices.get()
                                key=|d| d.clone()
                                children=move |d| view! {
                                    <option value={d.clone()}>{d.clone()}</option>
                                }
                            />
                        </select>
                    </div>
                </section>

                // Language Panel
                <section class="panel">
                    <div class="panel__header">
                        <span class="panel__icon">"🌐"</span>
                        <h2>"Language"</h2>
                    </div>

                    <div class="field">
                        <label class="field__label">"Transcription language"</label>
                        <select
                            class="field__input"
                            prop:value=move || whisper_language.get()
                            on:change=move |ev| set_whisper_language.set(event_target_value(&ev))
                        >
                            <option value="auto">"Auto-detect"</option>
                            <option value="en">"English"</option>
                            <option value="pl">"Polish"</option>
                            <option value="de">"German"</option>
                            <option value="es">"Spanish"</option>
                            <option value="fr">"French"</option>
                        </select>
                    </div>
                </section>
            </div>

            <footer class="settings-footer">
                <button class="btn btn--primary" on:click=save_config>
                    "Save Settings"
                </button>
                <span class="footer-hint">"Settings are applied immediately"</span>
            </footer>
        </div>
    }
}
