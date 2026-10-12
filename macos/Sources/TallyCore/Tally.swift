import Foundation

/// Where the tally CLI is. Apps opened from Finder don't get the shell's PATH, so the usual places
/// are tried in turn: the `command` setting, the copy inside Tally.app, then ~/.local/bin,
/// /opt/homebrew/bin and /usr/local/bin.
public func locateTally(
    setting: String?,
    bundled: String?,
    home: String = NSHomeDirectory(),
    isExecutable: (String) -> Bool = { FileManager.default.isExecutableFile(atPath: $0) }
) -> String? {
    var candidates: [String] = []
    if let s = setting?.trimmingCharacters(in: .whitespaces), !s.isEmpty {
        candidates.append((s as NSString).expandingTildeInPath)
    }
    if let b = bundled { candidates.append(b) }
    candidates += [home + "/.local/bin/tally", "/opt/homebrew/bin/tally", "/usr/local/bin/tally"]
    return candidates.first(where: isExecutable)
}

/// Runs the tally CLI.
public struct Tally {
    public var path: String
    /// `TALLY_MODE=offline`: no network at all (apps don't get the shell's environment).
    public var offline: Bool

    public init(path: String, offline: Bool) {
        self.path = path
        self.offline = offline
    }

    /// `tally --json <args>`, parsed; nil when it could not be run or said nothing readable.
    public func json(_ args: [String]) async -> Reply? {
        guard let out = await run(["--json"] + args) else { return nil }
        return Reply.decode(out)
    }

    /// The command's standard output; nil when it could not be started. Run off the main thread.
    public func run(_ args: [String]) async -> Data? {
        let path = self.path
        var env = ProcessInfo.processInfo.environment
        if offline { env["TALLY_MODE"] = "offline" }
        return await withCheckedContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                let process = Process()
                process.executableURL = URL(fileURLWithPath: path)
                process.arguments = args
                process.environment = env
                let pipe = Pipe()
                process.standardOutput = pipe
                process.standardError = FileHandle.nullDevice
                do {
                    try process.run()
                } catch {
                    continuation.resume(returning: nil)
                    return
                }
                let data = pipe.fileHandleForReading.readDataToEndOfFile()
                process.waitUntilExit()
                continuation.resume(returning: data)
            }
        }
    }
}
