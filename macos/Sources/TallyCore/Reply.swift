import Foundation

/// What `tally --json …` prints: the result, or the error and, for a request with several
/// readings, the choices.
public struct Reply: Decodable, Equatable {
    public var ok: Bool
    public var result: String?
    public var error: String?
    public var exit: Int?
    public var notes: [String]?
    public var warnings: [String]?
    public var ambiguous: Ambiguity?
    public var peakKb: Int?

    enum CodingKeys: String, CodingKey {
        case ok, result, error, exit, notes, warnings, ambiguous
        case peakKb = "peak_kb"
    }

    public init(ok: Bool, result: String? = nil, error: String? = nil) {
        self.ok = ok
        self.result = result
        self.error = error
    }

    public static func decode(_ data: Data) -> Reply? {
        try? JSONDecoder().decode(Reply.self, from: data)
    }

    /// The line under the field: `= 6.2137 mi`, or what is wrong, or the question for the choices.
    public var headline: String {
        if ok { return "= " + (result ?? "") }
        if let a = ambiguous {
            switch a.kind {
            case "city": return (a.note ?? "`\(a.text ?? "")` is not a city") + ": did you mean…"
            case "country": return "`\(a.text ?? "")` has several time zones: which one?"
            default: return "Which date is \(a.text ?? "")?"
            }
        }
        return error ?? "no answer"
    }

    /// Warnings first, then notes (how a date was read).
    public var remarks: [String] {
        ok ? (warnings ?? []) + (notes ?? []) : []
    }
}

/// Several readings of a request: a date (`12/04/2026`), a misspelt city (`Tokio`), a country
/// with several time zones (`usa`).
public struct Ambiguity: Decodable, Equatable {
    public var kind: String?
    public var text: String?
    public var note: String?
    public var choices: [Choice]
}

/// One reading: `pattern` and `date` describe it, `request` is the request to write instead.
public struct Choice: Decodable, Equatable, Hashable {
    public var pattern: String?
    public var date: String?
    public var request: String

    public var label: String {
        [pattern, date].compactMap { $0 }.joined(separator: "   ")
    }
}

/// The request's words, as the command line gets them.
public func words(_ query: String) -> [String] {
    query.split(whereSeparator: { $0.isWhitespace }).map(String.init)
}

/// The value to copy: the result without the format's name (`2026/11/22 YYYY/MM/DD` →
/// `2026/11/22`), the ISO time of a city's time; `6.2137 mi` stays as it is (the unit is part of
/// the value). Same rule as `valueOf()` in omarchy-widget/Panel.qml.
public func valueOf(_ result: String) -> String {
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

/// 900 KiB, 2.9 MiB.
public func size(kb: Int) -> String {
    kb >= 1024 ? String(format: "%.1f MiB", Double(kb) / 1024) : "\(kb) KiB"
}

/// A request and its answer, for Recent.
public struct Entry: Codable, Equatable, Hashable {
    public var input: String
    public var result: String

    public init(input: String, result: String) {
        self.input = input
        self.result = result
    }
}

/// Recent with `entry` first: no duplicate request, at most `limit` entries.
public func remember(_ entry: Entry, in recent: [Entry], limit: Int = 6) -> [Entry] {
    Array(([entry] + recent.filter { $0.input != entry.input }).prefix(limit))
}
