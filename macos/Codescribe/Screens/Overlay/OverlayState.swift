import SwiftUI
import AppKit

// View model for the dictation overlay, rewired from the old qube-ffi bridge
// (VistaEngine / VistaEventListener) onto the codescribe-ffi bridge
// (CodescribeDictation / CsTranscriptionListener).
//
// The view talks only to the thin `DictationEngine` protocol below, so #Preview
// renders standalone against `MockDictationEngine`. The orchestrator injects a
// `RealDictationEngine` (adapter over `CodescribeDictation`) in App.swift.
//
// TRANSCRIPT MODEL (new bridge semantics):
//   on_preview  → interim utterance, REPLACE-not-append (utterance-local).
//   on_final    → completed VAD-bounded utterance → commit + clear preview.
//   on_vad_active → speech start/stop → drives the WaveformView pulse.
//   on_no_speech / on_error → transient toast.
//
// AMPLITUDE GAP unchanged: the FFI exposes no audio-level callback, so the
// waveform is ambient (synthetic eq) and merely gated on VAD activity.

// MARK: - Engine seam (orchestrator injects the real adapter in App.swift)

/// Minimal slice of `CodescribeDictation` the overlay needs. The concrete
/// `RealDictationEngine` adapter (below) forwards every member to the bridge
/// object; `MockDictationEngine` no-ops for #Preview. Kept as a protocol so the
/// view-model + preview compile without a live Rust core.
protocol DictationEngine: AnyObject {
    func setListener(_ listener: CsTranscriptionListener)
    func startRecording(language: CsLanguage?) async throws
    func stopRecording() async throws -> String
    func isRecording() async -> Bool
    func initModel() async throws
    func isModelLoaded() -> Bool
    func transcribeFile(path: String) async throws -> CsTranscription
}

/// Two-state machine mirrored from the mock: live dictation vs the finalized
/// transcript returned by `stopRecording`.
enum OverlayMode: Equatable {
    case listening
    case formatted
}

@MainActor
final class OverlayState: ObservableObject {

    // MARK: Published state
    @Published var mode: OverlayMode = .listening
    @Published var preview: String = ""        // current utterance interim (replace-not-append)
    @Published var committed: String = ""      // accumulated finals (joined)
    @Published var formattedText: String = ""  // finalized transcript after stop
    @Published var vadActive: Bool = false     // drives the WaveformView pulse
    @Published var toast: String?              // transient no-speech / error notice
    @Published var errorMessage: String?

    // MARK: Injected collaborators (all optional so #Preview renders standalone)
    /// The recording core. Injected by the orchestrator. Do NOT instantiate here.
    var engine: DictationEngine?
    /// Handoff to the agent surface — wired by the orchestrator (routes the text
    /// into AgentChat, which streams it through `CodescribeAgent.streamReply`).
    var onSendToAgent: ((String) -> Void)?
    /// Dismiss the floating window — wired by the orchestrator.
    var onClose: (() -> Void)?

    /// Strong ref so the Rust-side callback (held via the UniFFI handle map) and
    /// our hop-to-main bridge stay alive for the lifetime of the overlay.
    private lazy var listener: CsTranscriptionListener = DictationListener(state: self)

    private var recording = false
    private var toastTask: Task<Void, Never>?
    private var mockRevealTask: Task<Void, Never>?

    init() {}

    // MARK: Derived display (one source of truth for the view)

    var statusText: String { mode == .listening ? "recording" : "Idle" }
    var statusColor: Color { mode == .listening ? CSColor.terracotta : CSColor.oliveLight }
    var statusRippling: Bool { mode == .listening }

    var tagText: String { mode == .listening ? "DICTATION" : "FINAL" }
    var tagColor: Color { mode == .listening ? CSColor.terracottaLight : CSColor.oliveLight }

    var metaText: String { mode == .listening ? "live preview · raw" : "final · transcript" }
    var footerRight: String { mode == .listening ? "vad-gated preview" : "captured" }

    /// committed finals + the current interim preview, space-joined.
    var liveText: String {
        [committed, preview].filter { !$0.isEmpty }.joined(separator: " ")
    }

    /// Text shown in the listening body, with the mock's "listening…" placeholder.
    var listeningDisplay: String { liveText.isEmpty ? "listening…" : liveText }

    /// Whatever the action row should copy/send for the current state.
    var activeText: String { mode == .listening ? liveText : formattedText }

    // MARK: Recording lifecycle (engine-backed; no-op when engine is absent)

    /// Start mic dictation. Gated on `micPermissionGranted()`; requests access
    /// once when undetermined. Fires the async bridge work in a Task so the view
    /// can call it from a synchronous context (onAppear / hotkey).
    func start(language: CsLanguage? = nil) {
        guard engine != nil, !recording else { return }
        Task { @MainActor in await self.runStart(language: language) }
    }

    /// Stop the mic and flip to the finalized transcript returned by the core.
    func stop() {
        guard engine != nil, recording else { return }
        Task { @MainActor in await self.runStop() }
    }

    private func runStart(language: CsLanguage?) async {
        guard let engine else { return }
        guard micPermissionGranted() || requestMicPermission() else {
            showToast("Microphone access denied")
            return
        }
        engine.setListener(listener)
        mode = .listening
        preview = ""
        committed = ""
        formattedText = ""
        errorMessage = nil
        recording = true
        do {
            if !engine.isModelLoaded() { try await engine.initModel() }
            try await engine.startRecording(language: language)
        } catch {
            recording = false
            errorMessage = "Couldn't start recording: \(error)"
            showToast("Couldn't start recording")
        }
    }

    private func runStop() async {
        guard let engine else { return }
        do {
            let raw = try await engine.stopRecording()
            recording = false
            vadActive = false
            formattedText = raw.isEmpty ? liveText : raw
            mode = .formatted
        } catch {
            recording = false
            errorMessage = "Couldn't finalize transcript: \(error)"
            showToast("Couldn't finalize transcript")
        }
    }

    // MARK: Action row

    func copyToPasteboard() {
        let pb = NSPasteboard.general
        pb.clearContents()
        pb.setString(activeText, forType: .string)
    }

    func sendToAgent() {
        onSendToAgent?(activeText)
    }

    func close() {
        mockRevealTask?.cancel()
        toastTask?.cancel()
        if recording, let engine {
            recording = false
            Task { @MainActor in _ = try? await engine.stopRecording() }
        }
        vadActive = false
        onClose?()
    }

    // MARK: Listener-driven mutations (called on the main actor by DictationListener)

    func applyPreview(_ text: String) {
        mode = .listening
        preview = text
    }

    func applyFinal(_ text: String) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmed.isEmpty {
            committed = committed.isEmpty ? trimmed : committed + " " + trimmed
        }
        preview = ""
    }

    func applyVad(_ active: Bool) {
        vadActive = active
    }

    func showToast(_ message: String) {
        toast = message
        toastTask?.cancel()
        toastTask = Task { @MainActor [weak self] in
            try? await Task.sleep(nanoseconds: 2_600_000_000)
            guard !Task.isCancelled else { return }
            self?.toast = nil
        }
    }

    // MARK: Preview / mock helpers (no engine required)

    /// Seeded view model for #Preview in the listening state, with a typing reveal
    /// that imitates `on_preview` arriving char-by-char (mock: 46ms).
    static func previewListening() -> OverlayState {
        let s = OverlayState()
        s.mode = .listening
        s.vadActive = true
        s.beginMockReveal("add a rate limiter to the login route and write a test for it")
        return s
    }

    /// Seeded view model for #Preview in the finalized state.
    static func previewFormatted() -> OverlayState {
        let s = OverlayState()
        s.mode = .formatted
        s.formattedText = "Add a rate limiter to the login route and write a test that covers the throttle window. Keep the existing error shape."
        return s
    }

    func beginMockReveal(_ full: String, interval: Double = 0.046) {
        mockRevealTask?.cancel()
        preview = ""
        mockRevealTask = Task { @MainActor [weak self] in
            var acc = ""
            for ch in full {
                if Task.isCancelled { return }
                acc.append(ch)
                self?.preview = acc
                try? await Task.sleep(nanoseconds: UInt64(interval * 1_000_000_000))
            }
        }
    }
}

// MARK: - Real adapter over the codescribe-ffi dictation engine

/// Backs the overlay with the REAL codescribe dictation engine via the UniFFI
/// bridge (`CodescribeDictation`). Pure forwarding; all callback hopping lives in
/// `DictationListener`.
final class RealDictationEngine: DictationEngine {
    private let dictation = CodescribeDictation()

    func setListener(_ listener: CsTranscriptionListener) {
        dictation.setListener(listener: listener)
    }
    func startRecording(language: CsLanguage?) async throws {
        try await dictation.startRecording(language: language)
    }
    func stopRecording() async throws -> String {
        try await dictation.stopRecording()
    }
    func isRecording() async -> Bool {
        await dictation.isRecording()
    }
    func initModel() async throws {
        try await dictation.initModel()
    }
    func isModelLoaded() -> Bool {
        dictation.isModelLoaded()
    }
    func transcribeFile(path: String) async throws -> CsTranscription {
        try await dictation.transcribeFile(path: path)
    }
}

// MARK: - Listener bridge (Rust callbacks → main actor → OverlayState)

/// Bridges Rust-side `CsTranscriptionListener` callbacks (fired from the core's
/// transcription thread) onto the main actor, driving `OverlayState`. Mirrors the
/// hop pattern used by `StreamListener` in RealChatEngine.
final class DictationListener: CsTranscriptionListener, @unchecked Sendable {
    private weak var state: OverlayState?

    init(state: OverlayState) {
        self.state = state
    }

    func onPreview(text: String) {
        DispatchQueue.main.async { MainActor.assumeIsolated { self.state?.applyPreview(text) } }
    }
    func onFinal(text: String) {
        DispatchQueue.main.async { MainActor.assumeIsolated { self.state?.applyFinal(text) } }
    }
    func onVadActive(active: Bool) {
        DispatchQueue.main.async { MainActor.assumeIsolated { self.state?.applyVad(active) } }
    }
    func onNoSpeech(reason: String) {
        DispatchQueue.main.async { MainActor.assumeIsolated { self.state?.showToast("No speech: \(reason)") } }
    }
    func onError(message: String) {
        DispatchQueue.main.async {
            MainActor.assumeIsolated {
                self.state?.errorMessage = message
                self.state?.showToast(message)
            }
        }
    }
}

// MARK: - Mock engine for #Preview

#if DEBUG
final class MockDictationEngine: DictationEngine {
    func setListener(_ listener: CsTranscriptionListener) {}
    func startRecording(language: CsLanguage?) async throws {}
    func stopRecording() async throws -> String { "" }
    func isRecording() async -> Bool { false }
    func initModel() async throws {}
    func isModelLoaded() -> Bool { true }
    func transcribeFile(path: String) async throws -> CsTranscription {
        CsTranscription(text: "", language: "en")
    }
}
#endif
