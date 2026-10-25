import Foundation

/// `~/Library/Logs/Aequalis.log`: what the app did (hotkey, panel, services, sync), for when it
/// misbehaves on a Mac nobody can debug on. Cut when it passes half a megabyte.
enum Log {
    static let url = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Logs/Aequalis.log")
    private static let queue = DispatchQueue(label: "aequalis.log")
    private static let stamp: DateFormatter = {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.dateFormat = "yyyy-MM-dd HH:mm:ss.SSS"
        return f
    }()

    static func write(_ message: String) {
        queue.async {
            let line = "\(stamp.string(from: Date())) \(message)\n"
            let fm = FileManager.default
            if let size = (try? fm.attributesOfItem(atPath: url.path))?[.size] as? Int, size > 512 * 1024 {
                try? fm.removeItem(at: url)
            }
            if !fm.fileExists(atPath: url.path) {
                try? fm.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
                fm.createFile(atPath: url.path, contents: nil)
            }
            guard let handle = try? FileHandle(forWritingTo: url), let data = line.data(using: .utf8) else { return }
            handle.seekToEndOfFile()
            handle.write(data)
            try? handle.close()
        }
    }
}
