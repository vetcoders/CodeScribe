import Foundation

// Backs the thread rail / drawer with REAL persisted threads from the codescribe
// ThreadStore via the UniFFI bridge (CodescribeThreads). Lists/searches thread
// summaries for the rail, loads messages on demand, and forwards lightweight
// thread mutations that already exist in the core.
final class RealThreadsEngine: ChatThreadsProviding {
    private let threads = CodescribeThreads()

    func listThreads() -> [ChatThread] {
        guard let list = try? threads.listThreads(filter: nil) else { return [] }
        return list.map(Self.thread)
    }

    func searchThreads(query: String) -> [ChatThread] {
        guard let list = try? threads.searchThreads(query: query) else { return [] }
        return list.map(Self.thread)
    }

    func generateThreadId() -> String {
        threads.generateThreadId()
    }

    func loadMessages(backendId: String) -> [ChatMessage] {
        guard let thread = try? threads.loadThread(id: backendId) else { return [] }
        return thread.messages.compactMap { message -> ChatMessage? in
            let text = message.text.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !text.isEmpty else { return nil }
            switch message.role {
            case "user": return ChatMessage(role: .you, timestamp: "", text: text)
            case "assistant": return ChatMessage(role: .assistant, timestamp: "", text: text)
            default: return nil  // skip system/tool turns in the transcript view
            }
        }
    }

    func deleteThread(backendId: String) -> Bool {
        do {
            try threads.deleteThread(id: backendId)
            return true
        } catch {
            return false
        }
    }

    func setThreadFavorite(backendId: String, isFavorite: Bool) -> Bool {
        (try? threads.setThreadFavorite(id: backendId, isFavorite: isFavorite)) ?? false
    }

    private static func thread(from summary: CsThreadSummary) -> ChatThread {
        var thread = ChatThread(
            title: summary.title.isEmpty ? "Untitled" : summary.title,
            meta: metaString(updatedAtMs: summary.updatedAtMs),
            isFavorite: summary.isFavorite
        )
        thread.backendId = summary.id
        return thread
    }

    /// "today HH:mm" / "yesterday" / "MMM d" from an epoch-millis timestamp.
    private static func metaString(updatedAtMs: Int64) -> String {
        let date = Date(timeIntervalSince1970: Double(updatedAtMs) / 1000.0)
        let formatter = DateFormatter()
        if Calendar.current.isDateInToday(date) {
            formatter.dateFormat = "'today' HH:mm"
        } else if Calendar.current.isDateInYesterday(date) {
            formatter.dateFormat = "'yesterday'"
        } else {
            formatter.dateFormat = "MMM d"
        }
        return formatter.string(from: date)
    }
}
