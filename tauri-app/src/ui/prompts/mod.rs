//! Prompts Tab - Edit AI prompts
//!
//! Allows editing formatting and assistive prompts.
//! Changes are saved to CLI via IPC.
//!
//! Created by M&K (c)2026 VetCoders

use leptos::prelude::*;

#[cfg(target_arch = "wasm32")]
use crate::ui::tauri::invoke;

/// Prompt type selector
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PromptType {
    Formatting,
    Assistive,
}

impl PromptType {
    fn as_str(&self) -> &'static str {
        match self {
            PromptType::Formatting => "formatting",
            PromptType::Assistive => "assistive",
        }
    }

    fn label(&self) -> &'static str {
        match self {
            PromptType::Formatting => "Formatting (KURIER)",
            PromptType::Assistive => "Assistive (ASYSTENT)",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            PromptType::Formatting => "Used for cleaning up raw transcription output",
            PromptType::Assistive => "Used for AI assistant conversations (Ctrl+Shift mode)",
        }
    }
}

/// Prompts Editor View
#[component]
pub fn PromptsView() -> impl IntoView {
    let (prompt_type, set_prompt_type) = signal(PromptType::Formatting);
    let (content, set_content) = signal(String::new());
    let (is_loading, set_loading) = signal(false);
    let (is_saving, set_saving) = signal(false);
    let (status, set_status) = signal(Option::<(bool, String)>::None);
    let (is_modified, set_modified) = signal(false);

    // Load prompt when type changes
    Effect::new(move |_| {
        let pt = prompt_type.get();
        set_loading.set(true);
        set_modified.set(false);

        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen_futures::spawn_local;

            let prompt_type_str = pt.as_str().to_string();
            spawn_local(async move {
                #[derive(serde::Serialize)]
                struct GetPromptArgs {
                    #[serde(rename = "promptType")]
                    prompt_type: String,
                }

                match invoke::<String>(
                    "get_ai_prompt",
                    GetPromptArgs {
                        prompt_type: prompt_type_str,
                    },
                )
                .await
                {
                    Ok(prompt) => {
                        set_content.set(prompt);
                    }
                    Err(e) => {
                        set_status.set(Some((false, format!("Failed to load: {}", e))));
                    }
                }
                set_loading.set(false);
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            set_loading.set(false);
        }
    });

    // Save prompt handler
    let save_prompt = move |_| {
        let pt = prompt_type.get();
        let text = content.get();

        set_saving.set(true);
        set_status.set(None);

        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen_futures::spawn_local;

            let prompt_type_str = pt.as_str().to_string();
            spawn_local(async move {
                #[derive(serde::Serialize)]
                struct SavePromptArgs {
                    #[serde(rename = "promptType")]
                    prompt_type: String,
                    content: String,
                }

                match invoke::<()>(
                    "save_ai_prompt",
                    SavePromptArgs {
                        prompt_type: prompt_type_str,
                        content: text,
                    },
                )
                .await
                {
                    Ok(_) => {
                        set_status.set(Some((true, "Saved successfully!".to_string())));
                        set_modified.set(false);
                    }
                    Err(e) => {
                        set_status.set(Some((false, format!("Save failed: {}", e))));
                    }
                }
                set_saving.set(false);

                // Clear success message after 3 seconds
                {
                    use gloo_timers::future::TimeoutFuture;
                    TimeoutFuture::new(3000).await;
                    set_status.set(None);
                }
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            set_saving.set(false);
        }
    };

    // Reset to default handler
    let reset_prompt = move |_| {
        let pt = prompt_type.get();

        set_loading.set(true);
        set_status.set(None);

        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen_futures::spawn_local;

            let prompt_type_str = pt.as_str().to_string();
            spawn_local(async move {
                #[derive(serde::Serialize)]
                struct PromptTypeArgs {
                    #[serde(rename = "promptType")]
                    prompt_type: String,
                }

                match invoke::<()>(
                    "reset_ai_prompt",
                    PromptTypeArgs {
                        prompt_type: prompt_type_str.clone(),
                    },
                )
                .await
                {
                    Ok(_) => {
                        // Reload the prompt
                        if let Ok(prompt) = invoke::<String>(
                            "get_ai_prompt",
                            PromptTypeArgs {
                                prompt_type: prompt_type_str,
                            },
                        )
                        .await
                        {
                            set_content.set(prompt);
                        }
                        set_status.set(Some((true, "Reset to default!".to_string())));
                        set_modified.set(false);
                    }
                    Err(e) => {
                        set_status.set(Some((false, format!("Reset failed: {}", e))));
                    }
                }
                set_loading.set(false);
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            set_loading.set(false);
        }
    };

    view! {
        <div class="prompts-container">
            <div class="prompts-header">
                <h2>"AI Prompts"</h2>
                <p class="description">"Edit the system prompts used by CodeScribe AI."</p>
            </div>

            <div class="prompt-selector">
                <label>"Prompt Type:"</label>
                <select
                    on:change=move |ev| {
                        let value = event_target_value(&ev);
                        let pt = match value.as_str() {
                            "formatting" => PromptType::Formatting,
                            "assistive" => PromptType::Assistive,
                            _ => PromptType::Formatting,
                        };
                        set_prompt_type.set(pt);
                    }
                >
                    <option value="formatting" selected=move || prompt_type.get() == PromptType::Formatting>
                        {PromptType::Formatting.label()}
                    </option>
                    <option value="assistive" selected=move || prompt_type.get() == PromptType::Assistive>
                        {PromptType::Assistive.label()}
                    </option>
                </select>
                <p class="type-description">{move || prompt_type.get().description()}</p>
            </div>

            <div class="editor-container">
                {move || if is_loading.get() {
                    view! { <div class="loading">"Loading prompt..."</div> }.into_any()
                } else {
                    view! {
                        <textarea
                            class="prompt-editor"
                            placeholder="Enter prompt..."
                            prop:value=move || content.get()
                            on:input=move |ev| {
                                set_content.set(event_target_value(&ev));
                                set_modified.set(true);
                            }
                        />
                    }.into_any()
                }}
            </div>

            <div class="actions">
                <button
                    class="save-btn"
                    on:click=save_prompt
                    disabled=move || is_saving.get() || is_loading.get() || !is_modified.get()
                >
                    {move || if is_saving.get() { "Saving..." } else { "Save Changes" }}
                </button>
                <button
                    class="reset-btn"
                    on:click=reset_prompt
                    disabled=move || is_loading.get()
                >
                    "Reset to Default"
                </button>
            </div>

            {move || status.get().map(|(success, msg)| {
                let class = if success { "status success" } else { "status error" };
                view! { <div class=class>{msg}</div> }
            })}

            {move || is_modified.get().then(|| view! {
                <div class="unsaved-warning">"* Unsaved changes"</div>
            })}
        </div>
    }
}
