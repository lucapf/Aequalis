import Foundation
import TallyFFI

/// Tally's Rust core (src/ffi.rs through include/tally.h), in-process: an iPhone app can't run
/// the `tally` program. Every call returns JSON, decoded here. Calls can take a moment (a sync
/// downloads): run them off the main thread (`Task.detached`).
enum Engine {
    /// Starts the core once: the data folder, the time zone, the number format and the measurement
    /// system, as the device says; and the downloader.
    static let started: Bool = {
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        try? FileManager.default.createDirectory(at: support, withIntermediateDirectories: true)
        let locale = Locale.current
        var config: [String: Any] = [
            "dir": support.path,
            "zone": TimeZone.current.identifier,
            "decimal": locale.decimalSeparator ?? ".",
            "thousands": locale.groupingSeparator ?? ",",
            "grouping": [3],
            "imperial": locale.measurementSystem == .us,
        ]
        if let zoneinfo = zoneinfo() { config["zoneinfo"] = zoneinfo }
        let json = String(data: try! JSONSerialization.data(withJSONObject: config), encoding: .utf8)!
        let reply = call { tally_init(json) }
        tally_set_fetcher(fetch)
        return reply.contains("\"ok\":true")
    }()

    /// The zone files: the system's when the app may read them, else the copy in the app.
    static func zoneinfo() -> String? {
        if FileManager.default.isReadableFile(atPath: "/usr/share/zoneinfo/Europe/Rome") { return "/usr/share/zoneinfo" }
        return Bundle.main.path(forResource: "zoneinfo", ofType: nil)
    }

    /// A string from the core, freed.
    private static func call(_ f: () -> UnsafeMutablePointer<CChar>?) -> String {
        guard let p = f() else { return #"{"ok":false,"error":"no answer"}"# }
        defer { tally_free(p) }
        return String(cString: p)
    }

    private static func decode<T: Decodable>(_ type: T.Type, _ json: String) -> T? {
        try? JSONDecoder().decode(type, from: Data(json.utf8))
    }

    /// The reply to a request, as `tally --json <request>`.
    static func eval(_ request: String) -> Reply {
        _ = started
        let json = call { tally_eval(request) }
        return decode(Reply.self, json) ?? Reply(ok: false, error: "unreadable answer")
    }

    static func settings() -> [Setting] {
        _ = started
        return decode(SettingsList.self, call { tally_settings() })?.settings ?? []
    }

    /// Changes a setting (nil: its default); the error, if any.
    static func set(_ key: String, _ value: String?) -> String? {
        _ = started
        let json = call { value.map { v in tally_set(key, v) } ?? tally_set(key, nil) }
        let reply = decode(Reply.self, json)
        return reply?.ok == true ? nil : (reply?.error ?? "could not save")
    }

    /// Downloads the online data: everything, or with `automatic` what the settings say.
    static func sync(automatic: Bool) -> Reply {
        _ = started
        return decode(Reply.self, call { tally_sync(automatic) }) ?? Reply(ok: false, error: "no answer")
    }

    static func trivia() -> [TriviaSection] {
        _ = started
        return decode(TriviaList.self, call { tally_trivia() })?.sections ?? []
    }

    static func measures() -> [UnitClass] {
        _ = started
        return decode(MeasureList.self, call { tally_measures() })?.classes ?? []
    }

    static func currencies() -> [Currency] {
        _ = started
        return decode(CurrencyList.self, call { tally_currencies() })?.currencies ?? []
    }
}

/// The downloader the core calls (from the thread that syncs): `url` saved into the file `to`.
private let fetch: tally_fetch_fn = { url, to, seconds, strict in
    guard let url, let to, let source = URL(string: String(cString: url)) else { return false }
    return Downloader.save(source, to: URL(fileURLWithPath: String(cString: to)), seconds: TimeInterval(seconds), strict: strict)
}

enum Downloader {
    /// Waits for the download (the core's thread, never the main one); with `strict`, an HTTP
    /// error status is a failure.
    static func save(_ url: URL, to destination: URL, seconds: TimeInterval, strict: Bool) -> Bool {
        if url.isFileURL {
            try? FileManager.default.removeItem(at: destination)
            return (try? FileManager.default.copyItem(at: url, to: destination)) != nil
        }
        var request = URLRequest(url: url, timeoutInterval: seconds)
        request.setValue("tally-ios", forHTTPHeaderField: "User-Agent")
        let done = DispatchSemaphore(value: 0)
        var saved = false
        URLSession.shared.downloadTask(with: request) { file, response, _ in
            defer { done.signal() }
            guard let file else { return }
            if strict, let http = response as? HTTPURLResponse, !(200..<300).contains(http.statusCode) { return }
            try? FileManager.default.removeItem(at: destination)
            saved = (try? FileManager.default.moveItem(at: file, to: destination)) != nil
        }.resume()
        done.wait()
        return saved
    }
}
