import AppKit
import ServiceManagement
import TallyCore

/// The panel's state, as in the Omarchy widget (omarchy-widget/Panel.qml): the request, its reply
/// from `tally --json`, Recent, online or offline, About.
@MainActor
final class PanelModel: ObservableObject {
    private let defaults = UserDefaults.standard

    @Published var query = "" {
        didSet { schedule() }
    }
    @Published private(set) var reply: Reply?
    /// The tally command was not found.
    @Published private(set) var missing = false
    @Published private(set) var recent: [Entry] = []
    @Published private(set) var copied = ""
    /// `tally --online`: online, offline, offline (forced), not synced; `checking` until it answers.
    @Published private(set) var status = "checking"
    @Published var showAbout = false {
        didSet { if showAbout { refreshAbout() } }
    }
    /// `tally --about`: the logo and the facts.
    @Published private(set) var about = ""
    /// Bumped when the panel opens: the field takes the focus.
    @Published private(set) var focusToken = 0

    /// `TALLY_MODE=offline` for every run (apps don't get the shell's environment).
    @Published var offline: Bool {
        didSet {
            defaults.set(offline, forKey: "offline")
            refreshStatus()
            schedule()
        }
    }
    /// The tally binary, when not found by itself (empty: automatic).
    @Published var command: String {
        didSet {
            defaults.set(command, forKey: "command")
            refreshStatus()
            schedule()
        }
    }

    private var pending: Task<Void, Never>?
    private var timer: Timer?

    init() {
        offline = defaults.bool(forKey: "offline")
        command = defaults.string(forKey: "command") ?? ""
        if let data = defaults.data(forKey: "recent"), let saved = try? JSONDecoder().decode([Entry].self, from: data) {
            recent = saved
        }
    }

    /// The tally CLI to run, if there is one.
    var tally: Tally? {
        let bundled = Bundle.main.path(forResource: "tally", ofType: nil)
        return locateTally(setting: command, bundled: bundled).map { Tally(path: $0, offline: offline) }
    }

    // MARK: - The request

    /// Evaluates the request 150 ms after the last change, cancelling the one before.
    private func schedule() {
        pending?.cancel()
        let request = words(query)
        pending = Task { [weak self] in
            try? await Task.sleep(nanoseconds: 150_000_000)
            guard let self, !Task.isCancelled else { return }
            guard !request.isEmpty else {
                self.reply = nil
                return
            }
            guard let tally = self.tally else {
                self.missing = true
                self.reply = nil
                return
            }
            let answer = await tally.json(request)
            guard !Task.isCancelled else { return }
            self.missing = false
            self.reply = answer ?? Reply(ok: false, error: "no answer from \(tally.path)")
        }
    }

    /// Enter or Copy: the value to the clipboard, the request to Recent.
    func commit() {
        guard let reply, reply.ok, let result = reply.result else { return }
        recent = remember(Entry(input: query.trimmingCharacters(in: .whitespaces), result: result), in: recent)
        if let data = try? JSONEncoder().encode(recent) {
            defaults.set(data, forKey: "recent")
        }
        copied = valueOf(result)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(copied, forType: .string)
    }

    /// A choice or a Recent entry: written into the field.
    func use(_ text: String) {
        query = text
        focusToken += 1
    }

    // MARK: - Open and close

    func opened() {
        focusToken += 1
        refreshStatus()
        if showAbout { refreshAbout() }
        timer?.invalidate()
        timer = Timer.scheduledTimer(withTimeInterval: 30, repeats: true) { [weak self] _ in
            guard let model = self else { return }
            Task { @MainActor in model.refreshStatus() }
        }
    }

    func closed() {
        timer?.invalidate()
        timer = nil
    }

    private func refreshStatus() {
        Task {
            guard let tally else {
                status = "not installed"
                return
            }
            status = await tally.json(["--online"])?.result ?? "offline"
        }
    }

    private func refreshAbout() {
        Task {
            guard let tally else {
                about = "The tally command was not found."
                return
            }
            about = await tally.json(["--about"])?.result ?? ""
        }
    }

    /// The last request's peak memory and the binary's size: `last request 3.9 MiB peak · binary 859 KiB`.
    var memory: String {
        var parts: [String] = []
        if let kb = reply?.peakKb { parts.append("last request \(size(kb: kb)) peak") }
        if let path = tally?.path, let bytes = (try? FileManager.default.attributesOfItem(atPath: path))?[.size] as? Int {
            parts.append("binary \(size(kb: bytes / 1024))")
        }
        return parts.joined(separator: " · ")
    }

    // MARK: - The interactive mode, login, quit

    /// The full interactive mode in Terminal: a `.command` file, which Terminal runs when opened
    /// (no Automation permission needed).
    func openTui() {
        guard let tally else { return }
        let script = FileManager.default.temporaryDirectory.appendingPathComponent("Tally.command")
        let quoted = "'" + tally.path.replacingOccurrences(of: "'", with: "'\\''") + "'"
        let env = tally.offline ? "TALLY_MODE=offline " : ""
        let text = "#!/bin/sh\nclear\n\(env)exec \(quoted)\n"
        do {
            try text.write(to: script, atomically: true, encoding: .utf8)
            try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: script.path)
            NSWorkspace.shared.open(script)
        } catch {
            status = "could not open the TUI"
        }
    }

    var launchAtLogin: Bool {
        get { SMAppService.mainApp.status == .enabled }
        set {
            objectWillChange.send()
            if newValue {
                try? SMAppService.mainApp.register()
            } else {
                try? SMAppService.mainApp.unregister()
            }
        }
    }
}
