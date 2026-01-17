use crate::ui::assistant::AssistantView;
use crate::ui::lab::LabView;
use crate::ui::prompts::PromptsView;
use crate::ui::settings::SettingsView;
use crate::ui::teacher::TeacherView;
use leptos::prelude::*;

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Assistant,
    Prompts,
    Lab,
    Teacher,
    Settings,
}

#[component]
pub fn App() -> impl IntoView {
    let (active_tab, set_active_tab) = signal(Tab::Assistant);

    view! {
        <div class="app-container">
            <nav class="tab-strip">
                <button
                    class=move || if active_tab.get() == Tab::Assistant { "active" } else { "" }
                    on:click=move |_| set_active_tab.set(Tab::Assistant)
                >
                    "Assistant"
                </button>
                <button
                    class=move || if active_tab.get() == Tab::Prompts { "active" } else { "" }
                    on:click=move |_| set_active_tab.set(Tab::Prompts)
                >
                    "Prompts"
                </button>
                <button
                    class=move || if active_tab.get() == Tab::Lab { "active" } else { "" }
                    on:click=move |_| set_active_tab.set(Tab::Lab)
                >
                    "Voice Lab"
                </button>
                <button
                    class=move || if active_tab.get() == Tab::Teacher { "active" } else { "" }
                    on:click=move |_| set_active_tab.set(Tab::Teacher)
                >
                    "Teacher"
                </button>
                <button
                    class=move || if active_tab.get() == Tab::Settings { "active" } else { "" }
                    on:click=move |_| set_active_tab.set(Tab::Settings)
                >
                    "Settings"
                </button>
            </nav>
            <main class="content">
                <Show when=move || active_tab.get() == Tab::Assistant>
                    <AssistantView />
                </Show>
                <Show when=move || active_tab.get() == Tab::Prompts>
                    <PromptsView />
                </Show>
                <Show when=move || active_tab.get() == Tab::Lab>
                    <LabView />
                </Show>
                <Show when=move || active_tab.get() == Tab::Teacher>
                    <TeacherView />
                </Show>
                <Show when=move || active_tab.get() == Tab::Settings>
                    <SettingsView />
                </Show>
            </main>
        </div>
    }
}
