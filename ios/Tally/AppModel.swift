import SwiftUI
import UIKit

enum Tab: Hashable {
    case calculate, measures, currencies, trivia, settings
}

/// The app's state, as in the Omarchy widget: the request, its reply, Recent, the notices.
@MainActor
final class AppModel: ObservableObject {
    private let defaults = UserDefaults.standard

    @Published var tab: Tab = .calculate
    @Published var query = "" {
        didSet {
            submitted = false
            schedule()
        }
    }
    @Published private(set) var reply: Reply?
    /// Return was pressed on this request: its error shows even in the middle of a word.
    @Published private(set) var submitted = false
    @Published private(set) var recent: [Entry] = []
    @Published private(set) var copied = ""
    @Published private(set) var notices: [String] = []
    /// What the last download said.
    @Published private(set) var syncMessage = ""
    @Published private(set) var syncing = false

    private var pending: Task<Void, Never>?

    init() {
        if let data = defaults.data(forKey: "recent"), let saved = try? JSONDecoder().decode([Entry].self, from: data) {
            recent = saved
        }
    }

    /// No error or question while a word is being typed.
    var quiet: Bool { isQuiet(reply, query: query, submitted: submitted) }

    /// Evaluates the request 150 ms after the last change, cancelling the one before.
    private func schedule() {
        pending?.cancel()
        let request = query.trimmingCharacters(in: .whitespaces)
        pending = Task { [weak self] in
            try? await Task.sleep(nanoseconds: 150_000_000)
            guard let self, !Task.isCancelled else { return }
            guard !request.isEmpty else {
                self.reply = nil
                return
            }
            let answer = await Task.detached { Engine.eval(request) }.value
            guard !Task.isCancelled else { return }
            self.reply = answer
            self.notices = answer.appNotices
        }
    }

    /// Return or Copy: the value to the clipboard, the request to Recent; an error shows now.
    func submit() {
        submitted = true
        guard let reply, reply.ok, let result = reply.result else { return }
        recent = remember(Entry(input: query.trimmingCharacters(in: .whitespaces), result: result), in: recent)
        if let data = try? JSONEncoder().encode(recent) {
            defaults.set(data, forKey: "recent")
        }
        copied = valueOf(result)
        UIPasteboard.general.string = copied
    }

    /// A choice, a Recent entry, a Trivia value: written into the field, on the Calculate tab.
    func use(_ text: String) {
        query = text
        tab = .calculate
    }

    /// A unit or a currency from the lists: added to the request.
    func add(_ unit: String) {
        use(adding(unit, to: query))
    }

    func clearRecent() {
        recent = []
        defaults.removeObject(forKey: "recent")
    }

    /// Downloads the online data: by hand everything (Settings), at launch and in the background
    /// what the refresh settings say.
    func sync(automatic: Bool) {
        guard !syncing else { return }
        syncing = true
        if !automatic { syncMessage = "Downloading…" }
        Task {
            let answer = await Task.detached { Engine.sync(automatic: automatic) }.value
            syncing = false
            if !automatic || !answer.ok {
                syncMessage = answer.ok ? (answer.result ?? "") : (answer.error ?? "the download failed")
            }
            notices = answer.appNotices
            schedule()
        }
    }
}
