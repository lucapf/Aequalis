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
    /// Enter's choice of a question.
    var defaultRequest: String?
    var choices: [(label: String, request: String)] = []

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
        for choice in json["choices"] as? [[String: Any]] ?? [] {
            if let label = choice["label"] as? String, let request = choice["request"] as? String {
                choices.append((label, request))
            }
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

    /// The settings file's path (`aequalis_settings().file`).
    static func settingsFile() -> String? {
        take(aequalis_settings())["file"] as? String
    }
}
