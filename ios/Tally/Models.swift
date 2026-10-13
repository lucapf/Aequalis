import Foundation

/// What `tally_eval` (as `tally --json`) answers: the result, or the error and, for a request
/// with several readings, the choices; the notices (the city archive missing, data out of date).
struct Reply: Decodable, Equatable {
    var ok: Bool
    var result: String?
    var error: String?
    var notes: [String]?
    var warnings: [String]?
    var ambiguous: Ambiguity?
    var notices: [String]?

    init(ok: Bool, result: String? = nil, error: String? = nil) {
        self.ok = ok
        self.result = result
        self.error = error
    }

    /// The line under the field: `= 6.2137 mi`, or what is wrong, or the question for the choices.
    var headline: String {
        if ok { return "= " + (result ?? "") }
        if let a = ambiguous {
            switch a.kind {
            case "city": return (a.note ?? "`\(a.text ?? "")` is not a city") + ": did you mean…"
            case "country": return "`\(a.text ?? "")` has several time zones: which one?"
            case "currency", "place": return (a.note ?? "") + ": which one?"
            default: return "Which date is \(a.text ?? "")?"
            }
        }
        return error ?? "no answer"
    }

    /// Warnings first, then notes (how a date was read, the currencies countries stand for).
    var remarks: [String] { ok ? (warnings ?? []) + (notes ?? []) : [] }

    /// The notices for the app: "tally --sync" is the Settings' download here.
    var appNotices: [String] {
        (notices ?? []).map { $0.replacingOccurrences(of: ": tally --sync", with: " (Settings › Download everything)") }
    }
}

/// Several readings of a request: a date (`12/04/2026`), a city (`Roma`), a currency (`dollar`),
/// a country with several time zones (`usa`).
struct Ambiguity: Decodable, Equatable {
    var kind: String?
    var text: String?
    var note: String?
    var choices: [Choice]
}

/// One reading: `pattern` and `date` describe it, `request` is the request to write instead.
struct Choice: Decodable, Equatable, Hashable {
    var pattern: String?
    var date: String?
    var request: String
}

/// While a word is being typed (no space after it, no Enter), errors and questions wait: `Rie` on
/// the way to `Rieti to Rome` is no mistake (as in the TUI and the widget).
func isQuiet(_ reply: Reply?, query: String, submitted: Bool) -> Bool {
    guard let reply, !reply.ok, !submitted else { return false }
    return !(query.last?.isWhitespace ?? true)
}

/// The value to copy: the result without the format's name (`2026/11/22 YYYY/MM/DD` →
/// `2026/11/22`); `6.2137 mi` stays as it is. Same rule as the widget's `valueOf()`.
func valueOf(_ result: String) -> String {
    var side = result
    if let range = result.range(of: " = ", options: .backwards),
       result.range(of: #"T\d\d:\d\d"#, options: .regularExpression) != nil {
        side = String(result[range.upperBound...])
    }
    if let iso = side.range(of: #"^\d{4}-\d\d-\d\dT\S+ "#, options: .regularExpression) {
        return String(side[iso]).trimmingCharacters(in: .whitespaces)
    }
    guard let cut = side.lastIndex(of: " ") else { return side }
    let last = String(side[side.index(after: cut)...])
    let format = #"^(iso|iso-[a-z]+|unix|unix-[a-z]+|local|[YMD]{2,4}/[YMD]{2,4}/[YMD]{2,4}(-(12|24)h(-tz)?)?)$"#
    return last.range(of: format, options: .regularExpression) != nil ? String(side[..<cut]) : side
}

/// `unit` added to the request: `1 km` on an empty one, else after it, with a space to go on.
func adding(_ unit: String, to query: String) -> String {
    let trimmed = query.trimmingCharacters(in: .whitespaces)
    return trimmed.isEmpty ? "1 \(unit) " : "\(trimmed) \(unit) "
}

/// A request and its answer, for Recent.
struct Entry: Codable, Equatable, Hashable {
    var input: String
    var result: String
}

/// Recent with `entry` first: no duplicate request, at most `limit` entries.
func remember(_ entry: Entry, in recent: [Entry], limit: Int = 10) -> [Entry] {
    Array(([entry] + recent.filter { $0.input != entry.input }).prefix(limit))
}

// MARK: - Lists from the core

struct SettingsList: Decodable { var settings: [Setting] }

/// A setting: its value, default, kind (`bool`, `choice`, or typed in), choices and what it does.
struct Setting: Decodable, Identifiable, Equatable {
    var key: String
    var value: String
    var `default`: String
    var kind: String
    var options: [String]?
    var about: String
    var id: String { key }
}

struct TriviaList: Decodable { var sections: [TriviaSection] }

struct TriviaSection: Decodable, Identifiable {
    var name: String
    var entries: [TriviaEntry]
    var id: String { name }
}

/// A Trivia entry: its value as shown, what goes into the request, and the popup's lines.
struct TriviaEntry: Decodable, Identifiable {
    var name: String
    var short: String
    var description: String
    var value: String
    var written: String
    var popup: [[String]]
    var id: String { name }
}

struct MeasureList: Decodable { var classes: [UnitClass] }

/// A unit class (Length, Temperature…), its two tabs (`Metric`/`Imperial`, `Decimal`/`Binary`…).
struct UnitClass: Decodable, Identifiable, Hashable {
    var name: String
    var key: String
    var systems: [String]
    var units: [UnitItem]
    var id: String { name }
}

struct UnitItem: Decodable, Identifiable, Hashable {
    var symbol: String
    var name: String
    /// `metric`, `imperial` or `both`.
    var system: String
    var id: String { symbol }
}

struct CurrencyList: Decodable { var currencies: [Currency] }

struct Currency: Decodable, Identifiable, Hashable {
    var code: String
    var name: String
    var symbols: [String]
    var id: String { code }
}
