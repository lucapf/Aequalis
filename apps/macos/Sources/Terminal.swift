import Foundation

/// The interactive mode (`aequalis` with no arguments, the command line inside the app) in a new
/// Terminal window. One at a time: the shell running it writes its pid, and while that process
/// lives the TUI is open. No permission is needed to know it (no Automation, no Accessibility).
enum Terminal {
    private static let folder = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("Aequalis", isDirectory: true)
    private static let script = folder.appendingPathComponent("aequalis.command")
    private static let pidFile = folder.appendingPathComponent("tui.pid")

    /// The command line inside the app (`Contents/Resources/aequalis`).
    private static var command: String {
        Bundle.main.path(forResource: "aequalis", ofType: nil) ?? "aequalis"
    }

    /// Whether the TUI opened from here still runs.
    static var isOpen: Bool {
        guard let text = try? String(contentsOf: pidFile, encoding: .utf8), let pid = pid_t(text.trimmingCharacters(in: .whitespacesAndNewlines)) else {
            return false
        }
        return kill(pid, 0) == 0
    }

    /// Opens the TUI in Terminal, unless it is open already; the error, empty when it opened.
    @discardableResult
    static func openTUI() -> String {
        guard !isOpen else { return "" }
        let quoted = "'" + command.replacingOccurrences(of: "'", with: "'\\''") + "'"
        let lines = [
            "#!/bin/sh",
            "echo $$ > '\(pidFile.path)'",
            "clear",
            "exec \(quoted)",
        ]
        do {
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            try? FileManager.default.removeItem(at: pidFile)
            try (lines.joined(separator: "\n") + "\n").write(to: script, atomically: true, encoding: .utf8)
            try FileManager.default.setAttributes([.posixPermissions: 0o700], ofItemAtPath: script.path)
            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/usr/bin/open")
            process.arguments = ["-a", "Terminal", script.path]
            try process.run()
            Log.write("TUI opened in Terminal")
            return ""
        } catch {
            Log.write("TUI not opened: \(error)")
            return error.localizedDescription
        }
    }
}
