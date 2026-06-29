import SwiftUI

// codescribe redesign — SwiftUI host (Option B). Hosts the Agent Chat window, the
// standard Settings scene (⌘,), a menu-bar Tray, and the floating dictation
// Overlay — all backed by the REAL codescribe engine through the UniFFI bridge.
@main
struct CodescribeRedesignApp: App {
    @StateObject private var model = AppModel()

    init() {
        FontLoader.register()
    }

    var body: some Scene {
        WindowGroup("codescribe — Agent", id: "agent") {
            AgentChatView(store: model.chat)
                .frame(minWidth: 900, minHeight: 600)
        }
        .windowStyle(.titleBar)

        // Standard macOS Settings scene (⌘,) backed by the real config bridge.
        Settings {
            SettingsView(model: SettingsViewModel(engine: RealSettingsEngine()))
        }

        // Menu-bar tray: status, dictation toggle, quick toggles, navigation.
        MenuBarExtra("codescribe", systemImage: "waveform") {
            TrayMenuHost(model: model)
        }
        .menuBarExtraStyle(.window)
    }
}

/// Binds the tray's navigation intents to the SwiftUI scene actions (only
/// available inside a View's environment) + the overlay controller.
private struct TrayMenuHost: View {
    @ObservedObject var model: AppModel
    @Environment(\.openWindow) private var openWindow
    @Environment(\.openSettings) private var openSettings

    var body: some View {
        TrayMenuView(viewModel: model.tray)
            .onAppear {
                model.tray.onIntent = { intent in
                    switch intent {
                    case .openChat: openWindow(id: "agent")
                    case .openSettings: openSettings()
                    case .openOverlay: model.overlay.show()
                    }
                }
            }
    }
}
