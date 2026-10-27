import Foundation

/// What a request's reply says to show (`reply::view`): a result, a hint, a question with choices,
/// an error, or nothing.
struct Reply {
    var show = "none"
    var message = ""
    var result = ""
    /// What Copy gives: the number with its unit kept.
    var copy = ""
    var details: [String] = []
    var rates: String?
    var grouping: String?
    /// The other results, a line each (`extra`): `shoes: US 8.5 men, US 10 women`.
    var extras: [String] = []
    /// A calendar's months, today and holidays (`calendar`), to draw them as the TUI does.
    var calendar: [String: Any]?
    /// Online data to download (a new city archive, out-of-date rates): a Sync button each.
    var notices: [String] = []
    /// Enter's choice of a question.
    var defaultRequest: String?
    var choices: [(label: String, detail: String, request: String)] = []

    /// A result of several lines: a table, its columns lined up.
    var isTable: Bool { show == "result" && result.contains("\n") }

    init() {}

    init(json: [String: Any]) {
        func string(_ key: String) -> String? { json[key] as? String }
        show = string("show") ?? "none"
        message = string("message") ?? ""
        result = string("result") ?? ""
        copy = string("copy") ?? result
        details = json["details"] as? [String] ?? []
        rates = string("rates")
        grouping = string("grouping_shown") ?? string("grouping")
        defaultRequest = string("default")
        notices = json["notices"] as? [String] ?? []
        calendar = json["calendar"] as? [String: Any]
        for choice in json["choices"] as? [[String: Any]] ?? [] {
            if let label = choice["label"] as? String, let request = choice["request"] as? String {
                choices.append((label, choice["detail"] as? String ?? "", request))
            }
        }
        extras = Reply.extras(json["extra"] as? [[String: Any]] ?? [], result: result)
    }

    /// The extra results as the Omarchy widget says them: the request's other readings with what
    /// they convert (`clothes: 42 eu = US 12 women`); none beside a table, which says them already;
    /// the other clothes charts by their name; beside a size the other people's sizes alone.
    private static func extras(_ all: [[String: Any]], result: String) -> [String] {
        let table = result.contains("\n")
        let size = ["US", "UK", "EU", "JP", "CN"].contains(String(result.split(separator: " ").first ?? ""))
        return all.compactMap { extra in
            let type = extra["type"] as? String ?? ""
            let line = (extra["results"] as? [String] ?? []).joined(separator: ", ")
            if let from = extra["from"] as? String, !from.isEmpty { return "\(type): \(from) = \(line)" }
            if table { return nil }
            if type.hasPrefix("clothes ") { return "\(line) \(type.dropFirst(8))" }
            return size ? line : "\(type): \(line)"
        }
    }
}

/// The Rust core, through its C interface (`include/aequalis.h`).
enum Core {
    /// Reads and frees a JSON reply.
    private static func take(_ reply: UnsafeMutablePointer<CChar>?) -> [String: Any] {
        guard let reply = reply else { return [:] }
        defer { aequalis_free(reply) }
        let data = Data(bytes: reply, count: strlen(reply))
        return (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]
    }

    /// Starts the core with what the Mac says: the same folder as the command line
    /// (`~/.config/aequalis`), the time zone, the number format, the measurement system, the
    /// language and the first day of the week.
    static func start() {
        let locale = Locale.current
        var config: [String: Any] = [
            "dir": NSHomeDirectory() + "/.config",
            "zone": TimeZone.current.identifier,
            "zoneinfo": "/usr/share/zoneinfo",
            "imperial": !locale.usesMetricSystem,
        ]
        if let decimal = locale.decimalSeparator, let thousands = locale.groupingSeparator {
            config["decimal"] = decimal
            config["thousands"] = thousands
        }
        // The holidays' names in the Mac's language; the calendar's weeks from its first day (1 Sunday … 7 Saturday).
        if let language = locale.languageCode {
            config["language"] = language
        }
        switch Calendar.current.firstWeekday {
        case 1: config["week_start"] = "sunday"
        case 2: config["week_start"] = "monday"
        case 7: config["week_start"] = "saturday"
        default: break
        }
        guard let data = try? JSONSerialization.data(withJSONObject: config), let text = String(data: data, encoding: .utf8) else { return }
        let reply = take(aequalis_init(text))
        Log.write("core started: \(reply)")
        aequalis_set_fetcher(fetch)
    }

    /// The app's downloader: the core cannot start `curl` from an app.
    private static let fetch: @convention(c) (UnsafePointer<CChar>?, UnsafePointer<CChar>?, UInt32, Bool) -> Bool = { url, to, seconds, strict in
        guard let url = url, let to = to, let address = URL(string: String(cString: url)) else { return false }
        let target = URL(fileURLWithPath: String(cString: to))
        let done = DispatchSemaphore(value: 0)
        var saved = false
        let task = URLSession.shared.dataTask(with: URLRequest(url: address, timeoutInterval: TimeInterval(seconds))) { data, response, error in
            defer { done.signal() }
            guard let data = data, error == nil else { return }
            if strict, let http = response as? HTTPURLResponse, !(200..<300).contains(http.statusCode) { return }
            saved = (try? data.write(to: target)) != nil
        }
        task.resume()
        done.wait()
        return saved
    }

    static func view(_ request: String, submitted: Bool) -> Reply {
        Reply(json: take(aequalis_view(request, submitted)))
    }

    /// Downloads the online data (rates, the city archive): everything, or with `automatic` what the
    /// settings say. Blocks: call it off the main thread.
    @discardableResult
    static func sync(automatic: Bool) -> String {
        let reply = take(aequalis_sync(automatic))
        Log.write("sync(auto: \(automatic)): \(reply)")
        return reply["error"] as? String ?? ""
    }

    /// Every setting the app can change (`aequalis_settings`): key, value, default, kind, options,
    /// about, advanced, and `auto`, `detail`, `currencies` where they apply.
    static func settings() -> [[String: Any]] {
        take(aequalis_settings())["settings"] as? [[String: Any]] ?? []
    }

    /// Changes a setting (`nil`: its default); the error, empty when it was saved.
    @discardableResult
    static func set(_ key: String, _ value: String?) -> String {
        let reply: [String: Any]
        if let value = value {
            reply = take(value.withCString { aequalis_set(key, $0) })
        } else {
            reply = take(aequalis_set(key, nil))
        }
        Log.write("set \(key)=\(value ?? "default"): \(reply)")
        return reply["error"] as? String ?? ""
    }

    /// Every setting back to its default; the error, empty when done.
    @discardableResult
    static func reset() -> String {
        let reply = take(aequalis_reset())
        Log.write("reset: \(reply)")
        return reply["error"] as? String ?? ""
    }

    /// What a UI says of the sync now: `state` (idle, running, done, failed), `line`, `label`,
    /// `fraction` (without a percentage: none), `message`.
    static func syncStatus() -> [String: Any] {
        take(aequalis_sync_status())
    }

    /// Online or offline and the age of the last sync: `online (2h old)`.
    static func online() -> String {
        take(aequalis_online())["line"] as? String ?? ""
    }

    /// The version, and the facts About shows (`[label, text]`): the last sync among them.
    static func about() -> (version: String, facts: [(String, String)]) {
        let reply = take(aequalis_about())
        let facts = (reply["facts"] as? [[String]] ?? []).compactMap { $0.count == 2 ? ($0[0], $0[1]) : nil }
        return (reply["version"] as? String ?? "", facts)
    }

    /// The words to offer for the word being typed at the end of `before` (the text up to the
    /// cursor), from its third letter: where the word starts, in Unicode scalars, and the words.
    static func suggest(_ before: String) -> (start: Int, items: [(write: String, label: String)]) {
        let reply = take(aequalis_suggest(before, 3))
        let items = (reply["items"] as? [[String: Any]] ?? []).compactMap { item -> (String, String)? in
            guard let write = item["write"] as? String, let label = item["label"] as? String else { return nil }
            return (write, label)
        }
        return (reply["start"] as? Int ?? 0, items)
    }
}
